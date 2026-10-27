# Changelog

## 0.4.0

Picasso Suite 0.4.0 makes playback smoother and steadier: the viewer's picture is put together on a thread of its own, scrubbing camera video is faster, the timelines move at the right speed at every frame rate, and the Edit page's source viewer no longer blinks. It adds saved layouts, working track buttons and height arrows on the Cut page, a mini timeline on the Color page and a Hardware Decoding switch in the Playback menu, with security fixes for project and Resolve files. Packages for Windows, macOS and Linux (x86_64 and ARM64).

### ✨ New
- ✅ **Playback > Hardware Decoding:** turns decoding on the graphics card off (or on) straight from the Playback menu, the same switch as System Preferences > Decode Options. Pictures already made with the card are forgotten, so the change shows at once. If playback shakes, blinks or goes black on your computer, try it off and tell us your graphics card.
- ✅ **Smoother playback:** the viewer's picture is put together on a thread of its own, so the window no longer waits for it. Measured with a 1080p clip playing, the interface's work per frame went from 3.0 to 1.35 ms on the Edit page, 2.2 to 0.66 ms on Cut and 3.9 to 2.3 ms on Color on average; the medians are now 0.6 to 0.9 ms, from 2.2 to 2.6 ms.
- ✅ **Faster scrubbing on long-GOP video** (camera H.264/H.265): a picture being decoded is dropped as soon as the playhead moves on, and the key frame it starts from is shown while the rest is decoded and kept for the next pass. Dragging across 4K video with a key frame every 2 seconds, the exact picture arrives about 200 ms after letting go instead of 370 ms (worst case 270 ms instead of 570 ms).
- ✅ **Cut page:** the height arrows beside the tracks make the picture rows taller or shorter; the height is kept with the panel sizes, in saved layouts too, and Reset UI Layout puts it back.
- ✅ **Color page:** the mini timeline under the clips shows each clip where it sits on up to three picture tracks, with the clip being graded outlined, instead of one solid bar.
- ✅ **Development:** `--play` starts playback once the files given with `--place` are on the timeline (answering the frame-rate question), so `PICASSO_PERF=1` can time playback.
- ✅ **Saved layouts:** Workspace › Layout › Save Layout keeps the sizes of the panels on every page under a name (`layouts/<name>.txt` in the data folder, readable and editable by hand); Load Layout puts one back. Thanks to GANESH NIKAM for the pull request.
- ✅ **Cut page track buttons:** the button row beside the tracks now works. Lock, mute, **solo** and **picture** act on the track of the selected clip (V1 when nothing is selected), undo, and are saved with the project. Solo silences every track that is not soloed, in playback, the mixdown and compound clips; picture off keeps a video track out of the program, the Color and Deliver previews and the subtitles. Older projects open with every track unsoloed and its picture on. Thanks to GANESH NIKAM.

