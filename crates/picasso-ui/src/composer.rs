//! The viewer's pictures are put together on a thread of their own: compositing a frame (decoded
//! video, stills, grades, titles, transitions) took about 2 ms of every frame of the interface
//! while a clip played, and more with several layers or heavy grades. The interface hands over
//! what the picture depends on and goes on drawing the picture it has; the new one is shown on
//! the frame after it arrives.
//!
//! Only the newest request matters: one asked for while another is being put together replaces
//! any that is still waiting, as the video decoders do. The browser has no threads, so there the
//! picture is put together at once, as before.

use std::sync::Arc;

use picasso_core::Project;
use picasso_render::{Frame, FrameKey, MediaCache};

use crate::viewer::ViewerKey;

/// A picture to put together.
pub(crate) struct Job {
    /// Its number, given by [`Composer::submit`].
    pub id: u64,
    pub viewer: ViewerKey,
    pub key: FrameKey,
    pub project: Arc<Project>,
    pub media: MediaCache,
}

/// A picture put together.
pub(crate) struct Done {
    /// The number of the job it was put together for.
    pub id: u64,
    pub viewer: ViewerKey,
    pub key: FrameKey,
    pub frame: Arc<Frame>,
    /// The picture as the viewer's texture takes it: turning every pixel into egui's colours
    /// costs about a millisecond at 960 x 540, so it is done here too.
    pub image: egui::ColorImage,
    /// Every video picture in it was the exact one, so it may be kept in the render cache.
    pub exact: bool,
    /// It took long enough to be worth keeping on disk too.
    pub slow: bool,
}

/// Puts the pictures together for one viewer.
#[derive(Default)]
pub(crate) struct Composer {
    /// The viewer key of the last picture asked for, so it is not asked for again every frame
    /// while it is on its way.
    pub asked: Option<ViewerKey>,
    /// The number the next job gets.
    next: u64,
    /// Jobs numbered below this are out of date: a newer picture was shown since they were
    /// asked for (from the render cache), and showing them would go back in time.
    stale_below: u64,
    #[cfg(not(target_arch = "wasm32"))]
    shared: Option<Arc<native::Shared>>,
    #[cfg(target_arch = "wasm32")]
    done: Option<Done>,
}

/// Puts the picture of `job` together, or `None` when it cannot be.
fn compose(job: &Job) -> Option<Done> {
    #[cfg(not(target_arch = "wasm32"))]
    let started = std::time::Instant::now();
    let (frame, exact) = picasso_render::render_frame_alpha(
        &job.project,
        &job.media,
        job.key.frame,
        job.key.width,
        job.key.height,
        job.key.alpha,
    )
    .ok()?;
    #[cfg(not(target_arch = "wasm32"))]
    let slow = started.elapsed() >= crate::render_cache::SLOW;
    #[cfg(target_arch = "wasm32")]
    let slow = false;
    Some(Done {
        id: job.id,
        viewer: job.viewer.clone(),
        key: job.key,
        image: crate::viewer::color_image(&frame),
        frame: Arc::new(frame),
        exact,
        slow,
    })
}

impl Composer {
    /// Asks for the picture of `job`; `ctx` is asked to draw again when it is ready.
    pub(crate) fn submit(&mut self, ctx: &egui::Context, mut job: Job) {
        job.id = self.next;
        self.next += 1;
        self.asked = Some(job.viewer.clone());
        #[cfg(not(target_arch = "wasm32"))]
        {
            let shared = self
                .shared
                .get_or_insert_with(|| native::start(ctx.clone()));
            native::submit(shared, job);
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = ctx;
            self.done = compose(&job);
        }
    }

    /// A picture that did not come from here was shown (one from the render cache): the jobs
    /// asked for before it are out of date, and their pictures, when they arrive, are not shown.
    pub(crate) fn outdate(&mut self) {
        self.stale_below = self.next;
        self.asked = None;
    }

    /// Whether `done` may be shown: no newer picture was shown since it was asked for.
    pub(crate) fn is_current(&self, done: &Done) -> bool {
        done.id >= self.stale_below
    }

    /// The picture put together since the last call, if one was.
    pub(crate) fn take_done(&mut self) -> Option<Done> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.shared.as_ref().and_then(|s| native::take_done(s))
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.done.take()
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for Composer {
    fn drop(&mut self) {
        if let Some(shared) = &self.shared {
            native::close(shared);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::sync::{Arc, Condvar, Mutex, MutexGuard};

    use super::{compose, Done, Job};

    #[derive(Default)]
    struct Slot {
        /// The picture waiting to be put together; a newer one replaces it.
        job: Option<Job>,
        /// The last picture put together, until the interface takes it.
        done: Option<Done>,
        closed: bool,
    }

    pub(super) struct Shared {
        slot: Mutex<Slot>,
        wake: Condvar,
    }

    impl Shared {
        fn lock(&self) -> MutexGuard<'_, Slot> {
            // A panic while putting a picture together poisons the lock; the slot is still usable.
            self.slot.lock().unwrap_or_else(|e| e.into_inner())
        }
    }

    pub(super) fn start(ctx: egui::Context) -> Arc<Shared> {
        let shared = Arc::new(Shared {
            slot: Mutex::default(),
            wake: Condvar::new(),
        });
        let worker = shared.clone();
        let spawned = std::thread::Builder::new()
            .name("viewer-compose".into())
            .spawn(move || run(&worker, &ctx));
        if let Err(e) = spawned {
            tracing::error!("could not start the viewer's compositing thread: {e}");
        }
        shared
    }

    pub(super) fn submit(shared: &Shared, job: Job) {
        shared.lock().job = Some(job);
        shared.wake.notify_all();
    }

    pub(super) fn take_done(shared: &Shared) -> Option<Done> {
        shared.lock().done.take()
    }

    pub(super) fn close(shared: &Shared) {
        shared.lock().closed = true;
        shared.wake.notify_all();
    }

    fn run(shared: &Shared, ctx: &egui::Context) {
        loop {
            let job = {
                let mut slot = shared.lock();
                loop {
                    if slot.closed {
                        return;
                    }
                    if let Some(job) = slot.job.take() {
                        break job;
                    }
                    slot = shared.wake.wait(slot).unwrap_or_else(|e| e.into_inner());
                }
            };
            // A picture that cannot be put together leaves the one on show.
            if let Some(done) = compose(&job) {
                shared.lock().done = Some(done);
                ctx.request_repaint();
            }
        }
    }
}
