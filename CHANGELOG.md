# Changelog

## 0.2.1

Picasso Suite 0.2.1 is the full editor: all eight pages (Media, Photo, Cut, Edit, Fusion, Color,
Fairlight and Deliver), with FFmpeg included in every package.

**✨ New**
- ✅ **Curve editing:** on the Color page, add, drag and remove points on the Y, R, G and B curves to grade the picture
- ✅ **Curve intensity:** each curve's strength can be set from 0 to 100
- ✅ **Soft Clip:** clip and soften black (Low, L.S.) and white (High, H.S.)
- ✅ **Curves are saved** with each node, and a whole drag is one undo step
- ✅ **Check for Updates:** Help > Check for Updates finds the latest release and installs it on Windows, macOS, Flatpak, .deb and .rpm
- ✅ **Safe Windows updates:** if an update fails partway, your version is restored
- ✅ **More codecs on the Deliver page:** AV1 (MP4), VP9 (WebM), Avid DNxHR LB, SQ, HQ, HQX and 444 (QuickTime), lossless FFV1 with FLAC sound (Matroska) and MP3 audio
- ✅ **10-bit video:** H.264 High 10, H.265 Main 10, AV1 and VP9 profile 2, from the Encoding Profile list
- ✅ **Graphics card encoders:** Encoder can be NVIDIA, Intel Quick Sync, AMD, Apple VideoToolbox or VA-API for H.264, H.265 and AV1; the ones this computer has no card for are greyed out
- ✅ **Hardware decoding:** H.264, H.265, AV1, VP9, MPEG-2 and VC-1 are decoded on the graphics card (D3D11 on Windows, VideoToolbox on macOS, VA-API or CUDA on Linux), with a switch in System Preferences > Decode Options; a clip the card can't decode falls back to the processor
- ✅ **Keyboard Customization:** click any command's keys and press a new one; a key another command has is moved after asking; Reset and Reset All
- ✅ **OpenTimelineIO export:** File > Export > Timeline: OpenTimelineIO, for Resolve, Premiere, Avid and others
- ✅ System Preferences and your keys are saved, and stay after a restart

**🔄 Changed**
- ✅ Renders start at **60 fps** by default
- ✅ The camera tab on the page bar is now called **Media**

**🛠️ Fixed**
- 🛠️ Media and Deliver no longer flash black when you open them
- 🛠️ Dragging a clip from the media pool onto the Cut page timeline works again
- 🛠️ Dragging the playhead no longer makes the viewer flash black

**⚡ Performance**
- 🛠️ Playback is about **2x lighter**: Cut 4.8 → 2.4 ms, Edit 6.6 → 3.1 ms, Color 8.6 → ~4 ms per frame
- 🛠️ Videos are decoded at the viewer's size, so pictures show faster
- 🛠️ The Color page scopes update 20 times a second and no longer slow down playback
- 🛠️ Color grading is up to **14x faster** (wheels + curves: 6.5 → 0.45 ms per picture)

**⚠️ Known limitations**
- ❌ The Hue vs Hue, Hue vs Sat, Hue vs Lum, Lum vs Sat, Sat vs Sat and Sat vs Lum curves aren't done yet
- ❌ Check for Updates has only been tested on Windows so far
- ❌ The graphics card encoders were only tried with an AMD card on Windows; NVIDIA, Intel, Apple and VA-API are untested
- ❌ The cards make no 10-bit H.264 (it is offered with the Native encoder only)
- ❌ The macOS package is for Apple silicon only
- ❌ Projects from 0.2.0 and earlier (`.pcsp`) do not open in 0.2.1, which saves `.picasso` projects

## 0.2.0 hotfix

### New
- [x] **Preferences:** **File ▸ Preferences…** (`Ctrl/Cmd+,`) with a **Decode Options** section, arranged as in Resolve's Preferences ▸ System ▸ Decode Options. Contributed by @GMNEdits.
- [x] **Hardware-decode toggle:** turn hardware decoding on or off for the whole app, for when a GPU driver decodes a clip badly (green frames, corruption, media going offline). It was only possible with the `PICASSO_NO_HWACCEL` environment variable before.
- [x] The preview and source viewers restart their decoders when the toggle changes, so it applies without restarting the app. Export, thumbnails and overviews follow the setting when they are created.
- [x] The setting is saved to `preferences.json` in the per-user config folder next to `keybinds.json` (`%APPDATA%\PicassoSuite`, `~/Library/Application Support/PicassoSuite`, `~/.config/PicassoSuite`), and is not stored in the project.

### Changed
- [x] Hardware decoding stays on by default, as before; nothing is written until the toggle is used.
- [x] `PICASSO_NO_HWACCEL=1` still turns hardware decoding off for a single run, whatever the preference says.
- [x] The README documents the Preferences window, the `Ctrl/Cmd+,` shortcut and `preferences.json`.

### Thanks
- Thanks to @GMNEdits for the Preferences window and the hardware-decode toggle.

## 0.2.0

### New
- [x] **Sound during playback:** the timeline's audio plays in sync with the viewer at normal speed, with each clip's volume.
- [x] **Edit page:** the media pool, source and timeline viewers and track headers laid out as in DaVinci Resolve at 1080p.
- [x] **Color page:** Curves panel with a histogram that follows playback; wheels, Soft Clip and scopes arranged as in Resolve.
- [x] **Deliver page:** Custom Export preset list with icons and submenus; the render settings fit without scrolling.
- [x] **Frame-rate prompt:** when the first imported video has another frame rate, you choose whether to change the project's.
- [x] **Delete track:** right-click a track header to delete it (the last track of each kind stays).
- [x] New projects start with one picture track (V1) and one sound track (A1), as in Resolve.
- [x] Linux: the Flatpak may use the sound server (`--socket=pulseaudio`).

