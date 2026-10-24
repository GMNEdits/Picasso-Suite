//! Background threads: viewer rendering, media import, export.

use picasso_core::{Id, MediaItem, Project, Rational};
use picasso_media::RgbaImage;
use picasso_render::{DecoderPool, ExportProgress, ExportSettings, render_frame};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

// ---------------------------------------------------------------- preview

pub struct PreviewRequest {
    pub project: Arc<Project>,
    pub frame: i64,
    pub width: u32,
    pub height: u32,
}

pub struct PreviewFrame {
    pub image: RgbaImage,
    pub failures: HashMap<Id, String>,
}

pub struct Preview {
    tx: Sender<PreviewRequest>,
    rx: Receiver<PreviewFrame>,
    last_sent: Option<(u64, i64, u32, u32)>,
}

impl Preview {
    pub fn spawn(ctx: egui::Context) -> Self {
        let (tx, req_rx) = channel::<PreviewRequest>();
        let (frame_tx, rx) = channel();
        std::thread::Builder::new()
            .name("preview".into())
            .spawn(move || {
                let mut pool = DecoderPool::new(16);
                let mut media_version = None;
                while let Ok(mut req) = req_rx.recv() {
                    // Only the newest request matters (scrubbing, playback catch-up).
                    while let Ok(newer) = req_rx.try_recv() {
                        req = newer;
                    }
                    let fingerprint: Vec<_> = req.project.media.iter().map(|m| (m.id, m.path.clone())).collect();
                    if media_version.as_ref() != Some(&fingerprint) {
                        pool.reset();
                        media_version = Some(fingerprint);
                    }
                    let image = render_frame(&req.project, req.frame, req.width, req.height, &mut pool);
                    let msg = PreviewFrame { image, failures: pool.failures().clone() };
                    if frame_tx.send(msg).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            })
            .expect("spawn preview thread");
        Self { tx, rx, last_sent: None }
    }

    /// Requests a frame unless the identical request was already sent.
    pub fn request(&mut self, project: &Arc<Project>, version: u64, frame: i64, width: u32, height: u32) {
        let key = (version, frame, width, height);
        if self.last_sent == Some(key) {
            return;
        }
        self.last_sent = Some(key);
        let _ = self.tx.send(PreviewRequest { project: project.clone(), frame, width, height });
    }

    pub fn poll(&self) -> Option<PreviewFrame> {
        let mut latest = None;
        while let Ok(f) = self.rx.try_recv() {
            latest = Some(f);
        }
        latest
    }
}

// ---------------------------------------------------------------- import

pub type ImportOutcome = Vec<(PathBuf, Result<MediaItem, String>)>;

pub fn spawn_import(ctx: egui::Context, paths: Vec<PathBuf>, sequence_fps: Rational) -> Receiver<ImportOutcome> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let mut seen_patterns = Vec::new();
        let mut out = Vec::new();
        for path in paths {
            let result = picasso_media::probe(&path, sequence_fps).map_err(|e| e.to_string());
            // Selecting many frames of one sequence imports it once.
            if let Ok(item) = &result
                && item.sequence.is_some() {
                    if seen_patterns.contains(&item.path) {
                        continue;
                    }
                    seen_patterns.push(item.path.clone());
                }
            out.push((path, result));
        }
        let _ = tx.send(out);
        ctx.request_repaint();
    });
    rx
}

// ---------------------------------------------------------------- export

pub struct ExportJob {
    pub progress: Arc<ExportProgress>,
    pub result: Receiver<Result<(), String>>,
}

pub fn spawn_export(ctx: egui::Context, project: Project, settings: ExportSettings) -> ExportJob {
    let progress = Arc::new(ExportProgress::default());
    let (tx, result) = channel();
    let p = progress.clone();
    std::thread::spawn(move || {
        let ctx2 = ctx.clone();
        let ticker = std::thread::spawn({
            let p = p.clone();
            move || {
                while p.fraction() < 1.0 && !p.cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    ctx2.request_repaint();
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        });
        let r = picasso_render::export(&project, &settings, &p).map_err(|e| e.to_string());
        p.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = ticker.join();
        let _ = tx.send(r);
        ctx.request_repaint();
    });
    ExportJob { progress, result }
}

// ---------------------------------------------------------------- thumbnails and waveforms

pub enum OverviewResult {
    Thumbnail(Id, RgbaImage),
    Waveform(Id, Vec<f32>),
}

/// Waveform resolution: peaks per second of media.
pub const WAVE_PER_SECOND: u32 = 100;

pub struct Overviews {
    tx: Sender<MediaItem>,
    rx: Receiver<OverviewResult>,
    requested: std::collections::HashSet<Id>,
}

impl Overviews {
    pub fn spawn(ctx: egui::Context) -> Self {
        let (tx, req_rx) = channel::<MediaItem>();
        let (res_tx, rx) = channel();
        std::thread::Builder::new()
            .name("overviews".into())
            .spawn(move || {
                while let Ok(m) = req_rx.recv() {
                    let poster = (m.info.duration_secs * 0.1).min(2.0);
                    if let Ok(Some(img)) = picasso_media::overview::thumbnail(&m, poster, 320, 180) {
                        let _ = res_tx.send(OverviewResult::Thumbnail(m.id, img));
                        ctx.request_repaint();
                    }
                    if let Ok(w) = picasso_media::overview::waveform(&m, WAVE_PER_SECOND)
                        && !w.is_empty() {
                            let _ = res_tx.send(OverviewResult::Waveform(m.id, w));
                            ctx.request_repaint();
                        }
                }
            })
            .expect("spawn overview thread");
        Self { tx, rx, requested: Default::default() }
    }

    /// Queues thumbnails and waveforms for media not seen before.
    pub fn ensure(&mut self, media: &[MediaItem]) {
        for m in media {
            if m.info.format_name != "offline" && self.requested.insert(m.id) {
                let _ = self.tx.send(m.clone());
            }
        }
    }

    /// Forgets a media item so it is analysed again (after relinking).
    pub fn forget(&mut self, id: Id) {
        self.requested.remove(&id);
    }

    pub fn poll(&self) -> Vec<OverviewResult> {
        self.rx.try_iter().collect()
    }
}
