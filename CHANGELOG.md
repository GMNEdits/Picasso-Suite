# Changelog

## 0.1.0 hotfix

### Fixed
- [x] UI stalls while the About window was open: it started `ffprobe` and `ffmpeg` on every frame (about 115 ms each). The FFmpeg version is now read once at startup.
- [x] The window title was sent to the operating system on every frame; now only when it changes.
- [x] Flatpak: building FFmpeg from source inside the sandbox kept failing; the Flatpak now ships a prebuilt static FFmpeg 7 (GPL, with x264/x265), checked against its published MD5.

### Added
- [x] **Rebindable keyboard shortcuts** (Help → Keyboard Shortcuts): click a key and press the new combination. If another command already uses it you get a warning and can replace it or cancel. Reset per command or all; saved to `keybinds.json` in the user's config folder.
- [x] `PICASSO_PERF=1` prints frame times and closes, for checking performance.

### Docs
- [x] README: install table per operating system, build-from-source section, screenshots.
- [x] Roadmap grouped by phases; changelog with checkboxes.

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