### Changed
- [x] Every page is laid out to Resolve's 1080p size; the layout scales with the window.
- [x] Selection arrow and loop icon redrawn to match Resolve.
- [x] Cut page timeline, overview and detail views as in Resolve.
- [x] Release builds use full link-time optimisation and drop debug symbols.
- [x] Page bar and top toolbars labelled as in Resolve.

### Fixed
- [x] The Selection tool's red arrow was misshapen; it is a proper cursor arrow now. The speaker icon too.
- [x] The 1080p pages keep a safe minimum body height, so a very short window cannot give them negative sizes.

### Removed
- [x] The unused `tracing` dependency of `picasso-media`.

### Known limitations
- [ ] Curve editing in the Color page's Curves panel is not yet possible (the histogram and controls are shown).
- [ ] Edit and Soft Clip effects are shown but do nothing yet.
- [ ] Deliver's interchange presets (Premiere, Avid) are listed but not built.
- [ ] Cut page track buttons are shown but do nothing yet.
- [ ] The Photo page still shows the Capture tools.
- [ ] Sound plays at normal speed only; fast-forward and reverse stay silent.
- [ ] macOS package does not include FFmpeg yet.

## 0.1.0 hotfix

### Fixed
- ✅ UI stalls while the About window was open: it started `ffprobe` and `ffmpeg` on every frame (about 115 ms each). The FFmpeg version is now read once at startup.
- ✅ The window title was sent to the operating system on every frame; now only when it changes.
- ✅ **Windows: FFmpeg is now included.** The zip has `ffmpeg.exe` and `ffprobe.exe` next to `picasso-suite.exe`, so the app works right after unzipping (FFmpeg 8.1, BtbN's GPL build, checked against its SHA-256).
- ✅ The "FFmpeg is missing" message now says that two programs are needed (`ffmpeg` and `ffprobe`), names the one that is missing, and says exactly where to put them on each system.
- ✅ Flatpak: building FFmpeg from source inside the sandbox kept failing; the Flatpak now ships a prebuilt static FFmpeg 8.1 (BtbN's GPL build, with x264/x265), checked against its published SHA-256.

### Added
- ✅ **Rebindable keyboard shortcuts** (Help → Keyboard Shortcuts): click a key and press the new combination. If another command already uses it you get a warning and can replace it or cancel. Reset per command or all; saved to `keybinds.json` in the user's config folder.
- ✅ `PICASSO_PERF=1` prints frame times and closes, for checking performance.

### Docs
- ✅ README: install table per operating system, build-from-source section, screenshots.
- ✅ Roadmap grouped by phases; changelog with checkboxes.

## 0.1.0 — first release

### Application
- ✅ Native Rust application (egui/eframe, wgpu) with a borderless window and DaVinci Resolve-style
  chrome: menu bar, interface toolbar, page bar, and a design-grid scaler that keeps every
  proportion fixed at any window size.
- ✅ **Cut page** (default): media pool, viewer with dB meter, transport row with TCG timecode,
  overview timeline with draggable playhead and 10-second ruler labels, zoomable detail
  timeline with a fixed centre playhead and drag-to-scrub.
- ✅ **Edit page** and **Deliver page** with a reusable timeline widget.
- ✅ One command table drives menus, shortcut labels and key handling (a test asserts no key is
  bound twice).
- ✅ Original vector icon set drawn in code; Noto Sans bundled (SIL OFL).

### Editing
- ✅ Multi-track video and audio timeline; place/overwrite, blade, delete and ripple delete, move,
  trim, snapping, linked picture/sound groups.
- ✅ Per-clip zoom, position, opacity and volume.
- ✅ Snapshot-based undo/redo.
- ✅ Timelines start at 01:00:00:00, like Resolve.

### Media (via FFmpeg, on every OS)
- ✅ Probe and frame-accurate decode of every format FFmpeg reads: H.264, H.265/HEVC, ProRes,
  DNxHR, AV1, VP9, MPEG-2, image sequences (EXR, DPX, TIFF, PNG…), stills, and audio formats.
- ✅ Thumbnails and waveforms for the timeline.
- ✅ Float32 compositor with a decoder pool.

### Export
- ✅ H.264 and H.265 (8- and 10-bit) with x264/x265 and hardware encoders: NVENC, Quick Sync,
  AMF, VA-API (Linux), VideoToolbox (macOS), whichever your FFmpeg build provides.
- ✅ MP4/MOV/MKV containers; ProRes, DNxHR, AV1, VP9, FFV1, PNG sequences, WAV, MP3.
- ✅ Audio mixed across segments with an `amix` graph.

### Projects and interchange
- ✅ `.pcsp` project files (JSON).
- ✅ **`.drp` import**: timeline, tracks, clips, media paths, resolution and frame rate; verified
  against the shipped `Fuse.drp`. Media paths from other operating systems are relinked.
- ✅ OpenTimelineIO export.

### Command line
- ✅ `picasso`: `probe`, `presets`, `new`, `import-drp`, `otio`, `render`.

### Packaging
- ✅ Windows zip, Linux `.deb`, `.rpm` and Flatpak, macOS universal (Intel + Apple silicon) app,
  all built by the release workflow.

### Known limitations
- ❌ No real-time audio playback yet.
- ❌ Edit and Deliver pages are not yet restyled to match the Cut page.
- ❌ FFmpeg (`ffmpeg`/`ffprobe`) must be installed, except in the Flatpak, which bundles it.
- ❌ Only plain-XML parts of `.drp` files are imported.
- ❌ Not affiliated with Blackmagic Design.
