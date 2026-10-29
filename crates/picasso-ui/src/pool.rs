//! The media pool: thumbnails of the media, and the Media page.

use std::collections::HashMap;

use egui::{
    vec2, Color32, ColorImage, Context, Rect, Sense, StrokeKind, TextureHandle, TextureId,
    TextureOptions, Ui,
};
use picasso_core::{Command, MediaId, MediaItem, MediaSource};
use picasso_render::{render_thumbnail_exact, MediaCache};

use crate::app::StudioApp;
use crate::theme;

const THUMB: egui::Vec2 = vec2(112.0, 63.0);

/// How the pool orders its media.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PoolSort {
    /// In the order they were added.
    #[default]
    Added,
    Name,
    Kind,
    Duration,
}

impl PoolSort {
    pub(crate) fn label(self) -> &'static str {
        match self {
            PoolSort::Added => "Date Added",
            PoolSort::Name => "Clip Name",
            PoolSort::Kind => "Type",
            PoolSort::Duration => "Duration",
        }
    }

    fn next(self) -> Self {
        match self {
            PoolSort::Added => PoolSort::Name,
            PoolSort::Name => PoolSort::Kind,
            PoolSort::Kind => PoolSort::Duration,
            PoolSort::Duration => PoolSort::Added,
        }
    }
}

/// The bin the pool shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum BinView {
    /// Every media item.
    #[default]
    Master,
    Bin(u64),
    Smart(u64),
}

/// Thumbnails or a list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PoolView {
    #[default]
    Thumbnails,
    List,
}

/// The type of a media item, as the pool names it.
pub(crate) fn kind_name(source: &MediaSource) -> &'static str {
    picasso_core::media_kind(source)
}

struct Thumb {
    source: MediaSource,
    generation: u64,
    /// Whether the picture was final. One still waiting for a video picture is made again when
    /// one arrives (`video`, the video generation it was made at changes).
    exact: bool,
    video: u64,
    texture: TextureHandle,
}

pub(crate) struct Pool {
    thumbs: HashMap<MediaId, Thumb>,
    solid: [u8; 3],
    from: [u8; 3],
    to: [u8; 3],
    /// The size of the thumbnails, 0 … 1; the Media page's slider sets it.
    pub(crate) zoom: f32,
    /// The search field is shown, and what it holds (names containing it are shown).
    pub(crate) searching: bool,
    pub(crate) search: String,
    /// The search field takes the keys on the next frame (it was just opened).
    focus_search: bool,
    pub(crate) sort: PoolSort,
    pub(crate) view: PoolView,
    pub(crate) bin: BinView,
}

impl Default for Pool {
    fn default() -> Self {
        Self {
            thumbs: HashMap::new(),
            solid: [20, 140, 150],
            from: [250, 120, 40],
            to: [60, 20, 110],
            zoom: 0.333,
            searching: false,
            search: String::new(),
            focus_search: false,
            sort: PoolSort::Added,
            view: PoolView::Thumbnails,
            bin: BinView::Master,
        }
    }
}

impl Pool {
    pub(crate) fn solid_color(&self) -> [u8; 3] {
        self.solid
    }

    pub(crate) fn gradient_colors(&self) -> ([u8; 3], [u8; 3]) {
        (self.from, self.to)
    }

    /// The thumbnail of a media item, made again when the item or the images changed.
    fn thumb(
        &mut self,
        ctx: &Context,
        project: &picasso_core::Project,
        item: &MediaItem,
        cache: &MediaCache,
        generation: u64,
    ) -> Option<TextureId> {
        let video = cache.thumbnail_generation();
        if let Some(t) = self.thumbs.get(&item.id) {
            if t.source == item.source
                && t.generation == generation
                && (t.exact || t.video == video)
            {
                return Some(t.texture.id());
            }
        }
        let (frame, exact) = render_thumbnail_exact(
            project,
            item,
            &cache.for_thumbnails(),
            THUMB.x as u32,
            THUMB.y as u32,
        )
        .ok()?;
        let image = ColorImage::from_rgba_unmultiplied(
            [frame.width as usize, frame.height as usize],
            &frame.rgba,
        );
        let texture =
            ctx.load_texture(format!("thumb{}", item.id.0), image, TextureOptions::LINEAR);
        let id = texture.id();
        self.thumbs.insert(
            item.id,
            Thumb {
                source: item.source.clone(),
                generation,
                exact,
                video,
                texture,
            },
        );
        Some(id)
    }
}

