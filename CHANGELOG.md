# Changelog

## 0.1.0 — first release

### Application
- Native Rust application (egui/eframe, wgpu) with a borderless window and DaVinci Resolve-style
  chrome: menu bar, interface toolbar, page bar, and a design-grid scaler that keeps every
  proportion fixed at any window size.
- **Cut page** (default): media pool, viewer with dB meter, transport row with TCG timecode,
  overview timeline with draggable playhead and 10-second ruler labels, zoomable detail
  timeline with a fixed centre playhead and drag-to-scrub.
- **Edit page** and **Deliver page** with a reusable timeline widget.
- One command table drives menus, shortcut labels and key handling (a test asserts no key is
  bound twice).
- Original vector icon set drawn in code; Noto Sans bundled (SIL OFL).

### Editing
- Multi-track video and audio timeline; place/overwrite, blade, delete and ripple delete, move,
  trim, snapping, linked picture/sound groups.
- Per-clip zoom, position, opacity and volume.
- Snapshot-based undo/redo.
- Timelines start at 01:00:00:00, like Resolve.

### Media (via FFmpeg, on every OS)
- Probe and frame-accurate decode of every format FFmpeg reads: H.264, H.265/HEVC, ProRes,
  DNxHR, AV1, VP9, MPEG-2, image sequences (EXR, DPX, TIFF, PNG…), stills, and audio formats.
- Thumbnails and waveforms for the timeline.
- Float32 compositor with a decoder pool.

### Export
- H.264 and H.265 (8- and 10-bit) with x264/x265 and hardware encoders: NVENC, Quick Sync,
  AMF, VA-API (Linux), VideoToolbox (macOS), whichever your FFmpeg build provides.
- MP4/MOV/MKV containers; ProRes, DNxHR, AV1, VP9, FFV1, PNG sequences, WAV, MP3.
- Audio mixed across segments with an `amix` graph.

### Projects and interchange
- `.pcsp` project files (JSON).
- **`.drp` import**: timeline, tracks, clips, media paths, resolution and frame rate; verified
  against the shipped `Fuse.drp`. Media paths from other operating systems are relinked.
- OpenTimelineIO export.

### Command line
- `picasso`: `probe`, `presets`, `new`, `import-drp`, `otio`, `render`.

### Packaging
- Windows zip, Linux `.deb`, `.rpm` and Flatpak, macOS universal (Intel + Apple silicon) app,
  all built by the release workflow.

### Known limitations
- No real-time audio playback yet.
- Edit and Deliver pages are not yet restyled to match the Cut page.
- FFmpeg (`ffmpeg`/`ffprobe`) must be installed, except in the Flatpak, which bundles it.
- Only plain-XML parts of `.drp` files are imported.
- Not affiliated with Blackmagic Design.
