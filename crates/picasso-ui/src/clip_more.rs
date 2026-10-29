//! Clip Attributes (name and facts about a clip) and Render in Place (a clip with its colour,
//! effects, transform and speed baked into a new file that takes its place).

use egui::Context;
use picasso_core::{ClipId, Command, TrackKind};

use crate::app::StudioApp;

impl StudioApp {
    pub(crate) fn open_attributes(&mut self) {
        match self
            .target_clip()
            .and_then(|id| self.project().clip(id).map(|(_, c)| (id, c.name.clone())))
        {
            Some(open) => self.extra.attributes = Some(open),
            None => self.fail("select a clip first"),
        }
    }

    pub(crate) fn attributes_window(&mut self, ctx: &Context) {
        let Some((id, mut name)) = self.extra.attributes.take() else {
            return;
        };
        let Some((track, clip)) = self.project().clip(id) else {
            return;
        };
        let (track, clip) = (track.clone(), clip.clone());
        let media = self.project().media(clip.media).cloned();
        let tc = |frames: u64| self.tc(frames as i64);
        let mut open = true;
        let mut done = None;
        egui::Window::new("Clip Attributes")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name");
                    let field = ui.text_edit_singleline(&mut name);
                    if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        done = Some(true);
                    }
                });
                ui.separator();
                egui::Grid::new("clip_attributes")
                    .num_columns(2)
                    .show(ui, |ui| {
                        let row = |ui: &mut egui::Ui, k: &str, v: String| {
                            ui.label(k);
                            ui.label(v);
                            ui.end_row();
                        };
                        row(ui, "Track", track.name.clone());
                        row(
                            ui,
                            "Media",
                            media
                                .as_ref()
                                .map_or("(missing)".into(), |m| m.name.clone()),
                        );
                        if let Some(m) = &media {
                            row(ui, "Type", picasso_core::media_kind(&m.source).to_string());
                            if let Some(l) = m.length {
                                row(ui, "Media length", tc(l));
                            }
                        }
                        row(ui, "Start", tc(clip.start));
                        row(ui, "End", tc(clip.end()));
                        row(ui, "Duration", tc(clip.duration));
                        row(ui, "Source in", tc(clip.source_in));
                        row(ui, "Speed", format!("{:.0} %", clip.speed * 100.0));
                        row(ui, "Nodes", clip.node_count().to_string());
                        row(ui, "Effects", clip.effects.len().to_string());
                        row(
                            ui,
                            "Linked",
                            if clip.link.is_some() { "yes" } else { "no" }.into(),
                        );
                    });
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        done = Some(true);
                    }
                    if ui.button("Cancel").clicked() {
                        done = Some(false);
                    }
                });
            });
        if done == Some(true) {
            if name != clip.name {
                self.run(Command::RenameClip { clip: id, name });
            }
        } else if open && done.is_none() {
            self.extra.attributes = Some((id, name));
        }
    }

    /// Render in Place: renders the clip (with its grade, effects, transform and speed) in the
    /// background to a ProRes file, then swaps the clip's footage for it.
    pub(crate) fn render_in_place(&mut self, ctx: &Context) {
        let Some(id) = self.target_clip() else {
            self.fail("select a clip first");
            return;
        };
        let Some((track, clip)) = self.project().clip(id) else {
            return;
        };
        if track.kind != TrackKind::Video {
            self.fail("Render in Place needs a video clip");
            return;
        }
        if clip.locked {
            self.fail("the clip is locked");
            return;
        }
        // An effects clip has no picture of its own: what it shows is the tracks below it.
        if self
            .project()
            .media(clip.media)
            .is_some_and(|m| matches!(m.source, picasso_core::MediaSource::EffectsClip))
        {
            self.fail("Render in Place cannot render an effects clip: it has no picture of its own");
            return;
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = ctx;
            self.fail("rendering needs the desktop app");
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (track_id, duration) = (track.id, clip.duration);
            // A project of only this clip, from frame 0, at full strength.
            let mut sub = self.project().clone();
            sub.tracks.retain(|t| t.id == track_id);
            sub.timelines.clear();
            sub.markers.clear();
            sub.mark_in = None;
            sub.mark_out = None;
            if let Some(t) = sub.tracks.first_mut() {
                t.clips.retain(|c| c.id == id);
                for c in &mut t.clips {
                    c.start = 0;
                    c.transition = None;
                    c.opacity = 1.0;
                    c.blend = picasso_core::BlendMode::Normal;
                    c.link = None;
                }
            }
            let preset = picasso_export::Preset::ProRes422;
            let dir = if cfg!(test) {
                Some(std::env::temp_dir().join("picasso_render_in_place_test"))
            } else {
                crate::project_manager::data_dir().map(|d| d.join("Rendered Clips"))
            };
            let Some(dir) = dir.filter(|d| std::fs::create_dir_all(d).is_ok()) else {
                self.fail("cannot make the Rendered Clips folder");
                return;
            };
            let Some(path) = (1..)
                .map(|n| {
                    dir.join(format!(
                        "{} Clip {n}.{}",
                        self.project().name,
                        preset.extension()
                    ))
                })
                .find(|p| !p.exists())
            else {
                return;
            };
            let settings = picasso_export::Settings {
                preset,
                path: path.clone(),
                start: 0,
                end: duration,
                options: picasso_export::Options::default(),
            };
            let (inbox, ctx, cache) = (self.inbox.clone(), ctx.clone(), self.cache.clone());
            self.info("Rendering the clip...");
            std::thread::spawn(move || {
                let result = picasso_export::render(&sub, &cache, &settings, &mut |_, _| true);
                inbox.push(
                    &ctx,
                    match result {
                        Ok(()) => crate::platform::Incoming::Rendered {
                            clip: id,
                            path: path.to_string_lossy().into_owned(),
                        },
                        Err(e) => crate::platform::Incoming::Error(format!("render failed: {e}")),
                    },
                );
            });
        }
    }

    /// The render is ready: imports it and puts it in the clip's place.
    pub(crate) fn finish_render_in_place(&mut self, clip: ClipId, path: &str) {
        #[cfg(target_arch = "wasm32")]
        let _ = (clip, path);
        #[cfg(not(target_arch = "wasm32"))]
        {
            let before = self.project().media.len();
            self.open_path_now(path);
            if self.project().media.len() == before {
                return;
            }
            let Some(media) = self.project().media.last().map(|m| m.id) else {
                return;
            };
            if self.project().clip(clip).is_none() {
                self.info("The clip was removed while it rendered; the render is in the pool");
                return;
            }
            if self.run(Command::RenderInPlace { clip, media }).is_some() {
                self.info("Rendered in place");
            }
        }
    }
}
