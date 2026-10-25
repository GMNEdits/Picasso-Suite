# Roadmap

Status as of 0.1.0.

## Done

- [x] Rust workspace: `picasso-core`, `-media`, `-render`, `-interop`, `-app`, `-cli`
- [x] Resolve-style chrome: menu bar, interface toolbar, page bar, design-grid scaling
- [x] **Cut page**: media pool, viewer, transport, overview timeline, detail timeline with centre playhead
- [x] **Edit page** and **Deliver page**
- [x] Multi-track timeline: place/overwrite, blade, delete, ripple delete, move, trim, snapping, linked clips
- [x] Per-clip zoom, position, opacity, volume; undo/redo
- [x] Any FFmpeg-readable format: probe, frame-accurate decode, thumbnails, waveforms
- [x] Export: H.264 and H.265 (8/10-bit) with x264/x265 and hardware encoders, ProRes, DNxHR, AV1, VP9, FFV1, image sequences, WAV, MP3
- [x] `.pcsp` project files
- [x] `.drp` import (plain-XML parts) with cross-OS media relinking
- [x] OpenTimelineIO export
- [x] `picasso` command-line tool
- [x] Packaging: Windows zip, `.deb`, `.rpm`, macOS universal app
- [x] GPL-3.0-or-later licence, changelog, release workflow

## In progress

- [ ] Flatpak package: FFmpeg-in-sandbox build fix pushed, waiting for a successful CI run
- [ ] Cut page: pixel-level match with the reference (ruler, meter and media-pool empty state still being refined)

## Not done

- [ ] Real-time audio playback (preview is silent)
- [ ] Restyle Edit and Deliver pages to match the Cut page
- [ ] Color, Fusion and Fairlight pages
- [ ] Effects, transitions, titles and keyframes
- [ ] `.drp` binary-encoded parts, and a round-trip test against a real project exported from Resolve
- [ ] Signed and notarised macOS app
- [ ] Automated tests in CI (tests run locally only)
- [ ] Hardware-accelerated decode toggle in the UI
- [ ] Bundled FFmpeg for the Windows and macOS packages
