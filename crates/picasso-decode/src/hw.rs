//! The graphics card: decoding video on it (what `ffmpeg -hwaccel auto` does) and handing pictures
//! to the VA-API encoders of Linux.
//!
//! ffmpeg-next has no safe wrapper for FFmpeg's hardware devices, so this is the one module of the
//! program with `unsafe` code. It only passes FFmpeg's own pointers back to FFmpeg, checks every
//! pointer and return code, and owns each reference it takes until `Drop` gives it back.
#![allow(unsafe_code)]

use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use ff::ffi;
use ff::format::Pixel;
use ffmpeg_next as ff;

/// Whether video is decoded on the graphics card where it can be (System Preferences > Decode
/// Options). It belongs to the computer, not to a project. On by default, as in 0.2.0.
static HARDWARE_DECODING: AtomicBool = AtomicBool::new(true);

/// Turns hardware decoding on or off for the decoders opened from now on.
pub fn set_hardware_decoding(on: bool) {
    HARDWARE_DECODING.store(on, Ordering::Relaxed);
}

/// Whether decoders opened now use the graphics card. `PICASSO_NO_HWACCEL` turns it off for one
/// run whatever the preference says.
pub fn hardware_decoding() -> bool {
    HARDWARE_DECODING.load(Ordering::Relaxed) && std::env::var_os("PICASSO_NO_HWACCEL").is_none()
}

/// The codecs worth decoding on the card. Intra-frame codecs such as ProRes or DNxHR are left to
/// the processor, which decodes them fast and which most cards cannot help with.
const CODECS: [ff::codec::Id; 6] = [
    ff::codec::Id::H264,
    ff::codec::Id::HEVC,
    ff::codec::Id::AV1,
    ff::codec::Id::VP9,
    ff::codec::Id::MPEG2VIDEO,
    ff::codec::Id::VC1,
];

/// The kinds of device tried, best first. D3D11 and VA-API work with every vendor's card; CUDA is
/// NVIDIA's own.
fn device_kinds() -> &'static [ffi::AVHWDeviceType] {
    use ffi::AVHWDeviceType::*;
    if cfg!(windows) {
        &[
            AV_HWDEVICE_TYPE_D3D11VA,
            AV_HWDEVICE_TYPE_CUDA,
            AV_HWDEVICE_TYPE_DXVA2,
        ]
    } else if cfg!(target_os = "macos") {
        &[AV_HWDEVICE_TYPE_VIDEOTOOLBOX]
    } else {
        &[AV_HWDEVICE_TYPE_VAAPI, AV_HWDEVICE_TYPE_CUDA]
    }
}

/// A reference to an FFmpeg hardware device or frame pool.
struct BufferRef(*mut ffi::AVBufferRef);

// SAFETY: FFmpeg's buffer references count atomically, and the device contexts made here (D3D11,
// DXVA2, CUDA, VA-API, VideoToolbox) lock themselves when two threads use them.
unsafe impl Send for BufferRef {}

impl BufferRef {
    /// A new reference to the same buffer, for FFmpeg to own.
    fn new_ref(&self) -> *mut ffi::AVBufferRef {
        // SAFETY: `self.0` is a live reference (never null: see where it is made).
        unsafe { ffi::av_buffer_ref(self.0) }
    }
}

impl Drop for BufferRef {
    fn drop(&mut self) {
        // SAFETY: the reference is ours; unref sets the pointer to null.
        unsafe { ffi::av_buffer_unref(&mut self.0) }
    }
}

/// Opens a device of `kind` on the default card.
fn create_device(kind: ffi::AVHWDeviceType) -> Option<BufferRef> {
    let mut device: *mut ffi::AVBufferRef = ptr::null_mut();
    // SAFETY: FFmpeg fills `device` on success and leaves it null otherwise.
    let made =
        unsafe { ffi::av_hwdevice_ctx_create(&mut device, kind, ptr::null(), ptr::null_mut(), 0) };
    (made >= 0 && !device.is_null()).then_some(BufferRef(device))
}