impl StudioApp {
    /// The thumbnails of the media, to click, double-click or drag to a timeline.
    /// The thumbnail of one media item, for the source viewer.
    pub(crate) fn media_thumb(&mut self, ctx: &Context, id: MediaId) -> Option<TextureId> {
        let item = self.project().media(id)?.clone();
        let Self {
            pool,
            cache,
            media_generation,
            session,
            ..
        } = self;
        pool.thumb(ctx, session.project(), &item, cache, *media_generation)
    }

    /// A button of a media pool bar: search, sort and the views. Returns false for anything
    /// else.
    pub(crate) fn pool_tool(&mut self, what: &str) -> bool {
        match what {
            "Search" | "Search media" => {
                self.pool.searching = !self.pool.searching;
                self.pool.focus_search = self.pool.searching;
                if !self.pool.searching {
                    self.pool.search.clear();
                }
            }
            "Sort" | "Sorting" | "Sort media" => {
                self.pool.sort = self.pool.sort.next();
                self.info(format!("Media pool sorted by {}", self.pool.sort.label()));
            }
            "Thumbnails" | "The thumbnail view" | "Thumbnail view" => {
                self.pool.view = PoolView::Thumbnails;
            }
            "The filmstrip view" => {
                self.pool.view = PoolView::Thumbnails;
                self.pool.zoom = 1.0;
            }
            "List" | "The list view" => self.pool.view = PoolView::List,
            "Media pool view" | "View options" => {
                self.pool.view = match self.pool.view {
                    PoolView::Thumbnails => PoolView::List,
                    PoolView::List => PoolView::Thumbnails,
                };
            }
            _ => return false,
        }
        true
    }

    /// The media the pool shows: those whose names match the search, in the chosen order.
    pub(crate) fn pool_items(&self) -> Vec<MediaItem> {
        let needle = self.pool.search.trim().to_lowercase();
        let smart = match self.pool.bin {
            BinView::Smart(id) => self.project().smart_bins.iter().find(|b| b.id == id),
            _ => None,
        };
        let mut items: Vec<MediaItem> = self
            .project()
            .media
            .iter()
            .filter(|m| needle.is_empty() || m.name.to_lowercase().contains(&needle))
            .filter(|m| match self.pool.bin {
                BinView::Master => true,
                BinView::Bin(id) => m.bin == Some(id),
                BinView::Smart(_) => smart.is_some_and(|s| s.matches(m)),
            })
            .cloned()
            .collect();
        match self.pool.sort {
            PoolSort::Added => {}
            PoolSort::Name => items.sort_by_key(|m| m.name.to_lowercase()),
            PoolSort::Kind => items.sort_by_key(|m| kind_name(&m.source)),
            PoolSort::Duration => items.sort_by_key(|m| std::cmp::Reverse(m.length.unwrap_or(0))),
        }
        items
    }

    /// The name of the bin the pool shows.
    pub(crate) fn pool_bin_title(&self) -> String {
        let project = self.project();
        match self.pool.bin {
            BinView::Master => None,
            BinView::Bin(id) => project
                .bins
                .iter()
                .find(|b| b.id == id)
                .map(|b| b.name.clone()),
            BinView::Smart(id) => project
                .smart_bins
                .iter()
                .find(|b| b.id == id)
                .map(|b| b.name.clone()),
        }
        .unwrap_or_else(|| "Master".to_string())
    }

    /// Makes a new bin ("Bin 1", "Bin 2"...) and shows it.
    pub(crate) fn new_bin(&mut self) {
        let n = (1..)
            .find(|n| {
                !self
                    .project()
                    .bins
                    .iter()
                    .any(|b| b.name == format!("Bin {n}"))
            })
            .unwrap_or(1);
        if self
            .run(Command::AddBin {
                name: format!("Bin {n}"),
            })
            .is_some()
        {
            if let Some(b) = self.project().bins.last() {
                self.pool.bin = BinView::Bin(b.id);
            }
            self.info("Added a bin: right click a clip to move it in");
        }
    }

    /// Goes to the next bin (Master, the bins, the smart bins, and round again).
    pub(crate) fn next_bin(&mut self) {
        let mut all = vec![BinView::Master];
        all.extend(self.project().bins.iter().map(|b| BinView::Bin(b.id)));
        all.extend(
            self.project()
                .smart_bins
                .iter()
                .map(|b| BinView::Smart(b.id)),
        );
        let at = all.iter().position(|b| *b == self.pool.bin).unwrap_or(0);
        self.pool.bin = all.get(at + 1).copied().unwrap_or(BinView::Master);
        self.info(format!("Showing the {} bin", self.pool_bin_title()));
    }

