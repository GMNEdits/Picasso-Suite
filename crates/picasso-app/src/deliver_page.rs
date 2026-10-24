//! The Deliver page: render settings, viewer, render queue, timeline.

use crate::app::{JobState, PicassoApp};
use crate::theme::*;
use crate::timeline_ui::{self, Overviews};
use egui::{Align2, Color32, Rect, RichText, Stroke, TextureOptions, pos2, vec2};
use picasso_core::time::format_timecode;
use std::sync::atomic::Ordering;

const SETTINGS_W: f32 = 470.0;
const QUEUE_W: f32 = 420.0;

impl PicassoApp {
    pub(crate) fn deliver_page(&mut self, ui: &mut egui::Ui, body: Rect) {
        let ctx = ui.ctx().clone();
        let top_bottom = body.top() + (body.height() * 0.56).max(360.0);
        let settings = Rect::from_min_max(body.min, pos2(body.left() + SETTINGS_W, top_bottom));
        let queue = Rect::from_min_max(pos2(body.right() - QUEUE_W, body.top()), pos2(body.right(), top_bottom));
        let viewer = Rect::from_min_max(pos2(settings.right() + DIVIDER_W, body.top()), pos2(queue.left() - DIVIDER_W, top_bottom));
        let timeline = Rect::from_min_max(pos2(body.left(), top_bottom + DIVIDER_W), body.max);

        // Render settings.
        let painter = ui.painter_at(settings);
        painter.rect_filled(settings, 0.0, HEADER_BG);
        let header = Rect::from_min_size(settings.min, vec2(settings.width(), HEADER_H));
        painter.text(pos2(header.left() + 14.0, header.center().y), Align2::LEFT_CENTER, "Render Settings", semibold(14.0), TEXT);
        painter.line_segment([header.left_bottom(), header.right_bottom()], Stroke::new(1.0, DIVIDER));
        let form = Rect::from_min_max(pos2(settings.left() + 16.0, header.bottom() + 14.0), settings.max - vec2(16.0, 56.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(form));
        egui::ScrollArea::vertical().id_salt("render-settings").show(&mut child, |ui| {
            // Quick format chooser, H.264 and H.265 first.
            ui.horizontal_wrapped(|ui| {
                let quick: Vec<(usize, &'static str)> = self
                    .render
                    .presets
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| matches!(p.id, "h264" | "h265" | "h265_10bit" | "prores_hq" | "dnxhr_hq" | "av1"))
                    .map(|(i, p)| (i, p.id))
                    .collect();
                for (i, id) in quick {
                    let label = match id {
                        "h264" => "H.264",
                        "h265" => "H.265",
                        "h265_10bit" => "H.265 10-bit",
                        "prores_hq" => "ProRes",
                        "dnxhr_hq" => "DNxHR",
                        _ => "AV1",
                    };
                    if ui.selectable_label(self.render.preset == i, label).clicked() {
                        self.render.preset = i;
                        self.render.extension = self.render.presets[i].default_extension();
                    }
                }
            });
            ui.add_space(10.0);
            self.render_settings_form(ui);
        });
        let add = Rect::from_min_max(pos2(settings.left() + 16.0, settings.bottom() - 46.0), pos2(settings.right() - 16.0, settings.bottom() - 12.0));
        if ui.put(add, egui::Button::new(RichText::new("Add to Render Queue").size(15.0))).clicked() {
            self.add_to_queue();
        }

        // Viewer.
        let vp = ui.painter_at(viewer);
        vp.rect_filled(viewer, 0.0, VIEWER_BG);
        let vh = Rect::from_min_size(viewer.min, vec2(viewer.width(), HEADER_H));
        vp.rect_filled(vh, 0.0, HEADER_BG);
        let fps = self.fps();
        let tc0 = self.project.timeline.start_timecode;
        vp.text(vh.center(), Align2::CENTER_CENTER, &self.project.timeline.name, semibold(14.0), TEXT);
        vp.text(pos2(vh.right() - 12.0, vh.center().y), Align2::RIGHT_CENTER, format_timecode(self.playhead + tc0, fps), semibold(14.0), TIMECODE);
        let (mi, mo) = self.marks;
        let range = format!(
            "IN {}   OUT {}   DURATION {}",
            format_timecode(mi.unwrap_or(0) + tc0, fps),
            format_timecode(mo.unwrap_or(self.project.timeline.duration()) + tc0, fps),
            format_timecode(mo.unwrap_or(self.project.timeline.duration()) - mi.unwrap_or(0), fps)
        );
        vp.text(pos2(viewer.center().x, viewer.bottom() - 16.0), Align2::CENTER_CENTER, range, regular(12.0), TEXT_DIM);
        let s = &self.project.settings;
        let aspect = s.width as f32 / s.height.max(1) as f32;
        let avail = Rect::from_min_max(pos2(viewer.left() + 10.0, vh.bottom() + 10.0), pos2(viewer.right() - 10.0, viewer.bottom() - 34.0));
        let (mut w, mut h) = (avail.width(), avail.width() / aspect);
        if h > avail.height() {
            h = avail.height();
            w = h * aspect;
        }
        let pic = Rect::from_center_size(avail.center(), vec2(w.max(2.0), h.max(2.0)));
        let scale = ((w * ctx.pixels_per_point()) / s.width.max(1) as f32).min(1.0);
        let pw = ((s.width as f32 * scale) as u32).max(2) & !1;
        let ph = ((s.height as f32 * scale) as u32).max(2) & !1;
        if let Some(frame) = self.preview.poll() {
            let ci = egui::ColorImage::from_rgba_unmultiplied([frame.image.width as usize, frame.image.height as usize], &frame.image.data);
            match &mut self.texture {
                Some(t) => t.set(ci, TextureOptions::LINEAR),
                None => self.texture = Some(ctx.load_texture("viewer", ci, TextureOptions::LINEAR)),
            }
        }
        self.preview.request(&self.snapshot, self.version, self.playhead, pw, ph);
        if let Some(t) = &self.texture {
            vp.image(t.id(), pic, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }

        // Render queue.
        let qp = ui.painter_at(queue);
        qp.rect_filled(queue, 0.0, HEADER_BG);
        let qh = Rect::from_min_size(queue.min, vec2(queue.width(), HEADER_H));
        qp.text(pos2(qh.left() + 14.0, qh.center().y), Align2::LEFT_CENTER, "Render Queue", semibold(14.0), TEXT);
        qp.line_segment([qh.left_bottom(), qh.right_bottom()], Stroke::new(1.0, DIVIDER));
        let list = Rect::from_min_max(pos2(queue.left() + 12.0, qh.bottom() + 10.0), queue.max - vec2(12.0, 56.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(list));
        let mut remove = None;
        let running = self.job.as_ref().map(|(i, j)| (*i, j.progress.clone()));
        egui::ScrollArea::vertical().id_salt("queue").show(&mut child, |ui| {
            if self.queue.is_empty() {
                ui.add_space(30.0);
                ui.vertical_centered(|ui| ui.label(RichText::new("No jobs in render queue").color(TEXT_FAINT).size(15.0)));
            }
            for (i, job) in self.queue.iter().enumerate() {
                egui::Frame::new().fill(Color32::from_rgb(40, 39, 46)).corner_radius(4.0).inner_margin(10.0).show(ui, |ui| {
                    ui.set_width(list.width() - 24.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("Job {}", i + 1)).strong());
                        ui.label(RichText::new(&job.name).color(TEXT_DIM));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if !matches!(job.state, JobState::Rendering) && ui.small_button("✕").on_hover_text("Remove job").clicked() {
                                remove = Some(i);
                            }
                        });
                    });
                    ui.label(RichText::new(job.settings.preset.label).size(12.0).color(TEXT_DIM));
                    match &job.state {
                        JobState::Queued => {
                            ui.label(RichText::new("Queued").size(12.0).color(TEXT_DIM));
                        }
                        JobState::Rendering => {
                            let frac = running.as_ref().filter(|(ri, _)| *ri == i).map_or(0.0, |(_, p)| p.fraction());
                            ui.add(egui::ProgressBar::new(frac).show_percentage().desired_height(14.0));
                        }
                        JobState::Done => {
                            ui.label(RichText::new(format!("Done · {}", job.settings.output.display())).size(12.0).color(OK));
                        }
                        JobState::Failed(e) => {
                            ui.label(RichText::new(format!("Failed: {e}")).size(12.0).color(ERROR));
                        }
                        JobState::Cancelled => {
                            ui.label(RichText::new("Cancelled").size(12.0).color(WARN));
                        }
                    }
                });
                ui.add_space(6.0);
            }
        });
        if let Some(i) = remove {
            self.queue.remove(i);
            if let Some((ri, _)) = &mut self.job
                && *ri > i {
                    *ri -= 1;
                }
        }
        let button = Rect::from_min_max(pos2(queue.left() + 12.0, queue.bottom() - 46.0), pos2(queue.right() - 12.0, queue.bottom() - 12.0));
        if let Some((_, job)) = &self.job {
            let progress = job.progress.clone();
            if ui.put(button, egui::Button::new(RichText::new("Stop").size(15.0))).clicked() {
                progress.cancel.store(true, Ordering::Relaxed);
            }
        } else {
            let any = self.queue.iter().any(|j| matches!(j.state, JobState::Queued));
            if ui.add_enabled_ui(any, |ui| ui.put(button, egui::Button::new(RichText::new("Render All").size(15.0)))).inner.clicked() {
                self.start_next_job(&ctx);
            }
        }

        // Timeline.
        if self.first_layout && self.project.timeline.duration() > 0 {
            self.timeline.zoom_to_fit(self.project.timeline.duration(), timeline.width());
            self.first_layout = false;
        }
        let overviews = Overviews { thumbs: &self.thumbs, waves: &self.waves, marks: self.marks };
        let actions = timeline_ui::show(ui, timeline, &mut self.timeline, &self.project, &self.selection, self.playhead, &overviews);
        for a in actions {
            self.apply(a);
        }
        self.status_toast(ui, timeline);
    }
}
