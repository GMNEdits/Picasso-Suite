# AGENTS.md - Picasso Suite - Clean Room Agent Instructions

This file instructs all coding agents (human or AI) working on Picasso Suite.

## 0. Project Goal
Build a DaVinci Resolve-class NLE in Rust + WebAssembly that runs on Windows, Mac, Linux with NO format restrictions (Linux-first codec freedom). UI already exists (see photo3869204475288371488.jpeg). Keep UI shell, implement engines underneath.

## 1. Tech Stack - Non-Negotiable

- Language: Rust 1.78+ (2021 edition)
- UI: Keep existing egui/iced shell. Do not rewrite to React.
- Graphics: wgpu only. No OpenGL direct calls. No glow.
- Decode: ffmpeg-next crate with static linking. Feature flag `all-formats`. Symphonia as fallback for audio.
- Audio: cpal + oddio or fundsp for mixer
- Project DB: rusqlite + serde_json
- WASM: wasm-bindgen, wasm-bindgen-futures, web-sys. Heavy decode must be in Web Worker (wasm_thread or comlink-like).
- Target: `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, `wasm32-unknown-unknown`

## 2. Clean Room Implementation Rules

**CRITICAL - Legal Safety:**
1.  DO NOT search for "DaVinci Resolve source code", "cracked Resolve", "decompile".
2.  Functional description only: Describe WHAT it does, not HOW Blackmagic did it.
3.  Implement color science from OpenColorIO docs, Academy ACES docs, and public GPU Gems.
4.  Implement timeline math from OTIO (OpenTimelineIO) spec, not Resolve.
5.  All shaders must be original. Use wgpu examples as base, not Resolve LUTs.
6.  If asked to copy a specific Resolve effect exactly, implement an academic equivalent and note difference.

## 3. Agent Roles

### Agent: media-agent
- Owns: crates/picasso-media, picasso-decode, picasso-cache
- Tasks: Probe file, extract metadata, generate thumbnail/waveform, frame cache
- API: `trait FrameProvider { fn frame_at(&self, pts: i64) -> Result<Texture> }`
- Must handle: Variable frame rate, timecode gaps, 10-bit, HDR

### Agent: timeline-agent
- Owns: crates/picasso-core, picasso-timeline
- Tasks: Clip/track model, trim logic, undo/redo
- Rules:
  - Time is always i64 microseconds or Rational (num-rational), never f32 seconds
  - Timeline is immutable + command log. Never mutate in place without command.
  - Implement OTIO import/export for interoperability

### Agent: render-agent
- Owns: crates/picasso-render, picasso-color
- Tasks: wgpu render graph, compositing, color pipeline
- MVP Pipeline: `Media -> Decoder -> Linearize (Rec.709->Scene Linear) -> Transform -> ColorCorrect -> ToneMap -> Viewer`
- Shaders in `/shaders/*.wgsl`

### Agent: audio-agent
- Owns: crates/picasso-audio
- Tasks: Waveform, gain, mixer, export mixdown
- Use 32-bit float pipeline, 48kHz internal

### Agent: export-agent
- Owns: crates/picasso-export
- Tasks: Encoding via ffmpeg, render queue
- Never block UI thread. Use tokio + channel for progress.

### Agent: wasm-agent
- Owns: WASM compatibility
- Tasks: Ensure no `std::fs` direct, no `tokio::fs` in core. Use trait for IO.
- Web worker for decode: send frame as ImageBitmap

## 4. Coding Standards

```rust
// Good: trait-based, testable, WASM friendly
pub trait MediaSource: Send + Sync {
    fn metadata(&self) -> MediaMetadata;
    fn decode_video(&self, pts: Time) -> Result<Frame>;
}

// Bad: OS-specific, blocking
// fn load_file(path: &str) -> std::fs::File
```

- Every crate must have `#[cfg(test)]` for timeline math
- No unwrap() in core, only in UI with expect msg
- Use `anyhow` + `thiserror`
- Log with `tracing`

## 5. Current UI Contract (from screenshot)

UI provides:
- Media Pool panel (left): `No clips in media pool` -> will call `media::import(paths)`
- Timeline (bottom): needs `timeline::state()` -> Vec<Track>
- Viewer (center): needs `render::current_frame()`
- Transport: JKL, Play, Stop, TCG (timecode)

Do not change UI file paths. Agents expose functions UI calls.

## 6. MVP Task List (in order)

1.  `cargo new --lib crates/picasso-core` - define Project, Clip, Track
2.  Implement ffmpeg probe in picasso-media
3.  wgpu texture upload + viewer draw
4.  Timeline insert/overwrite
5.  JKL playback (frame stepping)
6.  Transform shader + opacity
7.  Scopes (waveform compute shader)
8.  Export H264

## 7. How to Test Linux Format Freedom

```bash
# Must pass on Ubuntu 22.04 clean install:
cargo test --all -- --test-threads=1
./test_media.sh # includes: mkv h265 10bit, mov prores raw, avi dv, mp4 av1, webm vp9, mxf
```

If a format fails on Linux but works on Win/Mac, fix is in bundled ffmpeg, not OS install.

## 8. Prohibited

- No GPL violation: ffmpeg is LGPL if you use dynamic or GPL if static. For closed source, use dynamic linking on release, or open-source Picasso. For MVP, GPL static is OK for personal use.
- No hardcoded paths, no `C:\` 
- No Electron. Rust only.

## 9. Final Note to Agents

You are not cloning Resolve UI. You are building the engine that makes the UI in the screenshot functional. Think like OTIO + Blender VSE + kdenlive, but with Resolve's UX polish.

When in doubt: Rust book > Resolve manual.

END OF AGENTS.md