    /// The right-click menu of a clip in the pool: which bin it is kept in.
    fn media_menu(&mut self, ui: &mut Ui, id: MediaId) {
        let bins: Vec<(u64, String)> = self
            .project()
            .bins
            .iter()
            .map(|b| (b.id, b.name.clone()))
            .collect();
        let current = self.project().media(id).and_then(|m| m.bin);
        ui.menu_button("Move to Bin", |ui| {
            if ui.selectable_label(current.is_none(), "Master").clicked() {
                self.run(Command::SetMediaBin {
                    media: id,
                    bin: None,
                });
                ui.close();
            }
            for (b, name) in &bins {
                if ui.selectable_label(current == Some(*b), name).clicked() {
                    self.run(Command::SetMediaBin {
                        media: id,
                        bin: Some(*b),
                    });
                    ui.close();
                }
            }
        });
        if ui.button("New Bin with This Clip").clicked() {
            self.new_bin();
            if let Some(b) = self.project().bins.last().map(|b| b.id) {
                self.run(Command::SetMediaBin {
                    media: id,
                    bin: Some(b),
                });
            }
            ui.close();
        }
        if ui.button("Insert at Playhead").clicked() {
            self.insert_media(id);
            ui.close();
        }
        if ui.button("New Multicam Clip...").clicked() {
            self.open_new_multicam();
            ui.close();
        }
        self.relink_entry(ui);
    }

    /// "Relink Missing Media" in a menu. Off when nothing is missing, so a click always does
    /// something: the same rule as the button in Media Management.
    fn relink_entry(&mut self, ui: &mut Ui) {
        let missing = self.missing_media().len();
        let label = if missing == 0 {
            "Relink Missing Media".to_string()
        } else {
            format!("Relink {missing} Missing Media...")
        };
        let button = ui.add_enabled(missing > 0, egui::Button::new(label));
        if missing == 0 {
            button.clone().on_disabled_hover_text("Every media file is where the project says it is");
        }
        if button.clicked() {
            self.relink_missing(ui.ctx());
            ui.close();
        }
    }

    /// The right-click menu of the empty part of the pool: what can be done with no clip chosen.
    pub(crate) fn pool_background_menu(&mut self, ui: &mut Ui) {
        if ui.button("Import Media...").clicked() {
            self.do_act(ui.ctx(), crate::menu::Act::ImportMedia);
            ui.close();
        }
        if ui.button("New Bin").clicked() {
            self.new_bin();
            ui.close();
        }
        self.relink_entry(ui);
    }

