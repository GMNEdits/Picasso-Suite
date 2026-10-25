# Roadmap

Status as of 0.1.0 hotfix.

## Phase 1: Foundation (done)

- [x] Rust workspace: `picasso-core`, `-media`, `-render`, `-interop`, `-app`, `-cli`
- [x] Project model, undo/redo, `.pcsp` project files
- [x] Any FFmpeg-readable format: probe, frame-accurate decode, thumbnails, waveforms
- [x] Float32 compositor with a decoder pool

## Phase 2: Editor and interface (done)

- [x] Resolve-style chrome: menu bar, toolbars, page bar, design-grid scaling
- [x] Cut, Edit and Deliver pages
- [x] Multi-track timeline: place/overwrite, blade, delete, ripple delete, move, trim, snapping, linked clips
- [x] Per-clip zoom, position, opacity, volume

## Phase 3: Export and interchange (done)

- [x] H.264 and H.265 (8/10-bit) with x264/x265 and hardware encoders
- [x] ProRes, DNxHR, AV1, VP9, FFV1, image sequences, WAV, MP3
- [x] `.drp` import (plain-XML parts) with cross-OS media relinking
- [x] OpenTimelineIO export, `picasso` command-line tool

## Phase 4: Release (in progress)

- [x] GPL-3.0-or-later licence, changelog, release workflow
- [x] Windows zip, `.deb`, `.rpm`, macOS universal app
- [x] Flatpak, with FFmpeg included
- [x] 0.1.0 hotfix release: packages named after the tag, rebuilds replace the old files
- [ ] Cut page: pixel-level match with the reference (ruler, meter, empty media pool)

## Phase 5: Polish (in progress)

- [x] UI stalls fixed (About window, window title)
- [x] Rebindable keyboard shortcuts, with a warning when a key is taken
- [x] Clear message when FFmpeg is missing
- [ ] Real-time audio playback (preview is silent)
- [ ] Restyle Edit and Deliver pages to match the Cut page
- [ ] Hardware-decode toggle in the UI
- [ ] Automated tests in CI
- [x] Bundled FFmpeg in the Windows package
- [ ] Bundled FFmpeg in the macOS package
- [ ] Signed and notarised macOS app

## Phase 6: New pages and features (not done)

- [ ] Color, Fusion and Fairlight pages
- [ ] Effects, transitions, titles and keyframes
- [ ] `.drp` binary-encoded parts, and a round-trip test against a real Resolve export
