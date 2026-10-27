# Roadmap

Status at 0.2.1, with the work since (see Unreleased in the changelog).

## Done

- [x] Editing: cut/trim/ripple/roll/slip/slide, Dynamic Trim, Loop Trim, Asymmetric Trimming, locks, clip colours, compound clips, multicam, speed changes and ramps, keyframes, effects, titles with Text+ looks
- [x] Media: bins, smart bins, folder import as a bin, sort and views, source viewer with match frame, EDL and subtitle import/export
- [x] Colour: serial, parallel, layer and outside nodes, qualifier and power windows, LUTs, gallery stills, scopes
- [x] Audio: per-track EQ and compressor, recording from the default microphone, voice-over, normalize, loudness (BS.1770), Bounce Mix to Track
- [x] Deliver: H.264, H.265, ProRes, WAV, PNG sequence, render queue, subtitles
- [x] Fixes: cut with linked clips, looped video showing the offline picture
- [x] Timeline: Delete Track from the track header's right-click menu; selection arrow and loop icons redrawn as in Resolve
- [x] Media: "Change Project Frame Rate?" when the first video of an empty project has another frame rate
- [x] Deliver: Render Settings preset list with Resolve's presets, icons and resolution submenus
- [x] Interface at 1080p: page bar with labels, labelled top toolbars on every page, Cut page, Color's lower half, Deliver (see the changelog)
- [x] Cut page: timeline with clips as in Resolve (blue overview bar, film strip and waveform at the bottom, red V1/A1, red playhead)
- [x] Color page: Curves - Custom panel with a live histogram and editable Y, R, G and B curves (points added, dragged and removed; intensity and soft clip act on the picture; saved per node), the lower half on Resolve's proportions, wheels redrawn
- [x] Security: media paths from project and Resolve files cannot open network shares or FFmpeg addresses; project databases are read with an untrusted schema; Resolve import is capped
- [x] Performance: measured at 0.2-0.4 ms of interface work per frame on every page when idle (`PICASSO_PERF`); while a 1080p clip plays, 2.4 ms on Cut, 3.1 ms on Edit and about 4 ms on Color (videos decoded at the viewer's size, scopes measured 20 times a second)
- [x] Deliver: renders start at 60 frames per second (Timeline Frame Rate and the other rates stay in the list)
- [x] Performance: grading of per-channel nodes (wheels, curves, soft clip) turned into 8-bit tables, up to 14 times faster
- [x] Help: Check for Updates reads the latest GitHub release and installs it for the Windows zip, macOS app, Flatpak, .deb and .rpm (a build from source opens the release page)
- [x] Fixes: Media and Deliver no longer flash black when opened; clips dropped anywhere on the Cut page's timelines land on a track; dragging the playhead no longer flashes black
- [x] Deliver: AV1 (MP4), VP9 (WebM), DNxHR LB to 444 (QuickTime), lossless FFV1 (Matroska) and MP3; 10-bit H.264, H.265, AV1 and VP9
- [x] Deliver: graphics card encoders (NVIDIA NVENC, Intel Quick Sync, AMD AMF, Apple VideoToolbox, VA-API) for H.264, H.265 and AV1, checked at start so those without a card are greyed out
- [x] Decoding on the graphics card (D3D11, VideoToolbox, VA-API, CUDA) with a System Preferences switch, falling back to the processor
- [x] Keyboard Customization: every command's keys can be changed, a taken key is moved after asking, saved in the preferences
- [x] Interchange: OpenTimelineIO export (File > Export > Timeline: OpenTimelineIO)
- [x] Linux: files dragged from the file manager reach the media pool on Wayland desktops (the window opens through XWayland); the window is grouped with its launcher and icon; the packages open project files and offer "Open With" for video; .deb and .rpm name the window libraries they need; the Flatpak reads drives under `/media` and `/run/media`
- [x] Linux: every release's .deb and .rpm are installed and started on Ubuntu 22.04 and 24.04, Debian 12, Fedora and openSUSE Tumbleweed, opening the window on all but openSUSE
- [x] Linux: ARM64 (aarch64) .deb, .rpm and AppImage, installed and started on Ubuntu 24.04 and Fedora for ARM; an AppImage for x86_64 too, which Check for Updates replaces in place; icons from 16 to 256 pixels
- [x] Cut page: the grey lead-in before a track's first clip
- [x] Development: tests on every pull request; a test keeps `Menu-Status.md` in step with the menus; publishing waits for every package and the install tests
- [x] Linux: ARM64 Flatpak (`-aarch64.flatpak`), built on an ARM64 runner by the release workflow, with Check for Updates picking the Flatpak of the computer's processor; built there, not started on ARM hardware yet
- [x] Linux: the Flathub manifest (`packaging/flathub`) builds FFmpeg 7.1 and its codecs, then the program, from source on x86_64 and aarch64 (about 20 minutes in the `flathub` workflow), which also runs Flathub's linter
- [x] Graphics: `picasso-suite --graphics-check` (every adapter draws a test picture and is read back; tried on an AMD card with Vulkan, DirectX 12 and OpenGL, and run on Mesa's software Vulkan in the Linux install tests) and a "Graphics report" issue form for people to send theirs
- [x] Releases: release candidates (`v0.2.2-rc1`) are published as pre-releases with their version's notes, and Check for Updates does not offer them
- [x] Development: `PICASSO_PERF=pages picasso-suite [--place] <files>` shows each page in turn and prints its switch frame and frame times, with the time between frames (tried with twelve 1080p and 4K clips: every page under 1.5 ms a frame once shown, the first visit under 14 ms)
- [x] Status: the README shows how much is made as a picture and a table (`scripts/status.py` draws `docs/status.svg` from `Menu-Status.md` and this file; run it after changing either)
- [x] Interface: the panels on every page are resized by dragging the dividers between them (double click resets), kept in the preferences; panels keep at least two fifths of their usual size, and their text, icons and buttons shrink with them. Not yet: the Color palettes (the wheels have a fixed size), and the Deliver settings form only scrolls in a narrow column instead of shrinking

## To do

- [ ] Interface density: finish the pages measured on the 1080p screenshots (`docs/reference/resolve-1080p`). Done: page bar, labelled top toolbars, Cut, Edit, Fusion, Fairlight, Photo, Color, Media, Deliver. Every page is at Resolve's 1080p size; next, check the same pages at 4K and at small windows
- [ ] Deliver: IMF Generic, Final Cut Pro 7, Premiere XML, AVID AAF and Pro Tools presets (listed, greyed out) need interchange writers (XML, AAF, MXF)
- [ ] Color: the Hue vs Hue, Hue vs Sat, Hue vs Lum, Lum vs Sat, Sat vs Sat and Sat vs Lum curve modes
- [ ] Cut page: the button row beside the tracks (height, lock, solo, mute, picture) only draws
- [ ] Color page: the mini timeline strip under the clips
- [ ] Color: splitter/combiner nodes, viewer split/wipe, window tracking, magic mask, polygon and B-spline masks, Paint and particle tools
- [ ] Editing: AAF/XML export, Trim Editor window, JKL in dynamic trim, auto align (timecode, waveform), Paste Mix, New Effects Clip, audio scrubbing, timeline in a new window, Bounce Selected Tracks
- [ ] Multicam: sync by timecode or waveform, live angle previews, switching sound
- [ ] Audio: choosing the input device, the "Bus 1 -> Auto" output, ADR, audio effect cache, link groups
- [ ] Performance: instant scrubbing on long-GOP video (cache of key-frame pictures or optimised media); the remaining compositing on the interface thread (about 2 ms per picture) moved to a worker
- [ ] Project: libraries, archives, media management, metadata import/export, layouts (save/load), Lightbox, dual screen, clean feed
- [ ] Help: tutorials (the Welcome dialog was removed); Check for Updates still to be tried on macOS, Flatpak, .deb and .rpm (a per-user Flatpak now updates per-user; a .deb is told from an .rpm by dpkg's file list)
- [ ] Security: a time limit on reading project databases; Resolve import on a background thread
- [ ] Platforms: macOS (Metal), Linux (Vulkan), browser limits
- [ ] Linux: native Wayland drag and drop: not possible yet. eframe 0.37 uses winit 0.30.13, which takes no files dropped on a Wayland window; winit's own work on it (rust-windowing/winit#4773 is open, #4009 was closed unmerged) is in no release, 0.31 is a beta. Until then the window opens through XWayland. Writing it here against Wayland's data device would be a second, untestable copy of what winit will do
- [ ] Linux: Flathub submission, for the project's owner: a pull request to flathub/flathub with the manifest. The linter's findings on it: the id `io.github.fabox_os.PicassoSuite` does not match the repository's name (`appid-url-not-reachable`: it looks for github.com/fabox-os/picassosuite, the repository is Picasso-Suite; changing the id would make it a new app for people with the current Flatpak, so the id stays and the request for an exception is written in `packaging/flathub/README.md`); the X11 + Wayland sockets (exception text in `packaging/flathub/README.md`); and a newer runtime (26.08)
- [ ] Graphics: Vulkan on real cards: the check and the report form exist; the NVIDIA, Intel, AMD (Linux, Mesa), Apple and ARM GPUs need people to run `--graphics-check` and send it in
- [ ] Hardware: try the NVIDIA, Intel, Apple and VA-API encoders and decoding on those cards (only AMD on Windows is tried); an Intel Mac package
- [ ] Interchange: OpenTimelineIO import, and OTIO export from `picasso-bridge`