### 🛠️ Fixed
- ✅ **Viewer:** a picture still being put together when a newer one was shown from the render cache no longer replaces it when it arrives (playing a stretch a second time could jump back a frame). It is still kept for later.
- ✅ **DaVinci Resolve files:** one still being read when another project is opened no longer replaces that project when it arrives.
- ✅ **Playback with many stills:** the still images are shared with the viewer's thread instead of copied for every picture it puts together.
- ✅ **Memory:** the key frames kept for scrubbing take at most 32 MB per video (96 before), so a multicam of nine angles stays under 300 MB for them.
- ✅ **Edit page source viewer:** it no longer blinks black while the timeline plays the same clip, and playback no longer stutters for it. Both viewers asked one decoder for the clip, which then sought back and forth between their two moments, and the source viewer drew itself again on every frame. It now has a decoder of its own and draws again only when its own picture changes (Edit page with a 1080p clip: 1.04 to 0.49 ms of work a frame, the slowest frames 3.3 to 0.7 ms). Confirmed on the computer where the choppy, blinking playback was seen.
- ✅ **Long clip names:** the source viewer's header cuts a long name with “…” where it would run over the length beside it.
- ✅ **Timelines at 60 fps:** a second was two and a half times wider in a 60 fps project than in a 24 fps one (600 points on the Cut page's lower timeline instead of 240), so the playhead raced across while the picture and the timecode played at the right speed. The timelines now keep the width of a second at every frame rate.
- ✅ **Cut page:** the lower timeline keeps the playhead in the middle while playing, even after a drag of the playhead whose end it missed (it used to stay still and let the playhead run across).
- ✅ **Long clips:** the film strip and the waveform are drawn only where they are on screen; a 12-minute clip queued over a thousand film-strip tiles and spread its waveform along its whole length on every frame. The waveform is also sharper: one line per point.
- ✅ **Check for Updates on openSUSE:** the new .rpm is installed with `--allow-unsigned-rpm`; zypper refused the release's unsigned package before.
- ✅ **Check for Updates as root:** the .deb or .rpm is installed directly, without `pkexec`, which asked for a password that root does not need (and fails where there is no polkit).
- ✅ **Security:** a project file (`.picasso`) that takes more than 5 seconds to read is refused, so a file built to run a query for ever cannot freeze the app; at most 100,000 media entries are read.
- ✅ **DaVinci Resolve files** (`.drp`, `.drt`, `.drb`, XML) are read on a thread of their own, and the media they list is imported in the background, so a big project no longer freezes the window while it opens.
- ✅ **Timecode:** the readouts on the Cut (TCG), Color and Deliver pages started at 00:00:00:00 while the rulers and the Edit page start at 01:00:00:00; they now agree. Thanks to GANESH NIKAM.
- ✅ **Photo tab:** the second page tab said "Media" like the first; it now says **Photo**. `--page=photo`, `--page=fusion` and `--page=fairlight` open their pages (they were ignored); the old names still work. Thanks to GANESH NIKAM.
- ✅ **Reset UI Layout** now puts the panel dividers back too, not only the floating windows.
- ✅ A damaged layout file is refused with a message instead of loading as the default sizes.
- ✅ A layout is found by its file, so a file renamed by hand still loads; names that make the same file name (`My Layout`, `my-layout`) no longer replace each other; names like `con` or `nul` are not written to Windows devices.
- ✅ The Save and Load Layout windows read the layouts once when they open, not on every frame, and Enter with no name no longer closes Save Layout.

### 🔄 Changed
- ✅ **Releases:** the release workflow now tries Check for Updates where each package really runs (its first run is still to come): the .deb (Ubuntu 22.04 and 24.04, Debian 12) and .rpm (Fedora, openSUSE) are updated to a copy marked newer and the package manager must have it, the macOS app and the Windows folder are replaced with the old copy set aside, and the Flatpak is reinstalled through the host's flatpak, on x86_64 and ARM64.
- ✅ **Development:** `picasso-suite --update-from <package>` installs a release package over this copy with the updater's own code (the download is the only step left out), and `picasso-suite --version` prints the version and, for release builds, the commit. `PICASSO_PERF=1` also gives the time between frames and how many took over 50 ms (`PICASSO_PERF_FRAMES` sets how many frames are timed).
- ✅ **Development:** CI fails when `docs/status.svg` or the README's Status numbers no longer match `Menu-Status.md` and `ROADMAP.md` (run `python3 scripts/status.py`). Thanks to GANESH NIKAM.

### ⚠️ Known limitations
- ❌ Scrubbing video with very few key frames (one every 20 seconds of 4K) still takes seconds for the exact picture; that needs optimised media (proxies).
- ❌ Check for Updates is tested by the release workflow on every package from this release on; before, it was only tried on Windows.
- ❌ Not tried on real NVIDIA, Intel, Apple or ARM graphics cards, nor on a Raspberry Pi; the Color page's scopes still measure on the interface thread.
- ❌ Native Wayland windows take no dropped files (winit 0.30), and the Flathub submission is still to be made.

## 0.3.0

Picasso Suite 0.3.0 adds Linux on ARM64 (.deb, .rpm, AppImage and Flatpak), an AppImage for
x86_64, panels you can resize on every page, a graphics check, and a faster window when
switching pages and importing media. Packages for Windows, macOS and Linux (x86_64 and ARM64).

**✨ New**
- ✅ **Linux on ARM64:** .deb, .rpm and AppImage for 64-bit ARM computers (Raspberry Pi 5, ARM laptops and servers)
- ✅ **Linux Flatpak on ARM64:** `picasso-suite-X-aarch64.flatpak` beside the x86_64 one; Help > Check for Updates installs the one for the computer's processor
- ✅ **Graphics check:** `picasso-suite --graphics-check` lists the graphics cards and interfaces (Vulkan, DirectX 12, Metal, OpenGL), tells which ones draw a test picture and saves the report as `picasso-graphics-report.txt`; there is a "Graphics report" issue form for sending it in
- ✅ **AppImage:** one file that runs on most Linux distributions without installing; Help > Check for Updates replaces it with the new version
- ✅ **Cut page:** files dragged over the window show a drop target on the media pool, and 3GP, 3G2, F4V, VOB, DV, ASF and M2TS video can be imported
- ✅ **Resizable panels:** on every page, drag the dividers between the panels to resize them (Edit: bins, media pool, viewers, timeline; Cut: pool, viewer, timelines; Media: files, queue, viewer, storage, meters, upper and lower rows; Fusion: viewers, node area, inspector; Color: gallery, viewer, nodes, clips, scopes; Fairlight: meters, video, console, track headers, mixer; Deliver: settings, viewer, queue, timeline; Photo: pool, viewer, inspector, album). The divider lights up under the pointer, a double click puts it back, and the sizes are kept for the next start. Panels keep at least two fifths of their usual size; the text, icons and buttons in a narrowed panel get smaller with it instead of running into the next one (toolbars, track headers, transport, empty-panel messages), and the Deliver settings form scrolls sideways in a narrow column. The color wheels keep their size, so the palettes are not resizable
- ✅ **Cut page:** the empty timeline before a track's first clip is grey, as in Resolve, so a clip that starts late is easy to see

**🛠️ Fixed**
- ✅ **Linux:** dragging video files from the file manager into the media pool works on Wayland desktops (Ubuntu's default); the window opens through XWayland, and `PICASSO_WAYLAND=1` keeps a native Wayland window
- ✅ **Linux:** the dock shows Picasso Suite's icon and groups the window with its launcher
- ✅ **Linux:** a per-user Flatpak installs its update per-user, and the command shown when that fails says `--user`
- ✅ **Linux:** programs started from Picasso Suite (the browser, the installer) still see the Wayland desktop; the window asks for X11 itself instead of hiding the Wayland display from everything
- ✅ **Check for Updates:** a download that breaks off leaves no half-file behind
- ✅ **Media pool:** video thumbnails are made without holding up the window while the first frame decodes, by their own decoder, so the viewer no longer leaves them (and the clips on the timeline) black; they are decoded at a quarter of the size and the decoder is closed once the picture is made, so a pool of 4K clips does not keep a full picture and a thread for each
- ✅ **Speed:** importing videos and sound no longer holds up the window (twelve clips took half a second; their files are now read in the background, so no frame took more than 1 ms), opening the Color page with many clips no longer freezes for a moment (it read every video's codec before drawing the first frame), the Deliver page no longer copies the whole project on every frame, and the Media page's file browser and media pool draw only the clips in view, so big projects and folders with thousands of files stay smooth when switching pages
- ✅ **Timeline:** a disabled clip no longer shows through a transition (dissolve or dip) next to it
- ✅ **Timeline:** dragging a clip, trim or roll no longer copies the whole project on every pointer move
- ✅ **Stills:** shrinking a big picture uses less memory, and an image too big to show is refused from its header before it is decoded
- ✅ **Projects:** the pictures kept in a project (stills) load faster
- ✅ **Video:** a frame with no time of its own is placed by its best-effort time instead of at the start of the file

**🔄 Changed**
- ✅ **Linux packages:** `.picasso`, `.drp`, `.drt` and `.drb` files open with Picasso Suite, and video files offer it under "Open With", several at once
- ✅ **Linux packages:** the .deb and .rpm name the window libraries they need, so a minimal system gets them; the .rpm names them in a way both Fedora and openSUSE understand
- ✅ **Flatpak:** footage on USB drives and memory cards (`/media`, `/run/media`) can be imported
- ✅ **Linux:** icons in 16 to 256 pixels, so menus and docks show a sharp icon at every size
- ✅ **Releases:** a release is only published when every package built and the Linux ones installed and started on seven systems, so a failed build can no longer leave a release with fewer packages
- ✅ **Flathub:** a manifest that builds FFmpeg and its codecs from source (`packaging/flathub`), built in CI on x86_64 and ARM64 and checked with Flathub's linter, with notes for the submission, which is not made yet; the screenshots and links in the app's appstream entry are filled in
- ✅ **Releases:** a release candidate (a tag such as `v0.3.0-rc1`) is published as a pre-release for testing; Check for Updates does not offer it
- ✅ **Development:** the tests run on every pull request, and a test checks `Menu-Status.md` against the menus' table
- ✅ **Development:** `PICASSO_PERF=pages picasso-suite [--place] <files>` shows each page in turn and prints how long its frames take, and files given with `--place` now go one after another on the timeline instead of all at the playhead
- ✅ **Status in the README:** a picture and a table of how much is made (menu entries that work, roadmap items done), drawn from `Menu-Status.md` and `ROADMAP.md` by `python3 scripts/status.py`

**⚠️ Known limitations**
- ❌ A native Wayland window takes no dropped files (winit 0.30 has no support and eframe 0.37 uses it), so the window opens through XWayland
- ❌ The ARM64 and Flatpak packages are only started on GitHub's machines (containers with a virtual screen and a software Vulkan), not on real ARM or Linux desktops
- ❌ Vulkan and the other graphics interfaces were only tried on an AMD card; run `picasso-suite --graphics-check` and send the report in
- ❌ Nothing has been tried on a real Raspberry Pi or other ARM computer yet; if you have one, run `picasso-suite --graphics-check` and send the report in
- ❌ The Color page's palettes (the wheels) keep their size, so they cannot be resized, and the Deliver settings form scrolls sideways in a narrow column instead of shrinking
- ❌ The Flathub submission is not made yet

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
