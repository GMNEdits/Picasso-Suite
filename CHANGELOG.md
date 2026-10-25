# Changelog

## 0.1.0 — first release

### Application
- [x] Native Rust application (egui/eframe, wgpu) with a borderless window and DaVinci Resolve-style
  chrome: menu bar, interface toolbar, page bar, and a design-grid scaler that keeps every
  proportion fixed at any window size.
- [x] **Cut page** (default): media pool, viewer with dB meter, transport row with TCG timecode,
  overview timeline with draggable playhead and 10-second ruler labels, zoomable detail
  timeline with a fixed centre playhead and drag-to-scrub.
- [x] **Edit page** and **Deliver page** with a reusable timeline widget.
- [x] One command table drives menus, shortcut labels and key handling (a test asserts no key is
  bound twice).
- [x] Original vector icon set drawn in code; Noto Sans bundled (SIL OFL).

### Editing
- [x] Multi-track video and audio timeline; place/overwrite, blade, delete and ripple delete, move,
  trim, snapping, linked picture/sound groups.
- [x] Per-clip zoom, position, opacity and volume.
- [x] Snapshot-based undo/redo.
- [x] Timelines start at 01:00:00:00, like Resolve.

### Media (via FFmpeg, on every OS)
- [x] Probe and frame-accurate decode of every format FFmpeg reads: H.264, H.265/HEVC, ProRes,
  DNxHR, AV1, VP9, MPEG-2, image sequences (EXR, DPX, TIFF, PNG…), stills, and audio formats.
- [x] Thumbnails and waveforms for the timeline.
- [x] Float32 compositor with a decoder pool.

### Export
- [x] H.264 and H.265 (8- and 10-bit) with x264/x265 and hardware encoders: NVENC, Quick Sync,
  AMF, VA-API (Linux), VideoToolbox (macOS), whichever your FFmpeg build provides.
- [x] MP4/MOV/MKV containers; ProRes, DNxHR, AV1, VP9, FFV1, PNG sequences, WAV, MP3.
- [x] Audio mixed across segments with an `amix` graph.

### Projects and interchange
- [x] `.pcsp` project files (JSON).
- [x] **`.drp` import**: timeline, tracks, clips, media paths, resolution and frame rate; verified
  against the shipped `Fuse.drp`. Media paths from other operating systems are relinked.
- [x] OpenTimelineIO export.

### Command line
- [x] `picasso`: `probe`, `presets`, `new`, `import-drp`, `otio`, `render`.

### Packaging
- [x] Windows zip, Linux `.deb`, `.rpm` and Flatpak, macOS universal (Intel + Apple silicon) app,
  all built by the release workflow.

### Known limitations
- [ ] No real-time audio playback yet.
- [ ] Edit and Deliver pages are not yet restyled to match the Cut page.
- [ ] FFmpeg (`ffmpeg`/`ffprobe`) must be installed, except in the Flatpak, which bundles it.
- [ ] Only plain-XML parts of `.drp` files are imported.
- [ ] Not affiliated with Blackmagic Design.
