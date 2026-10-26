# Roadmap

Status at 0.2.1.

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

## To do

- [ ] Interface density: finish the pages measured on the 1080p screenshots (`docs/reference/resolve-1080p`). Done: page bar, labelled top toolbars, Cut, Edit, Fusion, Fairlight, Photo, Color, Media, Deliver. Every page is at Resolve's 1080p size; next, check the same pages at 4K and at small windows
- [ ] Deliver: IMF Generic, Final Cut Pro 7, Premiere XML, AVID AAF and Pro Tools presets (listed, greyed out) need interchange writers (XML, AAF, MXF)
- [ ] Color: the Hue vs Hue, Hue vs Sat, Hue vs Lum, Lum vs Sat, Sat vs Sat and Sat vs Lum curve modes
- [ ] Cut page: the button row beside the tracks (height, lock, solo, mute, picture) only draws; the clip area before a clip's start (Resolve's grey lead-in)
- [ ] Color page: the mini timeline strip under the clips
- [ ] Color: splitter/combiner nodes, viewer split/wipe, window tracking, magic mask, polygon and B-spline masks, Paint and particle tools
- [ ] Editing: AAF/XML export, Trim Editor window, JKL in dynamic trim, auto align (timecode, waveform), Paste Mix, New Effects Clip, audio scrubbing, timeline in a new window, Bounce Selected Tracks
- [ ] Multicam: sync by timecode or waveform, live angle previews, switching sound
- [ ] Audio: choosing the input device, the "Bus 1 -> Auto" output, ADR, audio effect cache, link groups
- [ ] Performance: instant scrubbing on long-GOP video (cache of key-frame pictures or optimised media); the remaining compositing on the interface thread (about 2 ms per picture) moved to a worker
- [ ] Project: libraries, archives, media management, metadata import/export, layouts (save/load), Lightbox, dual screen, clean feed
- [ ] Help: tutorials (the Welcome dialog was removed); Check for Updates still to be tried on macOS, Flatpak, .deb and .rpm, and the Flatpak needs `--talk-name=org.freedesktop.Flatpak` to install its own update
- [ ] Security: a time limit on reading project databases; Resolve import on a background thread
- [ ] Platforms: macOS (Metal), Linux (Vulkan), browser limits
