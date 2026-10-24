//! Evaluates a timeline at a frame: video tracks are composited bottom to top
//! with each clip's transform (fit, zoom, pan, opacity) and alpha.

use picasso_core::{Clip, MediaItem, Project, TrackKind};
use picasso_media::RgbaImage;
use rayon::prelude::*;
use std::sync::Arc;

/// Supplies decoded source frames at a requested size.
pub trait FrameSource {
    fn video_frame(&mut self, media: &MediaItem, seconds: f64, width: u32, height: u32) -> Option<Arc<RgbaImage>>;
}

/// Where a clip lands in an output frame of `out_w`×`out_h`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub width: u32,
    pub height: u32,
    pub x: i64,
    pub y: i64,
}

/// Fits the source inside the project frame (preserving aspect), then applies
/// zoom and pan. Pan is in project pixels and scaled to the output size.
pub fn placement(project: &Project, clip: &Clip, media: &MediaItem, out_w: u32, out_h: u32) -> Option<Placement> {
    let v = media.info.video.as_ref()?;
    let fit = (out_w as f64 / v.width as f64).min(out_h as f64 / v.height as f64);
    let scale = fit * clip.transform.zoom.max(0.01) as f64;
    // Even dimensions keep FFmpeg's chroma-subsampled scalers happy.
    let even = |x: f64| ((x.round() as u32).max(2) + 1) & !1;
    let width = even(v.width as f64 * scale).min(16384);
    let height = even(v.height as f64 * scale).min(16384);
    let px = out_w as f64 / project.settings.width.max(1) as f64;
    let x = ((out_w as f64 - width as f64) / 2.0 + clip.transform.pan_x as f64 * px).round() as i64;
    let y = ((out_h as f64 - height as f64) / 2.0 - clip.transform.pan_y as f64 * px).round() as i64;
    Some(Placement { width, height, x, y })
}

/// Renders timeline `frame` at `out_w`×`out_h`. Areas with no picture are opaque black.
pub fn render_frame(project: &Project, frame: i64, out_w: u32, out_h: u32, source: &mut dyn FrameSource) -> RgbaImage {
    let mut out = RgbaImage::filled(out_w, out_h, [0, 0, 0, 255]);
    for track in project.timeline.tracks.iter().filter(|t| t.kind == TrackKind::Video && t.enabled) {
        let Some(clip) = track.clip_at(frame).filter(|c| c.enabled) else { continue };
        let Some(media) = project.media(clip.media_id) else { continue };
        let Some(place) = placement(project, clip, media, out_w, out_h) else { continue };
        let seconds = project.source_seconds(clip, frame);
        if let Some(img) = source.video_frame(media, seconds, place.width, place.height) {
            blend_over(&mut out, &img, place.x, place.y, clip.transform.opacity.clamp(0.0, 1.0));
        }
    }
    out
}

/// Straight-alpha "over" of `src` onto `dst` at (`x`, `y`).
pub fn blend_over(dst: &mut RgbaImage, src: &RgbaImage, x: i64, y: i64, opacity: f32) {
    if opacity <= 0.0 {
        return;
    }
    let (dw, dh) = (dst.width as i64, dst.height as i64);
    let x0 = x.max(0);
    let x1 = (x + src.width as i64).min(dw);
    let y0 = y.max(0);
    let y1 = (y + src.height as i64).min(dh);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let row_bytes = dw as usize * 4;
    let op = (opacity * 255.0).round() as u32;
    dst.data
        .par_chunks_mut(row_bytes)
        .enumerate()
        .skip(y0 as usize)
        .take((y1 - y0) as usize)
        .for_each(|(dy, row)| {
            let sy = (dy as i64 - y) as usize;
            let src_row = &src.data[sy * src.width as usize * 4..][..src.width as usize * 4];
            for dx in x0..x1 {
                let sx = (dx - x) as usize;
                let s = &src_row[sx * 4..sx * 4 + 4];
                let a = s[3] as u32 * op / 255;
                if a == 0 {
                    continue;
                }
                let d = &mut row[dx as usize * 4..dx as usize * 4 + 4];
                if a == 255 {
                    d.copy_from_slice(&[s[0], s[1], s[2], 255]);
                    continue;
                }
                let inv = 255 - a;
                let da = d[3] as u32;
                let out_a = a + da * inv / 255;
                for c in 0..3 {
                    let v = (s[c] as u32 * a + d[c] as u32 * da * inv / 255) / out_a.max(1);
                    d[c] = v.min(255) as u8;
                }
                d[3] = out_a.min(255) as u8;
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use picasso_core::*;

    struct Solid([u8; 4]);
    impl FrameSource for Solid {
        fn video_frame(&mut self, _: &MediaItem, _: f64, w: u32, h: u32) -> Option<Arc<RgbaImage>> {
            Some(Arc::new(RgbaImage::filled(w, h, self.0)))
        }
    }

    fn project(src_w: u32, src_h: u32) -> (Project, Id) {
        let mut p = Project::new("t", ProjectSettings { width: 160, height: 90, ..Default::default() });
        let m = MediaItem {
            id: new_id(),
            name: "m".into(),
            path: "m.mp4".into(),
            sequence: None,
            info: MediaInfo {
                format_name: "mov".into(),
                duration_secs: 4.0,
                video: Some(VideoInfo {
                    codec: "h264".into(),
                    width: src_w,
                    height: src_h,
                    fps: Rational::new(25, 1),
                    pix_fmt: "yuv420p".into(),
                    has_alpha: false,
                    is_still: false,
                }),
                audio: None,
            },
        };
        let id = m.id;
        p.media.push(m);
        p.append_media(id).unwrap();
        (p, id)
    }

    #[test]
    fn pillarboxes_4x3_into_16x9() {
        let (p, _) = project(640, 480);
        let img = render_frame(&p, 0, 160, 90, &mut Solid([200, 10, 10, 255]));
        assert_eq!(img.pixel(80, 45), [200, 10, 10, 255]);
        assert_eq!(img.pixel(5, 45), [0, 0, 0, 255]);
        // Past the end of the clip is black.
        assert_eq!(render_frame(&p, 500, 160, 90, &mut Solid([1, 2, 3, 255])).pixel(80, 45), [0, 0, 0, 255]);
    }

    #[test]
    fn opacity_blends_and_pan_moves() {
        let (mut p, _) = project(1920, 1080);
        let id = p.timeline.tracks[0].clips[0].id;
        let c = p.timeline.clip_mut(id).unwrap();
        c.transform.opacity = 0.5;
        c.transform.zoom = 0.5;
        // Project is 160 wide, so pan is in output pixels; the 80 px image spans 60..140.
        c.transform.pan_x = 20.0;
        let img = render_frame(&p, 0, 160, 90, &mut Solid([255, 255, 255, 255]));
        let inside = img.pixel(138, 45);
        assert!((126..=129).contains(&inside[0]), "{inside:?}");
        assert_eq!(img.pixel(58, 45), [0, 0, 0, 255]);
        assert_eq!(img.pixel(141, 45), [0, 0, 0, 255]);
    }
}