/// The device the decoders share: opened the first time one asks, once for the whole run. `None`
/// inside means none of [`device_kinds`] opened, which is not tried again.
static DEVICE: Mutex<Option<Option<(ffi::AVHWDeviceType, BufferRef)>>> = Mutex::new(None);

/// The decoder's pixel format for pictures that stay on a device of `kind`, if it has one.
fn hardware_format(codec: &ff::Codec, kind: ffi::AVHWDeviceType) -> Option<ffi::AVPixelFormat> {
    let method = ffi::AV_CODEC_HW_CONFIG_METHOD_HW_DEVICE_CTX as i32;
    for i in 0..32 {
        // SAFETY: the codec is FFmpeg's; past the last configuration the answer is null.
        let config = unsafe { ffi::avcodec_get_hw_config(codec.as_ptr(), i) };
        if config.is_null() {
            return None;
        }
        // SAFETY: not null, and FFmpeg's configurations live as long as the program.
        let config = unsafe { &*config };
        if config.methods & method != 0 && config.device_type == kind {
            return Some(config.pix_fmt);
        }
    }
    None
}

/// Chooses the hardware format kept in `opaque` when the decoder offers it; otherwise FFmpeg's own
/// choice, which decodes on the processor.
unsafe extern "C" fn pick_format(
    ctx: *mut ffi::AVCodecContext,
    formats: *const ffi::AVPixelFormat,
) -> ffi::AVPixelFormat {
    // SAFETY: FFmpeg passes its context and a list that ends with AV_PIX_FMT_NONE.
    unsafe {
        let wanted = (*ctx).opaque as isize as i32;
        let mut p = formats;
        while !p.is_null() && *p != ffi::AVPixelFormat::AV_PIX_FMT_NONE {
            if *p as i32 == wanted {
                return *p;
            }
            p = p.add(1);
        }
        ffi::avcodec_default_get_format(ctx, formats)
    }
}

/// Sets up a decoder that is not open yet to decode on the graphics card, when hardware decoding
/// is on, the codec is one of [`CODECS`] and a device opens. Returns whether it did; the decoder
/// still falls back to the processor by itself if the card turns the video down.
pub(crate) fn attach(ctx: &mut ff::codec::context::Context) -> bool {
    if !hardware_decoding() || !CODECS.contains(&ctx.id()) {
        return false;
    }
    let Some(codec) = ff::decoder::find(ctx.id()) else {
        return false;
    };
    let Ok(mut shared) = DEVICE.lock() else {
        return false;
    };
    let device = shared.get_or_insert_with(|| {
        let found = device_kinds()
            .iter()
            .find_map(|kind| create_device(*kind).map(|d| (*kind, d)));
        match &found {
            Some((kind, _)) => tracing::info!("hardware decoding with {kind:?}"),
            None => tracing::info!("no hardware decoder: decoding on the processor"),
        }
        found
    });
    let Some((kind, device)) = device.as_ref() else {
        return false;
    };
    let Some(format) = hardware_format(&codec, *kind) else {
        return false;
    };
    // SAFETY: the context is not open; it takes the new device reference and frees it with
    // itself, and `opaque` is otherwise unused (ffmpeg-next never sets it).
    unsafe {
        let raw = ctx.as_mut_ptr();
        (*raw).hw_device_ctx = device.new_ref();
        (*raw).opaque = format as i32 as isize as *mut c_void;
        (*raw).get_format = Some(pick_format);
    }
    true
}

/// Whether a decoded picture is still on the graphics card.
pub(crate) fn on_card(frame: &ff::frame::Video) -> bool {
    // SAFETY: reading a field of a frame we hold.
    unsafe { !(*frame.as_ptr()).hw_frames_ctx.is_null() }
}

