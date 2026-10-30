//! The render settings of the Deliver page: the preset, the file name and place, and the Video,
//! Audio and File groups. Every control changes what is rendered (see `picasso_export::Options`).

#[cfg(target_arch = "wasm32")]
use crate::{app::StudioApp, chrome::Screen};
#[cfg(target_arch = "wasm32")]
use egui::{Context, Ui};

#[cfg(target_arch = "wasm32")]
impl StudioApp {
    pub(crate) fn render_form(
        &mut self,
        ui: &mut Ui,
        _ctx: &Context,
        s: &Screen,
        top: f32,
        bottom: f32,
    ) {
        let area = s.r(
            crate::deliver::LEFT,
            top + crate::deliver::BAR,
            crate::deliver::LEFT_RIGHT,
            bottom,
        );
        ui.painter().text(
            area.center(),
            egui::Align2::CENTER_CENTER,
            "Rendering needs the desktop app",
            crate::theme::regular(22.0),
            crate::theme::EMPTY_TITLE,
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use egui::{
        pos2, vec2, Align2, Color32, Context, Painter, Rect, Response, Sense, Stroke, StrokeKind,
        Ui, UiBuilder,
    };
    use picasso_export::{
        AudioCodec, Hardware, Preset, Profile, Quality, Rate, Speed, DNXHR_PROFILES,
    };

    use crate::app::StudioApp;
    use crate::chrome::Screen;
    use crate::deliver::{Tab, BAR, LEFT, LEFT_RIGHT, RESOLUTIONS};
    use crate::icons::{paint, Icon};
    use crate::platform;
    use crate::theme::{self, regular, semibold};

    const LABEL_RIGHT: f32 = 205.0;
    const CONTROL_LEFT: f32 = 220.0;
    const WIDE: f32 = 292.0;
    const FIELD: Color32 = Color32::from_rgb(34, 33, 38);
    const FIELD_LINE: Color32 = Color32::from_rgb(12, 12, 15);
    const TEXT: Color32 = Color32::from_rgb(244, 240, 236);
    const DIM: Color32 = Color32::from_rgb(130, 130, 136);
    const OFF: Color32 = Color32::from_rgb(86, 86, 92);

    /// The formats and the presets (codecs) each holds.
    const FORMATS: [(&str, &[Preset]); 7] = [
        ("MP4", &[Preset::H264Web, Preset::H265, Preset::Av1]),
        ("QuickTime", &[Preset::ProRes422, Preset::DnxHr]),
        ("WebM", &[Preset::Vp9]),
        ("Matroska", &[Preset::Ffv1]),
        ("WAV", &[Preset::Wav]),
        ("MP3", &[Preset::Mp3]),
        ("Image Sequence", &[Preset::PngSequence]),
    ];

    fn codec_name(p: Preset) -> &'static str {
        match p {
            Preset::H264Web => "H.264",
            Preset::H265 => "H.265",
            Preset::Av1 => "AV1",
            Preset::Vp9 => "VP9",
            Preset::ProRes422 => "Apple ProRes",
            Preset::DnxHr => "DNxHR",
            Preset::Ffv1 => "FFV1",
            Preset::Wav => "Linear PCM",
            Preset::Mp3 => "MP3",
            Preset::PngSequence => "PNG",
        }
    }

    /// Whether a preset makes a video file (not sound only, not a folder of pictures).
    fn video_file(preset: Preset) -> bool {
        !matches!(preset, Preset::Wav | Preset::Mp3 | Preset::PngSequence)
    }

    fn enabled_text(on: bool) -> Color32 {
        if on {
            TEXT
        } else {
            OFF
        }
    }

    fn label(p: &Painter, row: Rect, text: &str, on: bool) {
        p.text(
            pos2(row.left() + LABEL_RIGHT, row.center().y),
            Align2::RIGHT_CENTER,
            text,
            regular(18.0),
            if on {
                Color32::from_rgb(160, 160, 166)
            } else {
                OFF
            },
        );
    }

    fn control(row: Rect, width: f32) -> Rect {
        Rect::from_min_size(
            pos2(row.left() + CONTROL_LEFT, row.center().y - 15.0),
            vec2(width, 30.0),
        )
    }

    fn rule(p: &Painter, row: Rect) {
        p.hline(
            row.left() + 14.0..=row.right() - 6.0,
            row.center().y,
            Stroke::new(1.0, Color32::from_rgb(20, 20, 24)),
        );
    }

    /// A drop-down: the label, the current choice, and a menu of `options` (name, usable).
    /// Returns the choice made.
    #[allow(clippy::too_many_arguments)]
    fn combo(
        ui: &mut Ui,
        row: Rect,
        id: &str,
        name: &str,
        current: &str,
        options: &[(String, bool)],
        width: f32,
        on: bool,
    ) -> Option<usize> {
        let p = ui.painter().clone();
        label(&p, row, name, on);
        let rect = control(row, width);
        let response = ui.interact(
            rect,
            ui.id().with(("combo", id)),
            if on { Sense::click() } else { Sense::hover() },
        );
        let fill = if response.hovered() && on {
            Color32::from_rgb(44, 43, 49)
        } else {
            FIELD
        };
        p.rect_filled(rect, 2.0, fill);
        p.rect_stroke(rect, 2.0, Stroke::new(1.0, FIELD_LINE), StrokeKind::Inside);
        p.text(
            rect.left_center() + vec2(12.0, 0.0),
            Align2::LEFT_CENTER,
            current,
            regular(18.0),
            enabled_text(on),
        );
        paint(
            &p,
            rect.right_center() - vec2(14.0, 0.0),
            Icon::Chevron,
            if on { theme::ICON } else { OFF },
        );
        let mut picked = None;
        if on {
            egui::Popup::menu(&response).show(|ui| {
                ui.set_min_width(width.max(160.0));
                for (i, (text, usable)) in options.iter().enumerate() {
                    ui.add_enabled_ui(*usable, |ui| {
                        if ui
                            .selectable_label(text == current, text.as_str())
                            .clicked()
                        {
                            picked = Some(i);
                        }
                    });
                }
            });
        }
        picked
    }

    fn box_at(ui: &mut Ui, id: &str, rect: Rect, on: bool, checked: bool) -> Response {
        let p = ui.painter().clone();
        let r = ui.interact(
            rect,
            ui.id().with(("check", id)),
            if on { Sense::click() } else { Sense::hover() },
        );
        p.rect_filled(
            rect,
            3.0,
            if on {
                FIELD
            } else {
                Color32::from_rgb(30, 29, 34)
            },
        );
        p.rect_stroke(rect, 3.0, Stroke::new(1.0, FIELD_LINE), StrokeKind::Inside);
        if checked {
            let c = rect.center();
            let color = if on { theme::WHITE } else { OFF };
            p.line_segment(
                [c + vec2(-5.0, 0.0), c + vec2(-1.5, 4.0)],
                Stroke::new(2.0, color),
            );
            p.line_segment(
                [c + vec2(-1.5, 4.0), c + vec2(6.0, -5.0)],
                Stroke::new(2.0, color),
            );
        }
        r
    }

    /// A check box with its text; returns the new value when it was clicked.
    fn check(
        ui: &mut Ui,
        row: Rect,
        id: &str,
        x: f32,
        text: &str,
        value: bool,
        on: bool,
    ) -> Option<bool> {
        let rect = Rect::from_center_size(
            pos2(row.left() + x + 12.0, row.center().y),
            vec2(24.0, 24.0),
        );
        let r = box_at(ui, id, rect, on, value);
        ui.painter().text(
            pos2(rect.right() + 14.0, row.center().y),
            Align2::LEFT_CENTER,
            text,
            regular(18.0),
            enabled_text(on),
        );
        let hit = Rect::from_min_max(rect.min, pos2(rect.right() + 14.0 + 220.0, rect.bottom()));
        let click = ui.interact(
            hit,
            ui.id().with(("checkhit", id)),
            if on { Sense::click() } else { Sense::hover() },
        );
        (on && (r.clicked() || click.clicked())).then_some(!value)
    }

    /// A radio button with its text; true when it was clicked.
    fn radio(ui: &mut Ui, row: Rect, id: &str, x: f32, text: &str, value: bool, on: bool) -> bool {
        let p = ui.painter().clone();
        let c = pos2(row.left() + x + 12.0, row.center().y);
        let hit = Rect::from_min_max(
            pos2(c.x - 14.0, row.top() + 2.0),
            pos2(c.x + 28.0 + 7.0 * text.len() as f32, row.bottom() - 2.0),
        );
        let r = ui.interact(
            hit,
            ui.id().with(("radio", id)),
            if on { Sense::click() } else { Sense::hover() },
        );
        p.circle_filled(
            c,
            11.0,
            if on {
                FIELD
            } else {
                Color32::from_rgb(30, 29, 34)
            },
        );
        p.circle_stroke(c, 11.0, Stroke::new(1.0, FIELD_LINE));
        if value {
            p.circle_filled(c, 4.5, if on { theme::WHITE } else { OFF });
        }
        p.text(
            pos2(c.x + 22.0, c.y),
            Align2::LEFT_CENTER,
            text,
            regular(18.0),
            enabled_text(on),
        );
        on && r.clicked()
    }

    fn number(
        ui: &mut Ui,
        rect: Rect,
        value: &mut u32,
        range: std::ops::RangeInclusive<u32>,
        on: bool,
    ) -> bool {
        let p = ui.painter().clone();
        p.rect_filled(rect, 2.0, FIELD);
        p.rect_stroke(rect, 2.0, Stroke::new(1.0, FIELD_LINE), StrokeKind::Inside);
        ui.add_enabled_ui(on, |ui| {
            ui.put(
                rect.shrink2(vec2(2.0, 3.0)),
                egui::DragValue::new(value).range(range).speed(1.0),
            )
            .changed()
        })
        .inner
    }

    fn text_field(ui: &mut Ui, rect: Rect, text: &mut String, hint: &str) {
        ui.put(
            rect,
            egui::TextEdit::singleline(text)
                .font(regular(18.0))
                .hint_text(hint)
                .margin(vec2(10.0, 6.0))
                .desired_width(rect.width()),
        );
    }

    impl StudioApp {
        pub(crate) fn render_form(
            &mut self,
            ui: &mut Ui,
            ctx: &Context,
            s: &Screen,
            top: f32,
            bottom: f32,
        ) {
            let pane = s.r(LEFT, top + BAR, LEFT_RIGHT - 1.0, bottom);
            let usual = s.usual().r(LEFT, top + BAR, LEFT_RIGHT - 1.0, bottom);
            if pane.width() >= usual.width() - 0.5 {
                self.render_form_in(ui, ctx, pane);
                return;
            }
            // Its column was made narrower than the form: the form keeps its size and scrolls
            // sideways inside the column instead of running into the viewer.
            ui.scope_builder(UiBuilder::new().max_rect(pane), |ui| {
                egui::ScrollArea::horizontal()
                    .id_salt("render_form_narrow")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let size = vec2(usual.width(), pane.height() - 12.0);
                        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                        self.render_form_in(ui, ctx, rect);
                    });
            });
        }