    pub(crate) fn pool_grid(&mut self, ui: &mut Ui, ctx: &Context) {
        // Right-clicking the empty part of the pool: the clips drawn over this take their own
        // right-click first, so this only answers where there is no clip.
        let background = ui.interact(
            ui.max_rect(),
            ui.id().with("pool_background"),
            Sense::click(),
        );
        background.context_menu(|ui| self.pool_background_menu(ui));
        if self.pool.searching {
            ui.horizontal(|ui| {
                ui.label("Search");
                let field = ui.add(
                    egui::TextEdit::singleline(&mut self.pool.search)
                        .hint_text("Clip name")
                        .desired_width(ui.available_width() - 30.0),
                );
                if std::mem::take(&mut self.pool.focus_search) {
                    field.request_focus();
                }
                if ui
                    .small_button("x")
                    .on_hover_text("Close the search")
                    .clicked()
                {
                    self.pool.searching = false;
                    self.pool.search.clear();
                }
            });
        }
        let items: Vec<(MediaId, String, MediaItem)> = self
            .pool_items()
            .into_iter()
            .map(|m| (m.id, m.name.clone(), m))
            .collect();
        if items.is_empty() {
            if !self.pool.search.is_empty() {
                ui.label(egui::RichText::new("No clips match").color(theme::TEXT_DIM));
            }
            return;
        }
        if self.pool.view == PoolView::List {
            self.pool_list(ui, &items);
            return;
        }
        let textures: Vec<Option<TextureId>> = {
            let Self {
                pool,
                cache,
                media_generation,
                session,
                ..
            } = self;
            items
                .iter()
                .map(|(_, _, item)| {
                    pool.thumb(ctx, session.project(), item, cache, *media_generation)
                })
                .collect()
        };
        let scale = 0.7 + 0.9 * self.pool.zoom.clamp(0.0, 1.0);
        let thumb = THUMB * scale;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for ((id, name, item), texture) in items.iter().zip(textures) {
                        let (rect, response) = ui.allocate_exact_size(
                            vec2(thumb.x + 8.0, thumb.y + 28.0),
                            Sense::click_and_drag(),
                        );
                        // Clips scrolled out of view are not drawn.
                        if !ui.is_rect_visible(rect) {
                            continue;
                        }
                        let painter = ui.painter_at(rect);
                        let selected = self.selected_media == Some(*id);
                        let picture = Rect::from_min_size(rect.min + vec2(4.0, 4.0), thumb);
                        painter.rect_filled(picture, 3.0, Color32::from_rgb(20, 20, 23));
                        if let Some(texture) = texture {
                            painter.image(
                                texture,
                                picture,
                                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        }
                        if matches!(item.source, MediaSource::Audio { .. }) {
                            crate::icons::paint(
                                &painter,
                                picture.center(),
                                crate::icons::Icon::PageAudio,
                                Color32::from_rgb(150, 215, 205),
                            );
                        }
                        // As in the reference: an orange edge for the chosen clip, its name
                        // centred under the picture, shortened to fit.
                        let edge = if selected {
                            egui::Stroke::new(2.0, Color32::from_rgb(232, 120, 60))
                        } else if response.hovered() {
                            egui::Stroke::new(1.0, Color32::from_rgb(110, 110, 116))
                        } else {
                            egui::Stroke::new(1.0, theme::LINE)
                        };
                        painter.rect_stroke(picture, 3.0, edge, StrokeKind::Outside);
                        let shown = if name.chars().count() > 13 {
                            format!("{}...", name.chars().take(10).collect::<String>())
                        } else {
                            name.clone()
                        };
                        painter.text(
                            egui::pos2(picture.center().x, picture.bottom() + 6.0),
                            egui::Align2::CENTER_TOP,
                            shown,
                            crate::theme::regular(14.0),
                            Color32::from_rgb(176, 176, 182),
                        );
                        if response.clicked() || response.drag_started() {
                            self.selected_media = Some(*id);
                        }
                        if response.drag_started() {
                            response.dnd_set_drag_payload(*id);
                        }
                        if response.double_clicked() {
                            self.insert_media(*id);
                        }
                        let facts = self
                            .media_facts
                            .get(id)
                            .map(|f| f.summary())
                            .filter(|t| !t.is_empty())
                            .map_or(String::new(), |t| format!("\n{t}"));
                        let response = response.on_hover_text(format!(
                            "{name}{facts}\nDouble-click to add at the playhead, or drag to the timeline"
                        ));
                        response.context_menu(|ui| self.media_menu(ui, *id));
                    }
                });
            });
    }

    /// The list view: one row per item with its type, length and size.
    fn pool_list(&mut self, ui: &mut Ui, items: &[(MediaId, String, MediaItem)]) {
        let fps = self.project().fps;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("pool_list")
                    .striped(true)
                    .num_columns(4)
                    .min_col_width(60.0)
                    .show(ui, |ui| {
                        for h in ["Clip Name", "Type", "Duration", "Details"] {
                            ui.strong(h);
                        }
                        ui.end_row();
                        for (id, name, item) in items {
                            let selected = self.selected_media == Some(*id);
                            let r = ui.selectable_label(selected, name);
                            let r = r.interact(Sense::click_and_drag());
                            if r.clicked() || r.drag_started() {
                                self.selected_media = Some(*id);
                            }
                            if r.drag_started() {
                                r.dnd_set_drag_payload(*id);
                            }
                            if r.double_clicked() {
                                self.insert_media(*id);
                            }
                            r.context_menu(|ui| self.media_menu(ui, *id));
                            ui.label(kind_name(&item.source));
                            ui.monospace(item.length.map_or("--".into(), |l| fps.timecode(l)));
                            let facts = self
                                .media_facts
                                .get(id)
                                .map_or(String::new(), |f| f.summary());
                            ui.label(facts);
                            ui.end_row();
                        }
                    });
            });
    }

    pub(crate) fn add_generated(&mut self, name: &str, source: MediaSource) {
        let default_duration = self.project().fps.nominal() * 4;
        if let Some(picasso_core::Outcome::Media(id)) = self.run(Command::AddMedia {
            name: name.to_string(),
            source,
            length: None,
            default_duration,
        }) {
            self.selected_media = Some(id);
        }
    }
}