/// Copies a picture from the graphics card into memory (NV12 or P010), with its time.
pub(crate) fn download(frame: &ff::frame::Video) -> Result<ff::frame::Video, ff::Error> {
    let mut out = ff::frame::Video::empty();
    // SAFETY: both frames are allocated; FFmpeg allocates `out`'s buffers.
    unsafe {
        let r = ffi::av_hwframe_transfer_data(out.as_mut_ptr(), frame.as_ptr(), 0);
        if r < 0 {
            return Err(ff::Error::from(r));
        }
        let r = ffi::av_frame_copy_props(out.as_mut_ptr(), frame.as_ptr());
        if r < 0 {
            return Err(ff::Error::from(r));
        }
    }
    Ok(out)
}

/// A pool of pictures on the graphics card for a VA-API encoder (Linux), which takes pictures only
/// from the card.
pub struct VaapiFrames {
    frames: BufferRef,
}

impl VaapiFrames {
    /// A pool of `width` x `height` pictures that hold `layout` (NV12, or P010 for 10 bits) on the
    /// default card.
    pub fn new(width: u32, height: u32, layout: Pixel) -> Result<Self, String> {
        let device = create_device(ffi::AVHWDeviceType::AV_HWDEVICE_TYPE_VAAPI)
            .ok_or("no VA-API device: is there a graphics driver with VA-API?")?;
        // SAFETY: the pool takes its own reference to the device; the context fields are set
        // before it is initialised, as FFmpeg asks.
        unsafe {
            let raw = ffi::av_hwframe_ctx_alloc(device.0);
            if raw.is_null() {
                return Err("could not make a VA-API picture pool".into());
            }
            let frames = BufferRef(raw);
            let ctx = (*raw).data as *mut ffi::AVHWFramesContext;
            (*ctx).format = ffi::AVPixelFormat::AV_PIX_FMT_VAAPI;
            (*ctx).sw_format = layout.into();
            (*ctx).width = width as i32;
            (*ctx).height = height as i32;
            (*ctx).initial_pool_size = 20;
            let r = ffi::av_hwframe_ctx_init(raw);
            if r < 0 {
                return Err(format!(
                    "the VA-API picture pool did not start: {}",
                    ff::Error::from(r)
                ));
            }
            Ok(Self { frames })
        }
    }

    /// Gives an encoder that is not open yet the pool its pictures come from.
    pub fn attach(&self, encoder: &mut ff::codec::context::Context) {
        // SAFETY: the encoder is not open and takes its own reference.
        unsafe { (*encoder.as_mut_ptr()).hw_frames_ctx = self.frames.new_ref() }
    }

    /// Copies a picture in memory to the card, with its time.
    pub fn upload(&self, picture: &ff::frame::Video) -> Result<ff::frame::Video, ff::Error> {
        let mut out = ff::frame::Video::empty();
        // SAFETY: `out` is allocated and takes a picture from the pool; `picture` is in the
        // pool's layout and size.
        unsafe {
            let r = ffi::av_hwframe_get_buffer(self.frames.0, out.as_mut_ptr(), 0);
            if r < 0 {
                return Err(ff::Error::from(r));
            }
            let r = ffi::av_hwframe_transfer_data(out.as_mut_ptr(), picture.as_ptr(), 0);
            if r < 0 {
                return Err(ff::Error::from(r));
            }
            let r = ffi::av_frame_copy_props(out.as_mut_ptr(), picture.as_ptr());
            if r < 0 {
                return Err(ff::Error::from(r));
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_switch_and_the_environment_decide() {
        set_hardware_decoding(false);
        assert!(!hardware_decoding());
        set_hardware_decoding(true);
        assert_eq!(
            hardware_decoding(),
            std::env::var_os("PICASSO_NO_HWACCEL").is_none()
        );
    }

    #[test]
    fn only_long_gop_codecs_use_the_card() {
        crate::init();
        let Some(codec) = ff::decoder::find(ff::codec::Id::PRORES) else {
            return;
        };
        let mut ctx = ff::codec::context::Context::new_with_codec(codec);
        assert!(!attach(&mut ctx), "ProRes stays on the processor");
    }
}