        /// The form in `pane`.
        fn render_form_in(&mut self, ui: &mut Ui, ctx: &Context, pane: Rect) {
            let painter = ui.painter().clone();

            // The preset, always in view.
            let preset_row = Rect::from_min_size(pane.min, vec2(pane.width(), 96.0));
            painter.text(
                pos2(preset_row.left() + 88.0, preset_row.center().y),
                Align2::RIGHT_CENTER,
                "Preset",
                regular(19.0),
                Color32::from_rgb(160, 160, 166),
            );
            let box_rect = Rect::from_min_size(
                pos2(preset_row.left() + 102.0, preset_row.center().y - 27.0),
                vec2(392.0, 54.0),
            );
            let r = ui.interact(box_rect, ui.id().with("preset_box"), Sense::click());
            painter.rect_filled(
                box_rect,
                4.0,
                if r.hovered() {
                    Color32::from_rgb(40, 39, 45)
                } else {
                    FIELD
                },
            );
            painter.rect_stroke(
                box_rect,
                4.0,
                Stroke::new(1.0, FIELD_LINE),
                StrokeKind::Inside,
            );
            crate::render_presets::paint_badge(
                &painter,
                box_rect.left_center() + vec2(32.0, 0.0),
                self.preset_badge(),
            );
            painter.text(
                box_rect.left_center() + vec2(72.0, 0.0),
                Align2::LEFT_CENTER,
                &self.deliver_ui.preset_name,
                semibold(20.0),
                TEXT,
            );
            paint(
                &painter,
                box_rect.right_center() - vec2(16.0, 0.0),
                Icon::Chevron,
                theme::ICON,
            );
            egui::Popup::menu(&r)
                .width(box_rect.width())
                .show(|ui| self.preset_menu(ui));
            painter.hline(
                pane.x_range(),
                preset_row.bottom(),
                Stroke::new(1.0, Color32::from_rgb(14, 14, 18)),
            );

            // The footer: the size and the button.
            let footer = Rect::from_min_max(pos2(pane.left(), pane.bottom() - 74.0), pane.max);
            painter.rect_filled(footer, 0.0, theme::POOL_HEADER_BG);
            painter.hline(
                pane.x_range(),
                footer.top(),
                Stroke::new(1.0, Color32::from_rgb(14, 14, 18)),
            );
            painter.text(
                pos2(footer.left() + 40.0, footer.center().y),
                Align2::LEFT_CENTER,
                "Estimated File Size",
                regular(18.0),
                Color32::from_rgb(160, 160, 166),
            );
            let mb = self.estimated_mb();
            painter.text(
                pos2(footer.left() + 232.0, footer.center().y),
                Align2::LEFT_CENTER,
                if mb >= 1024.0 {
                    format!("{:.2} GB", mb / 1024.0)
                } else {
                    format!("{mb:.2} MB")
                },
                regular(19.0),
                theme::WHITE,
            );
            let can_add = self.project().duration() > 0;
            let button = Rect::from_min_size(
                pos2(footer.left() + 363.0, footer.center().y - 18.0),
                vec2(262.0, 36.0),
            );
            let r = ui.interact(
                button,
                ui.id().with("add_to_queue"),
                if can_add {
                    Sense::click()
                } else {
                    Sense::hover()
                },
            );
            painter.rect_filled(
                button,
                18.0,
                if can_add && r.hovered() {
                    Color32::from_rgb(52, 51, 58)
                } else {
                    Color32::from_rgb(38, 37, 43)
                },
            );
            painter.rect_stroke(
                button,
                18.0,
                Stroke::new(1.0, Color32::from_rgb(58, 57, 63)),
                StrokeKind::Inside,
            );
            painter.text(
                button.center(),
                Align2::CENTER_CENTER,
                "Add to Render Queue",
                regular(18.0),
                if can_add { TEXT } else { OFF },
            );
            if can_add
                && r.on_hover_text("Add a render of these settings to the queue")
                    .clicked()
            {
                self.add_to_queue(ctx);
            }

            // The settings, which scroll.
            let scroll = Rect::from_min_max(
                pos2(pane.left(), preset_row.bottom() + 1.0),
                pos2(pane.right(), footer.top()),
            );
            ui.scope_builder(UiBuilder::new().max_rect(scroll), |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                egui::ScrollArea::vertical()
                    .id_salt("render_settings")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let width = scroll.width() - 14.0;
                        let mut row = |ui: &mut Ui, height: f32| {
                            ui.allocate_exact_size(vec2(width, height), Sense::hover())
                                .0
                        };
                        self.form_top(ui, ctx, &mut row);
                        match self.deliver_ui.tab {
                            Tab::Video => self.form_video(ui, &mut row),
                            Tab::Audio => self.form_audio(ui, &mut row),
                            Tab::File => self.form_file(ui, &mut row),
                        }
                        row(ui, 16.0);
                    });
            });
        }

        /// The file name and place, how it is rendered, and the three tabs.
        fn form_top(
            &mut self,
            ui: &mut Ui,
            ctx: &Context,
            row: &mut dyn FnMut(&mut Ui, f32) -> Rect,
        ) {
            let p = ui.painter().clone();
            row(ui, 10.0);
            // File name.
            let r = row(ui, 46.0);
            label(&p, r, "File Name", true);
            let field = control(r, WIDE);
            let mut name = std::mem::take(&mut self.deliver_ui.file_name);
            text_field(
                ui,
                Rect::from_min_max(field.min, pos2(field.right() - 30.0, field.bottom())),
                &mut name,
                "Untitled",
            );
            self.deliver_ui.file_name = name;
            let plus =
                Rect::from_min_size(pos2(field.right() - 30.0, field.top()), vec2(30.0, 30.0));
            let pr = ui.interact(plus, ui.id().with("name_plus"), Sense::click());
            let c = if pr.hovered() {
                theme::ICON_HOVER
            } else {
                theme::ICON
            };
            p.line_segment(
                [
                    plus.center() - vec2(6.0, 0.0),
                    plus.center() + vec2(6.0, 0.0),
                ],
                Stroke::new(1.5, c),
            );
            p.line_segment(
                [
                    plus.center() - vec2(0.0, 6.0),
                    plus.center() + vec2(0.0, 6.0),
                ],
                Stroke::new(1.5, c),
            );
            let project_name = self.project().name.clone();
            let mut insert = None;
            egui::Popup::menu(&pr).show(|ui| {
                if ui.button("Project name").clicked() {
                    insert = Some(project_name.clone());
                }
                if ui.button("Preset name").clicked() {
                    insert = Some("Custom Export".to_string());
                }
            });
            if let Some(text) = insert {
                self.deliver_ui.file_name.push_str(&text);
            }
            // Location.
            let r = row(ui, 46.0);
            label(&p, r, "Location", true);
            let field = control(r, WIDE);
            let mut location = std::mem::take(&mut self.deliver_ui.location);
            text_field(ui, field, &mut location, "Choose a folder");
            self.deliver_ui.location = location;
            let browse = Rect::from_min_size(
                pos2(field.right() + 16.0, field.top() - 1.0),
                vec2(110.0, 32.0),
            );
            let br = ui.interact(browse, ui.id().with("browse"), Sense::click());
            p.rect_filled(
                browse,
                16.0,
                if br.hovered() {
                    Color32::from_rgb(52, 51, 58)
                } else {
                    Color32::from_rgb(38, 37, 43)
                },
            );
            p.rect_stroke(
                browse,
                16.0,
                Stroke::new(1.0, Color32::from_rgb(58, 57, 63)),
                StrokeKind::Inside,
            );
            p.text(
                browse.center(),
                Align2::CENTER_CENTER,
                "Browse",
                regular(18.0),
                TEXT,
            );
            if br.clicked() {
                self.deliver_ui.browsing = true;
                platform::pick_folder(self.inbox.clone(), ctx.clone());
            }
            let r = row(ui, 12.0);
            rule(&p, r);
            // Render: one file, or one for each clip.
            let r = row(ui, 62.0);
            label(&p, r, "Render", true);
            let individual = self.deliver_ui.individual;
            if radio(
                ui,
                r,
                "single",
                CONTROL_LEFT - 8.0,
                "Single clip",
                !individual,
                true,
            ) {
                self.deliver_ui.individual = false;
            }
            if radio(
                ui,
                r,
                "individual",
                CONTROL_LEFT + 140.0,
                "Individual clips",
                individual,
                true,
            ) {
                self.deliver_ui.individual = true;
            }
            let r = row(ui, 6.0);
            rule(&p, r);
            // The tabs.
            let r = row(ui, 56.0);
            let tabs = Rect::from_min_size(
                pos2(r.left() + 86.0, r.center().y - 17.0),
                vec2(462.0, 34.0),
            );
            p.rect_filled(tabs, 2.0, FIELD);
            p.rect_stroke(tabs, 2.0, Stroke::new(1.0, FIELD_LINE), StrokeKind::Inside);
            for (n, (tab, name)) in [
                (Tab::Video, "Video"),
                (Tab::Audio, "Audio"),
                (Tab::File, "File"),
            ]
            .into_iter()
            .enumerate()
            {
                let seg =
                    Rect::from_min_size(tabs.min + vec2(n as f32 * 154.0, 0.0), vec2(154.0, 34.0));
                let tr = ui.interact(seg, ui.id().with(("tab", name)), Sense::click());
                if tr.clicked() {
                    self.deliver_ui.tab = tab;
                }
                let on = self.deliver_ui.tab == tab;
                if on {
                    p.rect_filled(seg, 2.0, Color32::from_rgb(20, 20, 23));
                }
                p.text(
                    seg.center(),
                    Align2::CENTER_CENTER,
                    name,
                    regular(19.0),
                    if on {
                        theme::WHITE
                    } else {
                        Color32::from_rgb(170, 170, 176)
                    },
                );
            }
        }

        fn form_video(&mut self, ui: &mut Ui, row: &mut dyn FnMut(&mut Ui, f32) -> Rect) {
            let p = ui.painter().clone();
            let preset = self.deliver.preset;
            let is_video_preset = !preset.is_audio_only();
            let is_file = video_file(preset);

            let r = row(ui, 52.0);
            if let Some(v) = check(
                ui,
                r,
                "export_video",
                14.0,
                "Export Video",
                self.deliver.options.video && is_video_preset,
                is_video_preset && is_file,
            ) {
                self.deliver.options.video = v;
            }
            let r = row(ui, 12.0);
            rule(&p, r);

            // Format and codec.
            let format_now = FORMATS
                .iter()
                .position(|(_, ps)| ps.contains(&preset))
                .unwrap_or(0);
            let r = row(ui, 44.0);
            let formats: Vec<(String, bool)> = FORMATS
                .iter()
                .map(|(n, ps)| (n.to_string(), ps.iter().any(|p| p.available())))
                .collect();
            if let Some(i) = combo(
                ui,
                r,
                "format",
                "Format",
                FORMATS[format_now].0,
                &formats,
                WIDE,
                true,
            ) {
                if let Some(first) = FORMATS[i].1.iter().find(|p| p.available()) {
                    self.deliver.preset = *first;
                }
            }
            let r = row(ui, 44.0);
            let codecs: Vec<(String, bool)> = FORMATS[format_now]
                .1
                .iter()
                .map(|p| (codec_name(*p).to_string(), p.available()))
                .collect();
            if let Some(i) = combo(
                ui,
                r,
                "codec",
                "Codec",
                codec_name(preset),
                &codecs,
                WIDE,
                true,
            ) {
                if let Some(p) = FORMATS[format_now].1.get(i) {
                    self.deliver.preset = *p;
                }
            }
            // The processor's encoder or a graphics card's: the ones this FFmpeg has, those that did
            // not open on this computer greyed out.
            let r = row(ui, 44.0);
            let choices = preset.hardware_choices();
            // A codec without a card's encoder (ProRes ...) shows Native but keeps the choice
            // for when an H.264, H.265 or AV1 codec is chosen again.
            let hardware = if choices.contains(&self.deliver.options.hardware) {
                self.deliver.options.hardware
            } else {
                if choices.len() > 1 {
                    self.deliver.options.hardware = Hardware::Native;
                }
                Hardware::Native
            };
            let list: Vec<(String, bool)> = choices
                .iter()
                .map(|h| {
                    let works = picasso_export::hardware_works(preset, *h) != Some(false);
                    (h.label().to_string(), works)
                })
                .collect();
            if let Some(i) = combo(
                ui,
                r,
                "encoder",
                "Encoder",
                hardware.label(),
                &list,
                WIDE,
                is_file && choices.len() > 1,
            ) {
                if let Some(h) = choices.get(i) {
                    self.deliver.options.hardware = *h;
                    // The cards make no 10-bit H.264.
                    if *h != Hardware::Native && preset == Preset::H264Web {
                        self.deliver.options.ten_bit = false;
                    }
                }
            }
            let r = row(ui, 44.0);
            let mp4 = matches!(preset.extension(), "mp4" | "mov");
            if let Some(v) = check(
                ui,
                r,
                "network",
                CONTROL_LEFT - 8.0,
                "Network Optimization",
                self.deliver.options.faststart && mp4,
                is_file && mp4,
            ) {
                self.deliver.options.faststart = v;
            }
            let r = row(ui, 14.0);
            rule(&p, r);

            // Resolution and frame rate.
            let r = row(ui, 44.0);
            let mut names: Vec<(String, bool)> = vec![("Timeline Resolution".to_string(), true)];
            names.extend(RESOLUTIONS.iter().map(|(n, _, _)| (n.to_string(), true)));
            names.push(("Custom".to_string(), true));
            let mode = self.deliver_ui.res_mode.min(names.len() - 1);
            if let Some(i) = combo(
                ui,
                r,
                "resolution",
                "Resolution",
                &names[mode].0,
                &names,
                WIDE,
                is_video_preset,
            ) {
                self.deliver_ui.res_mode = i;
                if i == names.len() - 1 {
                    let (w, h) = (self.project().width, self.project().height);
                    self.deliver_ui.custom = (w, h);
                }
            }
            let custom = self.deliver_ui.res_mode == names.len() - 1;
            let (w, h) = match self.deliver_ui.res_mode {
                0 => (self.project().width, self.project().height),
                n if n <= RESOLUTIONS.len() => (RESOLUTIONS[n - 1].1, RESOLUTIONS[n - 1].2),
                _ => self.deliver_ui.custom,
            };
            let r = row(ui, 44.0);
            let (mut cw, mut ch) = (w, h);
            let left = pos2(r.left() + CONTROL_LEFT, r.center().y - 15.0);
            let changed_w = number(
                ui,
                Rect::from_min_size(left, vec2(100.0, 30.0)),
                &mut cw,
                16..=16_384,
                custom && !self.deliver_ui.vertical,
            );
            p.text(
                pos2(left.x + 126.0, r.center().y),
                Align2::CENTER_CENTER,
                "x",
                regular(18.0),
                DIM,
            );
            let changed_h = number(
                ui,
                Rect::from_min_size(left + vec2(152.0, 0.0), vec2(100.0, 30.0)),
                &mut ch,
                16..=16_384,
                custom,
            );
            if custom && (changed_w || changed_h) {
                self.deliver_ui.custom = (cw, ch);
            }
            let r = row(ui, 44.0);
            if let Some(v) = check(
                ui,
                r,
                "vertical",
                CONTROL_LEFT - 8.0,
                "Use vertical resolution",
                self.deliver_ui.vertical,
                custom,
            ) {
                self.deliver_ui.vertical = v;
            }
            let r = row(ui, 44.0);
            let mut rates: Vec<(String, bool)> = vec![("Timeline Frame Rate".to_string(), true)];
            rates.extend(
                picasso_core::Fps::PRESETS
                    .iter()
                    .map(|f| (fps_label(*f), true)),
            );
            let fps_now = self.deliver_ui.fps_choice.min(rates.len() - 1);
            if let Some(i) = combo(
                ui,
                r,
                "fps",
                "Frame rate",
                &rates[fps_now].0,
                &rates,
                WIDE,
                is_video_preset,
            ) {
                self.deliver_ui.fps_choice = i;
            }
            let r = row(ui, 40.0);
            let shown = match fps_now {
                0 => self.project().fps,
                n => picasso_core::Fps::PRESETS
                    .get(n - 1)
                    .copied()
                    .unwrap_or(self.project().fps),
            };
            p.text(
                pos2(r.left() + CONTROL_LEFT + 4.0, r.center().y),
                Align2::LEFT_CENTER,
                format!("{} frames per second", fps_label(shown)),
                regular(18.0),
                Color32::from_rgb(160, 160, 166),
            );
            let r = row(ui, 40.0);
            if let Some(v) = check(
                ui,
                r,
                "chapters",
                CONTROL_LEFT - 8.0,
                "Chapters from Markers",
                self.deliver.options.chapters,
                is_file,
            ) {
                self.deliver.options.chapters = v;
            }
            let r = row(ui, 14.0);
            rule(&p, r);

            // The encoder's settings.
            let encodes = matches!(
                preset,
                Preset::H264Web | Preset::H265 | Preset::Av1 | Preset::Vp9
            );
            let on_card = self.deliver.options.hardware != Hardware::Native;
            let ten = self.deliver.options.ten_bit;
            let r = row(ui, 44.0);
            // The profiles of the codec; for H.264, H.265, AV1 and VP9 the last is the 10-bit one.
            let profiles: Vec<(String, bool)> = match preset {
                Preset::ProRes422 => [
                    "ProRes 422 Proxy",
                    "ProRes 422 LT",
                    "ProRes 422",
                    "ProRes 422 HQ",
                ]
                .iter()
                .map(|n| (n.to_string(), true))
                .collect(),
                Preset::DnxHr => DNXHR_PROFILES
                    .iter()
                    .map(|(n, _)| (n.to_string(), true))
                    .collect(),
                Preset::H264Web => Profile::ALL
                    .iter()
                    .map(|p| (p.label().to_string(), true))
                    .chain([("High 10".to_string(), !on_card)])
                    .collect(),
                Preset::H265 => vec![("Main".into(), true), ("Main 10".into(), true)],
                Preset::Av1 => vec![("Main, 8-bit".into(), true), ("Main, 10-bit".into(), true)],
                Preset::Vp9 => vec![
                    ("Profile 0, 8-bit".into(), true),
                    ("Profile 2, 10-bit".into(), true),
                ],
                Preset::Ffv1 => vec![("Level 3, 10-bit 4:4:4".into(), true)],
                _ => vec![("Auto".into(), true)],
            };
            let current = match preset {
                Preset::ProRes422 => usize::from(self.deliver.options.prores),
                Preset::DnxHr => usize::from(self.deliver.options.dnxhr),
                Preset::H264Web if ten => Profile::ALL.len(),
                Preset::H264Web => Profile::ALL
                    .iter()
                    .position(|p| *p == self.deliver.options.profile)
                    .unwrap_or(0),
                Preset::H265 | Preset::Av1 | Preset::Vp9 => usize::from(ten),
                _ => 0,
            }
            .min(profiles.len().saturating_sub(1));
            let shown = profiles.get(current).map_or("Auto", |(n, _)| n.as_str());
            if let Some(i) = combo(
                ui,
                r,
                "profile",
                "Encoding Profile",
                shown,
                &profiles,
                240.0,
                is_file && profiles.len() > 1,
            ) {
                let o = &mut self.deliver.options;
                match preset {
                    Preset::ProRes422 => o.prores = i as u8,
                    Preset::DnxHr => o.dnxhr = i as u8,
                    Preset::H264Web => match Profile::ALL.get(i) {
                        Some(pr) => {
                            o.profile = *pr;
                            o.ten_bit = false;
                        }
                        None => {
                            o.profile = Profile::Auto;
                            o.ten_bit = true;
                        }
                    },
                    Preset::H265 | Preset::Av1 | Preset::Vp9 => o.ten_bit = i == 1,
                    _ => {}
                }
            }
            // Key frames.
            let r = row(ui, 34.0);
            label(&p, r, "Key Frames", encodes);
            let auto = self.deliver_ui.key_auto;
            if radio(
                ui,
                r,
                "key_auto",
                CONTROL_LEFT - 8.0,
                "Automatic",
                auto,
                encodes,
            ) {
                self.deliver_ui.key_auto = true;
            }
            let r = row(ui, 34.0);
            if radio(
                ui,
                r,
                "key_every",
                CONTROL_LEFT - 8.0,
                "Every",
                !auto,
                encodes,
            ) {
                self.deliver_ui.key_auto = false;
            }
            let mut every = self.deliver_ui.every;
            let field = Rect::from_min_size(
                pos2(r.left() + CONTROL_LEFT + 86.0, r.center().y - 14.0),
                vec2(80.0, 28.0),
            );
            if number(ui, field, &mut every, 1..=600, encodes && !auto) {
                self.deliver_ui.every = every;
            }
            p.text(
                pos2(field.right() + 14.0, r.center().y),
                Align2::LEFT_CENTER,
                "frames",
                regular(18.0),
                enabled_text(encodes && !auto),
            );
            let r = row(ui, 40.0);
            if let Some(v) = check(
                ui,
                r,
                "reorder",
                CONTROL_LEFT - 8.0,
                "Frame reordering",
                self.deliver.options.frame_reordering,
                encodes,
            ) {
                self.deliver.options.frame_reordering = v;
            }
            row(ui, 14.0);
            // Speed, rate control and quality.
            let r = row(ui, 44.0);
            let speeds = [
                ("Faster", Speed::Faster),
                ("Balanced", Speed::Balanced),
                ("Best", Speed::Best),
            ];
            let current = speeds
                .iter()
                .find(|(_, s)| *s == self.deliver.options.speed)
                .map_or("Balanced", |(n, _)| *n);
            let list: Vec<(String, bool)> =
                speeds.iter().map(|(n, _)| (n.to_string(), true)).collect();
            if let Some(i) = combo(ui, r, "speed", "Preset", current, &list, 188.0, encodes) {
                self.deliver.options.speed = speeds[i].1;
            }
            let r = row(ui, 44.0);
            let by_quality = matches!(self.deliver.options.rate, Rate::Quality(_));
            let modes = [
                ("Quality".to_string(), true),
                ("Restrict to".to_string(), true),
            ];
            let current = if by_quality { "Quality" } else { "Restrict to" };
            if let Some(i) = combo(
                ui,
                r,
                "rate",
                "Rate Control",
                current,
                &modes,
                218.0,
                encodes,
            ) {
                self.deliver.options.rate = if i == 0 {
                    Rate::Quality(Quality::Best)
                } else {
                    Rate::Bitrate(8000)
                };
            }
            let r = row(ui, 44.0);
            match self.deliver.options.rate {
                Rate::Quality(q) => {
                    let list: Vec<(String, bool)> = Quality::ALL
                        .iter()
                        .map(|q| (q.label().to_string(), true))
                        .collect();
                    if let Some(i) = combo(
                        ui,
                        r,
                        "quality",
                        "Quality",
                        q.label(),
                        &list,
                        188.0,
                        encodes,
                    ) {
                        self.deliver.options.rate = Rate::Quality(Quality::ALL[i]);
                    }
                }
                Rate::Bitrate(kbps) => {
                    label(&p, r, "Bitrate", encodes);
                    let mut value = kbps;
                    let field = Rect::from_min_size(
                        pos2(r.left() + CONTROL_LEFT, r.center().y - 15.0),
                        vec2(120.0, 30.0),
                    );
                    if number(ui, field, &mut value, 100..=500_000, encodes) {
                        self.deliver.options.rate = Rate::Bitrate(value);
                    }
                    p.text(
                        pos2(field.right() + 14.0, r.center().y),
                        Align2::LEFT_CENTER,
                        "Kb/s",
                        regular(18.0),
                        enabled_text(encodes),
                    );
                }
            }
            let r = row(ui, 14.0);
            rule(&p, r);
            // Advanced and subtitles.
            let r = row(ui, 44.0);
            self.section(ui, r, "Advanced Settings", true);
            if self.deliver_ui.advanced_open {
                self.form_advanced(ui, row);
            }
            let r = row(ui, 44.0);
            self.section(ui, r, "Subtitle Settings", false);
            if self.deliver_ui.subtitle_open {
                self.form_subtitles(ui, row);
            }
        }

        /// The Advanced Settings. The shape of a pixel, the levels, the colour and gamma tags, the
        /// burn-in and the script change the file; the ones that stand for media or passes this
        /// version does not have are kept and change nothing.
        fn form_advanced(&mut self, ui: &mut Ui, row: &mut dyn FnMut(&mut Ui, f32) -> Rect) {
            let p = ui.painter().clone();
            let x = CONTROL_LEFT - 8.0;
            let video = !self.deliver.preset.is_audio_only();
            let is_file = video_file(self.deliver.preset);
            let on = video && is_file;
            row(ui, 6.0);
            // The shape of a pixel.
            let r = row(ui, 38.0);
            label(&p, r, "Pixel aspect ratio", on);
            let cine = self.deliver_ui.cinemascope;
            if radio(ui, r, "pa_square", x, "Square", !cine, on) {
                self.deliver_ui.cinemascope = false;
            }
            let r = row(ui, 38.0);
            if radio(ui, r, "pa_cine", x, "Cinemascope", cine, on) {
                self.deliver_ui.cinemascope = true;
            }
            // The levels.
            let levels = self.deliver_ui.levels;
            for (n, name) in ["Auto", "Video", "Full"].into_iter().enumerate() {
                let r = row(ui, 38.0);
                if n == 0 {
                    label(&p, r, "Data Levels", on);
                }
                if radio(ui, r, name, x, name, levels == n, on) {
                    self.deliver_ui.levels = n;
                }
            }
            let r = row(ui, 44.0);
            if let Some(v) = check(
                ui,
                r,
                "retain",
                x,
                "Retain sub-black and super-white data",
                self.deliver_ui.retain_blacks,
                on,
            ) {
                self.deliver_ui.retain_blacks = v;
            }
            // The tags.
            let r = row(ui, 46.0);
            let list: Vec<(String, bool)> = crate::deliver::COLOR_TAGS
                .iter()
                .map(|(n, _)| (n.to_string(), true))
                .collect();
            let tag = self.deliver_ui.color_tag.min(list.len() - 1);
            if let Some(i) = combo(
                ui,
                r,
                "color_tag",
                "Color Space Tag",
                &list[tag].0,
                &list,
                WIDE,
                on,
            ) {
                self.deliver_ui.color_tag = i;
            }
            let r = row(ui, 46.0);
            let list: Vec<(String, bool)> = crate::deliver::GAMMA_TAGS
                .iter()
                .map(|(n, _)| (n.to_string(), true))
                .collect();
            let tag = self.deliver_ui.gamma_tag.min(list.len() - 1);
            if let Some(i) = combo(
                ui,
                r,
                "gamma_tag",
                "Gamma Tag",
                &list[tag].0,
                &list,
                WIDE,
                on,
            ) {
                self.deliver_ui.gamma_tag = i;
            }
            let r = row(ui, 14.0);
            rule(&p, r);
            // Burn-in.
            let r = row(ui, 46.0);
            let list: Vec<(String, bool)> = crate::deliver::BURN_INS
                .iter()
                .map(|n| (n.to_string(), true))
                .collect();
            let burn = self.deliver_ui.burn.min(list.len() - 1);
            if let Some(i) = combo(
                ui,
                r,
                "burn",
                "Data burn-in",
                &list[burn].0,
                &list,
                220.0,
                on,
            ) {
                self.deliver_ui.burn = i;
            }
            let r = row(ui, 14.0);
            rule(&p, r);
            // What is used.
            let boxes: [(&str, &str); 5] = [
                ("bypass", "Bypass re-encode when possible"),
                ("proxy", "Use proxy media"),
                ("cached", "Use render cached images"),
                ("sizing", "Force sizing to highest quality"),
                ("debayer", "Force debayer to highest quality"),
            ];
            for (id, text) in boxes {
                let r = row(ui, 52.0);
                let value = match id {
                    "bypass" => self.deliver_ui.bypass_reencode,
                    "proxy" => self.deliver_ui.proxy_media,
                    "cached" => self.deliver_ui.cached_images,
                    "sizing" => self.deliver_ui.force_sizing,
                    _ => self.deliver_ui.force_debayer,
                };
                if let Some(v) = check(ui, r, id, x, text, value, true) {
                    match id {
                        "bypass" => self.deliver_ui.bypass_reencode = v,
                        "proxy" => self.deliver_ui.proxy_media = v,
                        "cached" => self.deliver_ui.cached_images = v,
                        "sizing" => self.deliver_ui.force_sizing = v,
                        _ => self.deliver_ui.force_debayer = v,
                    }
                }
            }
            let r = row(ui, 14.0);
            rule(&p, r);
            // Every video track is always rendered.
            let r = row(ui, 52.0);
            let _ = check(
                ui,
                r,
                "all_tracks",
                x,
                "Render All Video Tracks",
                true,
                false,
            );
            let r = row(ui, 14.0);
            rule(&p, r);
            // Passes.
            let off_on = [("Off".to_string(), true), ("On".to_string(), true)];
            let r = row(ui, 46.0);
            let now = usize::from(self.deliver_ui.flat_pass);
            if let Some(i) = combo(
                ui,
                r,
                "flat",
                "Enable Flat Pass",
                &off_on[now].0,
                &off_on,
                220.0,
                true,
            ) {
                self.deliver_ui.flat_pass = i == 1;
            }
            let r = row(ui, 46.0);
            let now = usize::from(self.deliver_ui.vision_bypass);
            if let Some(i) = combo(
                ui,
                r,
                "vision",
                "visionOS Bypass",
                &off_on[now].0,
                &off_on,
                WIDE,
                true,
            ) {
                self.deliver_ui.vision_bypass = i == 1;
            }
            let r = row(ui, 44.0);
            if let Some(v) = check(
                ui,
                r,
                "no_sizing",
                x,
                "Disable sizing and blanking output",
                self.deliver_ui.disable_sizing,
                true,
            ) {
                self.deliver_ui.disable_sizing = v;
            }
            let r = row(ui, 14.0);
            rule(&p, r);
            // A program at the start or end of the job.
            let r = row(ui, 56.0);
            if let Some(v) = check(
                ui,
                r,
                "script",
                x,
                "Trigger script at",
                self.deliver_ui.script_on,
                true,
            ) {
                self.deliver_ui.script_on = v;
            }
            let at = Rect::from_min_size(
                pos2(r.left() + CONTROL_LEFT + 218.0, r.center().y - 15.0),
                vec2(130.0, 30.0),
            );
            let list = [("Start".to_string(), true), ("End".to_string(), true)];
            let now = usize::from(self.deliver_ui.script_at_end);
            let rr = ui.interact(
                at,
                ui.id().with("script_at"),
                if self.deliver_ui.script_on {
                    Sense::click()
                } else {
                    Sense::hover()
                },
            );
            p.rect_filled(at, 2.0, FIELD);
            p.rect_stroke(at, 2.0, Stroke::new(1.0, FIELD_LINE), StrokeKind::Inside);
            p.text(
                at.left_center() + vec2(12.0, 0.0),
                Align2::LEFT_CENTER,
                &list[now].0,
                regular(18.0),
                enabled_text(self.deliver_ui.script_on),
            );
            paint(
                &p,
                at.right_center() - vec2(14.0, 0.0),
                Icon::Chevron,
                if self.deliver_ui.script_on {
                    theme::ICON
                } else {
                    OFF
                },
            );
            let mut picked = None;
            if self.deliver_ui.script_on {
                egui::Popup::menu(&rr).show(|ui| {
                    for (i, (text, _)) in list.iter().enumerate() {
                        if ui.selectable_label(i == now, text.as_str()).clicked() {
                            picked = Some(i);
                        }
                    }
                });
            }
            if let Some(i) = picked {
                self.deliver_ui.script_at_end = i == 1;
            }
            p.text(
                pos2(at.right() + 14.0, r.center().y),
                Align2::LEFT_CENTER,
                "of render job",
                regular(18.0),
                enabled_text(self.deliver_ui.script_on),
            );
            if self.deliver_ui.script_on {
                let r = row(ui, 46.0);
                label(&p, r, "Program", true);
                let field = control(r, WIDE + 130.0);
                let mut path = std::mem::take(&mut self.deliver_ui.script_path);
                text_field(ui, field, &mut path, "Path of a program or script");
                self.deliver_ui.script_path = path;
            }
            row(ui, 10.0);
        }

        /// The Subtitle Settings: the titles of the timeline as subtitles, in a file of their
        /// own next to the render, or in the picture (where titles already are).
        fn form_subtitles(&mut self, ui: &mut Ui, row: &mut dyn FnMut(&mut Ui, f32) -> Rect) {
            use crate::subtitles::SubtitleKind;
            const FORMATS: [&str; 3] = [
                "As a separate file",
                "Burn into video",
                "As embedded captions",
            ];
            let p = ui.painter().clone();
            let r = row(ui, 52.0);
            if let Some(v) = check(
                ui,
                r,
                "export_subtitle",
                14.0,
                "Export Subtitle",
                self.deliver_ui.export_subtitle,
                true,
            ) {
                self.deliver_ui.export_subtitle = v;
            }
            let r = row(ui, 12.0);
            rule(&p, r);
            let on = self.deliver_ui.export_subtitle;
            row(ui, 8.0);
            let r = row(ui, 44.0);
            let format = self.deliver_ui.subtitle_format.min(FORMATS.len() - 1);
            // Embedded captions need a container that carries them; this version writes none.
            let list: Vec<(String, bool)> = FORMATS
                .iter()
                .enumerate()
                .map(|(i, f)| (f.to_string(), i < 2))
                .collect();
            if let Some(i) = combo(
                ui,
                r,
                "subtitle_format",
                "Format",
                FORMATS[format],
                &list,
                WIDE,
                on,
            ) {
                self.deliver_ui.subtitle_format = i;
            }
            let r = row(ui, 44.0);
            let kind = self.deliver_ui.subtitle_kind;
            let separate = format == 0;
            let list: Vec<(String, bool)> = SubtitleKind::ALL
                .iter()
                .map(|k| (k.label().to_string(), true))
                .collect();
            if let Some(i) = combo(
                ui,
                r,
                "subtitle_kind",
                "Export As",
                kind.label(),
                &list,
                WIDE,
                on && separate,
            ) {
                if let Some(k) = SubtitleKind::ALL.get(i) {
                    self.deliver_ui.subtitle_kind = *k;
                }
            }
            row(ui, 10.0);
        }

        /// A heading with a line after it that opens and closes.
        fn section(&mut self, ui: &mut Ui, r: Rect, name: &str, advanced: bool) {
            let p = ui.painter().clone();
            let open = if advanced {
                self.deliver_ui.advanced_open
            } else {
                self.deliver_ui.subtitle_open
            };
            let hit = Rect::from_min_size(r.min, vec2(r.width(), r.height()));
            let resp = ui.interact(hit, ui.id().with(("section", name)), Sense::click());
            paint(
                &p,
                pos2(r.left() + 22.0, r.center().y),
                if open { Icon::Chevron } else { Icon::ChevRight },
                theme::ICON,
            );
            let galley = p.layout_no_wrap(
                name.to_string(),
                regular(18.0),
                if resp.hovered() {
                    theme::WHITE
                } else {
                    Color32::from_rgb(160, 160, 166)
                },
            );
            let width = galley.size().x;
            p.galley(
                pos2(r.left() + 44.0, r.center().y - galley.size().y / 2.0),
                galley,
                TEXT,
            );
            p.hline(
                r.left() + 56.0 + width..=r.right() - 6.0,
                r.center().y,
                Stroke::new(1.0, Color32::from_rgb(40, 40, 46)),
            );
            if resp.clicked() {
                if advanced {
                    self.deliver_ui.advanced_open = !open;
                } else {
                    self.deliver_ui.subtitle_open = !open;
                }
            }
        }

        fn form_audio(&mut self, ui: &mut Ui, row: &mut dyn FnMut(&mut Ui, f32) -> Rect) {
            let p = ui.painter().clone();
            let preset = self.deliver.preset;
            let has_sound = !matches!(preset, Preset::PngSequence);
            let r = row(ui, 52.0);
            if let Some(v) = check(
                ui,
                r,
                "export_audio",
                14.0,
                "Export Audio",
                self.deliver.options.audio && has_sound,
                has_sound,
            ) {
                self.deliver.options.audio = v;
            }
            let r = row(ui, 12.0);
            rule(&p, r);
            let on = has_sound && self.deliver.options.audio;
            // The codec: each file type has its own, and a QuickTime file AAC or linear PCM.
            let r = row(ui, 44.0);
            let mov = matches!(preset, Preset::ProRes422 | Preset::DnxHr);
            let pcm_now = mov && self.deliver.options.audio_codec != AudioCodec::Aac;
            let shown = match preset {
                Preset::Wav => "Linear PCM",
                Preset::Mp3 => "MP3",
                Preset::Vp9 => "Opus",
                Preset::Ffv1 => "FLAC",
                _ if pcm_now => "Linear PCM",
                _ => "AAC",
            };
            let list: Vec<(String, bool)> = if mov {
                vec![("AAC".to_string(), true), ("Linear PCM".to_string(), true)]
            } else {
                vec![(shown.to_string(), true)]
            };
            if let Some(i) = combo(ui, r, "acodec", "Codec", shown, &list, WIDE, on && mov) {
                self.deliver.options.audio_codec = if i == 0 {
                    AudioCodec::Aac
                } else {
                    AudioCodec::Pcm
                };
            }
            // The data rate of the codecs that have one.
            let r = row(ui, 44.0);
            let rates = [128u32, 192, 256, 320];
            let list: Vec<(String, bool)> =
                rates.iter().map(|k| (format!("{k} kbps"), true)).collect();
            let current = format!("{} kbps", self.deliver.options.audio_kbps);
            let has_rate = on && matches!(shown, "AAC" | "Opus" | "MP3");
            if let Some(i) = combo(ui, r, "arate", "Data Rate", &current, &list, WIDE, has_rate) {
                if let Some(k) = rates.get(i) {
                    self.deliver.options.audio_kbps = *k;
                }
            }
            let r = row(ui, 44.0);
            label(&p, r, "Output Track 1", on);
            p.text(
                pos2(r.left() + CONTROL_LEFT + 4.0, r.center().y),
                Align2::LEFT_CENTER,
                "Main, stereo, 48 kHz",
                regular(18.0),
                enabled_text(on),
            );
        }

        fn form_file(&mut self, ui: &mut Ui, row: &mut dyn FnMut(&mut Ui, f32) -> Rect) {
            let p = ui.painter().clone();
            let r = row(ui, 52.0);
            if let Some(v) = check(
                ui,
                r,
                "unique",
                14.0,
                "Use unique filename",
                self.deliver_ui.unique,
                true,
            ) {
                self.deliver_ui.unique = v;
            }
            let r = row(ui, 40.0);
            p.text(
                pos2(r.left() + 52.0, r.center().y),
                Align2::LEFT_CENTER,
                "Adds (1), (2) ... to the name instead of writing over a file.",
                regular(17.0),
                Color32::from_rgb(160, 160, 166),
            );
            let r = row(ui, 12.0);
            rule(&p, r);
            let r = row(ui, 44.0);
            label(&p, r, "File type", true);
            let ext = match self.deliver.preset.extension() {
                "" => "folder of PNG pictures".to_string(),
                e => format!(".{e}"),
            };
            p.text(
                pos2(r.left() + CONTROL_LEFT + 4.0, r.center().y),
                Align2::LEFT_CENTER,
                ext,
                regular(18.0),
                TEXT,
            );
            let r = row(ui, 44.0);
            label(&p, r, "Range", true);
            let (marks, whole) = (
                self.project().mark_in.is_some() || self.project().mark_out.is_some(),
                self.range_is_whole(),
            );
            let r2 = r;
            if radio(
                ui,
                r2,
                "range_whole",
                CONTROL_LEFT - 8.0,
                "Whole timeline",
                whole,
                true,
            ) {
                self.set_range_whole(true);
            }
            let r = row(ui, 44.0);
            if radio(
                ui,
                r,
                "range_marks",
                CONTROL_LEFT - 8.0,
                "In to out marks",
                !whole,
                marks,
            ) {
                self.set_range_whole(false);
            }
        }
    }

    fn fps_label(f: picasso_core::Fps) -> String {
        if f == picasso_core::Fps::FPS_2997 {
            "29.97".to_string()
        } else {
            format!("{}", f.as_f64())
        }
    }
}
