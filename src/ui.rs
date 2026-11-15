//! The desktop editor: a small egui app with a designer, preset library,
//! position/size page and settings.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

use crosshair_core::{
    color, preset, Crosshair, Direction, DotShape, Layer, LineShape, Point, RenderOptions,
    Settings, Store,
};
use eframe::egui;

use crate::overlay::{self, AppData, DisplayInfo, OverlayHandle};
use crate::startup;

const ACCENT: egui::Color32 = egui::Color32::from_rgb(92, 148, 255);
const ACCENT_DIM: egui::Color32 = egui::Color32::from_rgb(40, 62, 110);
const PANEL: egui::Color32 = egui::Color32::from_rgb(17, 18, 23);
const CARD: egui::Color32 = egui::Color32::from_rgb(26, 28, 36);
const MUTED: egui::Color32 = egui::Color32::from_rgb(150, 156, 172);

const PREVIEW_SIZE: u32 = 320;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Designer,
    Presets,
    Position,
    Settings,
    About,
}

impl Page {
    pub fn parse(name: &str) -> Option<Page> {
        match name.to_ascii_lowercase().as_str() {
            "designer" => Some(Page::Designer),
            "presets" => Some(Page::Presets),
            "position" | "position-size" => Some(Page::Position),
            "settings" => Some(Page::Settings),
            "about" => Some(Page::About),
            _ => None,
        }
    }

    const ALL: [(Page, &'static str); 5] = [
        (Page::Designer, "Designer"),
        (Page::Presets, "Presets"),
        (Page::Position, "Position & Size"),
        (Page::Settings, "Settings"),
        (Page::About, "About"),
    ];
}

pub struct App {
    data: Arc<Mutex<AppData>>,
    overlay: OverlayHandle,
    page: Page,
    preview: Option<egui::TextureHandle>,
    preview_stamp: u64,
    zoom: f32,
    status: String,
    selected_layer: usize,
    launch_on_startup: bool,
    warning: Option<String>,
    frames: u32,
    snapshot_path: Option<std::path::PathBuf>,
    snapshot_done: bool,
}

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        snapshot_path: Option<std::path::PathBuf>,
        initial_page: Option<Page>,
    ) -> Self {
        apply_theme(&cc.egui_ctx);

        let store = Store::load();
        let warning = store.warning.clone();
        let data = Arc::new(Mutex::new(AppData::new(&store)));
        let wake_ctx = cc.egui_ctx.clone();
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || wake_ctx.request_repaint());
        let overlay = overlay::spawn(data.clone(), wake);
        let launch_on_startup = startup::is_launch_on_startup();

        Self {
            data,
            overlay,
            page: initial_page.unwrap_or(Page::Designer),
            preview: None,
            preview_stamp: 0,
            zoom: 3.0,
            status: String::new(),
            selected_layer: 0,
            launch_on_startup,
            warning,
            frames: 0,
            snapshot_path,
            snapshot_done: false,
        }
    }

    fn snapshot(&self) -> Snapshot {
        let data = self.data.lock().expect("app data poisoned");
        Snapshot {
            settings: data.settings.clone(),
            current: data.current.clone(),
            presets: data.presets.clone(),
            displays: data.displays.clone(),
            hotkey_log: data.hotkey_log.clone(),
        }
    }

    fn commit(&mut self, snap: &Snapshot) {
        if let Ok(mut data) = self.data.lock() {
            data.settings = snap.settings.clone();
            data.current = snap.current.clone();
            data.presets = snap.presets.clone();
            let _ = data.save_settings();
            let _ = data.save_presets();
        }
        self.overlay.notify();
    }

    fn refresh_preview(&mut self, ctx: &egui::Context, snap: &Snapshot) {
        let stamp = preview_stamp(snap, self.zoom);
        if self.preview.is_some() && stamp == self.preview_stamp {
            return;
        }
        self.preview_stamp = stamp;
        let options = RenderOptions {
            scale: snap.settings.scale * self.zoom,
            opacity: snap.settings.global_opacity,
            padding: 8.0,
        };
        if let Some(rendered) = crosshair_core::render_sized(&snap.current, &options, PREVIEW_SIZE) {
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [PREVIEW_SIZE as usize, PREVIEW_SIZE as usize],
                &rendered.to_unmultiplied_rgba(),
            );
            self.preview =
                Some(ctx.load_texture("crosshair-preview", image, egui::TextureOptions::NEAREST));
        }
    }
}

struct Snapshot {
    settings: Settings,
    current: Crosshair,
    presets: Vec<Crosshair>,
    displays: Vec<DisplayInfo>,
    hotkey_log: Vec<String>,
}

fn preview_stamp(snap: &Snapshot, zoom: f32) -> u64 {
    let mut hasher = DefaultHasher::new();
    serde_json::to_string(&snap.current)
        .unwrap_or_default()
        .hash(&mut hasher);
    snap.settings.scale.to_bits().hash(&mut hasher);
    snap.settings.global_opacity.to_bits().hash(&mut hasher);
    zoom.to_bits().hash(&mut hasher);
    hasher.finish()
}


fn apply_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = PANEL;
    visuals.window_fill = CARD;
    visuals.extreme_bg_color = egui::Color32::from_rgb(11, 12, 15);
    visuals.faint_bg_color = egui::Color32::from_rgb(24, 26, 33);
    visuals.widgets.noninteractive.bg_fill = CARD;
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(38, 41, 52);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(54, 59, 74);
    visuals.widgets.active.bg_fill = ACCENT_DIM;
    visuals.selection.bg_fill = ACCENT_DIM;
    visuals.hyperlink_color = ACCENT;
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 7.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    ctx.set_style(style);
}

fn heading(text: &str) -> egui::RichText {
    egui::RichText::new(text).size(19.0).strong()
}

fn muted(text: &str) -> egui::RichText {
    egui::RichText::new(text).size(12.0).color(MUTED)
}

fn card(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::NONE
        .fill(CARD)
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new(title).size(13.0).strong().color(MUTED));
            ui.add_space(2.0);
            add(ui);
        });
    ui.add_space(6.0);
}

fn hex_to_rgb(hex: &str) -> [u8; 3] {
    let rgba = color::parse(hex).unwrap_or(color::Rgba8::WHITE);
    [rgba.r, rgba.g, rgba.b]
}

fn rgb_to_hex(rgb: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])
}

fn upsert(presets: &mut Vec<Crosshair>, crosshair: Crosshair) -> usize {
    if let Some(index) = presets
        .iter()
        .position(|p| p.name.eq_ignore_ascii_case(&crosshair.name))
    {
        presets[index] = crosshair;
        index
    } else {
        presets.push(crosshair);
        presets.len() - 1
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut snap = self.snapshot();
        let mut dirty = false;

        if let Some(warning) = self.warning.take() {
            self.status = format!("Recovered from a problem: {warning}");
        }

        self.top_bar(ctx, &mut snap, &mut dirty);
        self.side_panel(ctx);
        let status = self.status.clone();
        if !status.is_empty() {
            egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(muted(&status));
                });
            });
        }
        let panel_frame = egui::Frame::NONE
            .inner_margin(egui::Margin {
                left: 16,
                right: 16,
                top: 6,
                bottom: 8,
            });
        egui::CentralPanel::default()
            .frame(panel_frame)
            .show(ctx, |ui| match self.page {
            Page::Designer => self.designer(ui, &mut snap, &mut dirty),
            Page::Presets => self.presets(ui, &mut snap, &mut dirty),
            Page::Position => self.position(ui, &mut snap, &mut dirty),
            Page::Settings => self.settings(ui, &mut snap, &mut dirty),
            Page::About => self.about(ui),
        });

        if dirty {
            self.preview_stamp = 0;
            self.commit(&snap);
            ctx.request_repaint();
        }
        self.refresh_preview(ctx, &snap);
        self.maybe_snapshot(ctx);
    }
}

impl App {
    /// `--ui-snapshot`: let the editor draw itself, save the framebuffer, quit.
    /// Uses egui's own screenshot request so the capture is of this window even
    /// when another app is in front.
    fn maybe_snapshot(&mut self, ctx: &egui::Context) {
        let Some(path) = self.snapshot_path.clone() else {
            return;
        };
        self.frames += 1;
        if self.frames < 20 {
            ctx.request_repaint();
            return;
        }
        if !self.snapshot_done {
            self.snapshot_done = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            ctx.request_repaint();
            return;
        }
        let image = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image {
            let mut rgba = Vec::with_capacity(image.pixels.len() * 4);
            for pixel in &image.pixels {
                rgba.extend_from_slice(&pixel.to_srgba_unmultiplied());
            }
            match crosshair_core::save_rgba_png(
                &path,
                image.size[0] as u32,
                image.size[1] as u32,
                &rgba,
            ) {
                Ok(()) => println!("saved {} ({}x{})", path.display(), image.size[0], image.size[1]),
                Err(error) => println!("snapshot failed: {error}"),
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else {
            ctx.request_repaint();
        }
    }

    fn top_bar(&mut self, ctx: &egui::Context, snap: &mut Snapshot, dirty: &mut bool) {
        egui::TopBottomPanel::top("top")
            .exact_height(54.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new("CrosshairX")
                            .size(18.0)
                            .strong()
                            .color(egui::Color32::WHITE),
                    );
                    ui.label(egui::RichText::new("-Open").size(18.0).strong().color(ACCENT));
                    ui.add_space(12.0);
                    ui.label(muted("free · offline · no accounts"));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let label = if snap.settings.overlay_visible {
                            "Overlay: on"
                        } else {
                            "Overlay: off"
                        };
                        let fill = if snap.settings.overlay_visible {
                            ACCENT_DIM
                        } else {
                            egui::Color32::from_rgb(46, 28, 30)
                        };
                        if ui
                            .add(egui::Button::new(egui::RichText::new(label).strong()).fill(fill))
                            .on_hover_text("Toggle the overlay (also bound to the global hotkey)")
                            .clicked()
                        {
                            snap.settings.overlay_visible = !snap.settings.overlay_visible;
                            *dirty = true;
                        }
                        ui.label(muted(&snap.settings.toggle_hotkey.describe()));
                        ui.add_space(8.0);
                    });
                });
            });
    }

    fn side_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("nav")
            .exact_width(186.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.add_space(10.0);
                for (page, label) in Page::ALL {
                    let selected = self.page == page;
                    let text = egui::RichText::new(label)
                        .size(14.0)
                        .color(if selected {
                            egui::Color32::WHITE
                        } else {
                            MUTED
                        });
                    let button = egui::Button::new(text)
                        .fill(if selected { ACCENT_DIM } else { egui::Color32::TRANSPARENT })
                        .min_size(egui::vec2(ui.available_width(), 34.0));
                    if ui.add(button).clicked() {
                        self.page = page;
                    }
                }
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);
                ui.label(muted("Everything runs locally."));
                ui.label(muted("No telemetry, no login."));
            });
    }

    fn designer(&mut self, ui: &mut egui::Ui, snap: &mut Snapshot, dirty: &mut bool) {
        ui.horizontal(|ui| {
            ui.label(heading("Designer"));
            ui.label(muted("changes apply to the overlay instantly"));
        });
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(6.0);

        let total_width = ui.available_width();
        let controls_width = (total_width * 0.52).clamp(340.0, 460.0);
        let preview_side = (total_width - controls_width - 28.0).clamp(220.0, 420.0);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(controls_width, ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| self.designer_controls(ui, snap, dirty));
                },
            );
            ui.add_space(16.0);
            ui.vertical(|ui| self.preview_area(ui, snap, preview_side));
        });
    }

    fn designer_controls(&mut self, ui: &mut egui::Ui, snap: &mut Snapshot, dirty: &mut bool) {
        let mut changed = false;

        card(ui, "PRESET", |ui| {
            ui.horizontal(|ui| {
                let response = ui.add(
                    egui::TextEdit::singleline(&mut snap.current.name)
                        .desired_width(ui.available_width() - 74.0)
                        .hint_text("Preset name"),
                );
                changed |= response.changed();
                if ui.button("Save").clicked() {
                    snap.current.sanitize();
                    upsert(&mut snap.presets, snap.current.clone());
                    snap.settings.active_preset = snap.current.name.clone();
                    self.status = format!("Saved \"{}\"", snap.current.name);
                    changed = true;
                }
            });
            ui.label(muted("Saving with an existing name overwrites that preset."));
        });

        card(ui, "LAYERS", |ui| {
            ui.horizontal(|ui| {
                for index in 0..snap.current.layers.len() {
                    let selected = index == self.selected_layer;
                    if ui
                        .add(egui::Button::new(format!("{}", index + 1)).fill(if selected {
                            ACCENT_DIM
                        } else {
                            egui::Color32::TRANSPARENT
                        }))
                        .on_hover_text(format!("Edit layer {}", index + 1))
                        .clicked()
                    {
                        self.selected_layer = index;
                    }
                }
                ui.separator();
                if ui
                    .add_enabled(
                        snap.current.layers.len() < 8,
                        egui::Button::new("+ Add"),
                    )
                    .on_hover_text("Stack another crosshair on top of this one")
                    .clicked()
                {
                    let mut layer = snap.current.layers[self.selected_layer].clone();
                    layer.position = Point::new(0.0, 0.0);
                    snap.current.layers.push(layer);
                    self.selected_layer = snap.current.layers.len() - 1;
                    changed = true;
                }
                if ui
                    .add_enabled(
                        snap.current.layers.len() > 1,
                        egui::Button::new("Remove"),
                    )
                    .clicked()
                {
                    snap.current.layers.remove(self.selected_layer);
                    self.selected_layer = self.selected_layer.saturating_sub(1);
                    changed = true;
                }
            });
            ui.label(muted("Layers stack in order; each has its own dot, arms and position."));
        });
        self.selected_layer = self.selected_layer.min(snap.current.layers.len().saturating_sub(1));
        let layer = &mut snap.current.layers[self.selected_layer];

        card(ui, "CENTER DOT", |ui| {
            changed |= self.dot_controls(ui, layer);
        });
        card(ui, "LINES", |ui| {
            changed |= self.line_controls(ui, layer);
        });
        card(ui, "OUTLINE", |ui| {
            changed |= self.outline_controls(ui, layer);
        });
        card(ui, "POSITION & SCALE (THIS LAYER)", |ui| {
            ui.horizontal(|ui| {
                ui.label("X");
                changed |= ui
                    .add(egui::DragValue::new(&mut layer.position.x).speed(0.5).range(-500.0..=500.0))
                    .changed();
                ui.label("Y");
                changed |= ui
                    .add(egui::DragValue::new(&mut layer.position.y).speed(0.5).range(-500.0..=500.0))
                    .changed();
            });
            changed |= ui
                .add(egui::Slider::new(&mut layer.layer.size, 0.1..=4.0).text("Layer scale"))
                .changed();
            if ui.button("Reset layer").clicked() {
                layer.position = Point::ZERO;
                layer.layer.size = 1.0;
                changed = true;
            }
        });

        if changed {
            snap.current.sanitize();
            *dirty = true;
        }
    }

    fn dot_controls(&mut self, ui: &mut egui::Ui, layer: &mut Layer) -> bool {
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("Shape");
            egui::ComboBox::from_id_salt("dot-shape")
                .selected_text(layer.dot.shape.label())
                .show_ui(ui, |ui| {
                    for shape in DotShape::ALL {
                        changed |= ui
                            .selectable_value(&mut layer.dot.shape, shape, shape.label())
                            .changed();
                    }
                });
            let mut rgb = hex_to_rgb(&layer.dot.color);
            if ui.color_edit_button_srgb(&mut rgb).changed() {
                layer.dot.color = rgb_to_hex(rgb);
                changed = true;
            }
        });
        changed |= ui
            .add(egui::Slider::new(&mut layer.dot.diameter, 0.0..=40.0).text("Diameter"))
            .changed();
        if layer.dot.shape == DotShape::Ring {
            changed |= ui
                .add(egui::Slider::new(&mut layer.dot.thickness, 0.0..=12.0).text("Ring width"))
                .changed();
        }
        changed |= ui
            .add(egui::Slider::new(&mut layer.dot.opacity, 0.0..=1.0).text("Opacity"))
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut layer.dot.blur, 0.0..=8.0).text("Blur"))
            .changed();
        changed
    }

    fn line_controls(&mut self, ui: &mut egui::Ui, layer: &mut Layer) -> bool {
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("Shape");
            egui::ComboBox::from_id_salt("line-shape")
                .selected_text(layer.line.shape.label())
                .show_ui(ui, |ui| {
                    for shape in LineShape::ALL {
                        changed |= ui
                            .selectable_value(&mut layer.line.shape, shape, shape.label())
                            .changed();
                    }
                });
            let mut rgb = hex_to_rgb(&layer.line.color);
            if ui.color_edit_button_srgb(&mut rgb).changed() {
                layer.line.color = rgb_to_hex(rgb);
                changed = true;
            }
        });
        changed |= ui
            .add(egui::Slider::new(&mut layer.line.length, 0.0..=60.0).text("Length"))
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut layer.line.thickness, 0.0..=20.0).text("Thickness"))
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut layer.line.offset, 0.0..=60.0).text("Gap"))
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut layer.line.opacity, 0.0..=1.0).text("Opacity"))
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut layer.line.rotation, 0.0..=360.0).text("Rotation"))
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut layer.line.blur, 0.0..=8.0).text("Blur"))
            .changed();
        ui.label(muted("Arms"));
        ui.horizontal(|ui| {
            for dir in Direction::ALL {
                let mut visible = layer.line.is_visible(dir);
                if ui.checkbox(&mut visible, dir.label()).changed() {
                    if visible {
                        if !layer.line.is_visible(dir) {
                            layer.line.visible.push(dir);
                        }
                    } else {
                        layer.line.visible.retain(|d| *d != dir);
                    }
                    changed = true;
                }
            }
        });
        changed
    }

    fn outline_controls(&mut self, ui: &mut egui::Ui, layer: &mut Layer) -> bool {
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("Color");
            let mut rgb = hex_to_rgb(&layer.outline.color);
            if ui.color_edit_button_srgb(&mut rgb).changed() {
                layer.outline.color = rgb_to_hex(rgb);
                changed = true;
            }
            ui.label(muted("thickness 0 disables the outline"));
        });
        changed |= ui
            .add(egui::Slider::new(&mut layer.outline.thickness, 0.0..=12.0).text("Thickness"))
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut layer.outline.opacity, 0.0..=1.0).text("Opacity"))
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut layer.outline.blur, 0.0..=8.0).text("Blur"))
            .changed();
        changed
    }

    fn preview_area(&mut self, ui: &mut egui::Ui, snap: &mut Snapshot, side: f32) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Live preview").strong());
            ui.label(muted(&format!("{:.0}x", self.zoom)));
            if ui.small_button("Zoom -").clicked() {
                self.zoom = (self.zoom - 0.5).max(1.0);
            }
            if ui.small_button("Zoom +").clicked() {
                self.zoom = (self.zoom + 0.5).min(8.0);
            }
        });
        ui.label(muted(
            "Rendered with the same rasteriser the overlay uses, so pixels match.",
        ));

        let (response, painter) = ui.allocate_painter(
            egui::vec2(ui.available_width().min(side), ui.available_width().min(side)),
            egui::Sense::hover(),
        );
        let rect = response.rect;
        painter.rect_filled(rect, 0, egui::Color32::from_rgb(12, 13, 17));
        // checkerboard so transparency is obvious
        let cell: f32 = 16.0;
        let mut y = rect.top();
        let mut row = 0;
        while y < rect.bottom() {
            let mut x = rect.left();
            let mut col = 0;
            while x < rect.right() {
                if (row + col) % 2 == 0 {
                    painter.rect_filled(
                        egui::Rect::from_min_size(
                            egui::pos2(x, y),
                            egui::vec2(cell.min(rect.right() - x), cell.min(rect.bottom() - y)),
                        ),
                        0,
                        egui::Color32::from_rgb(20, 22, 27),
                    );
                }
                x += cell;
                col += 1;
            }
            y += cell;
            row += 1;
        }

        if let Some(texture) = &self.preview {
            let texture_size = texture.size_vec2();
            let scale = (side / texture_size.x).min(1.0);
            let size = texture_size * scale;
            let image_rect = egui::Rect::from_center_size(rect.center(), size);
            painter.image(
                texture.id(),
                image_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }

        // centre guide
        let stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgba_unmultiplied(92, 148, 255, 70));
        painter.line_segment(
            [
                egui::pos2(rect.center().x, rect.top()),
                egui::pos2(rect.center().x, rect.bottom()),
            ],
            stroke,
        );
        painter.line_segment(
            [
                egui::pos2(rect.left(), rect.center().y),
                egui::pos2(rect.right(), rect.center().y),
            ],
            stroke,
        );

        ui.add_space(6.0);
        card(ui, "OVERLAY", |ui| {
            ui.label(format!(
                "Draws on {}",
                snap.displays
                    .get(snap.settings.monitor)
                    .map(|d| d.label())
                    .unwrap_or_else(|| "no display".to_string())
            ));
            ui.label(muted(&format!(
                "Global size {:.0}% · opacity {:.0}% · offset ({:.0}, {:.0})",
                snap.settings.scale * 100.0,
                snap.settings.global_opacity * 100.0,
                snap.settings.offset.x,
                snap.settings.offset.y
            )));
        });
    }

    fn presets(&mut self, ui: &mut egui::Ui, snap: &mut Snapshot, dirty: &mut bool) {
        ui.horizontal(|ui| {
            ui.label(heading("Presets"));
            ui.label(muted("saved locally, import/export as JSON"));
        });
        ui.separator();

        ui.horizontal(|ui| {
            if ui.button("Import JSON...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Crosshair preset", &["json"])
                    .set_title("Import crosshair presets")
                    .pick_file()
                {
                    match preset::import_file(&path) {
                        Ok(imported) => {
                            let count = imported.len();
                            for crosshair in imported {
                                upsert(&mut snap.presets, crosshair);
                            }
                            self.status = format!("Imported {count} preset(s)");
                            *dirty = true;
                        }
                        Err(error) => self.status = format!("Import failed: {error}"),
                    }
                }
            }
            if ui.button("Export current...").clicked() {
                let file_name = format!("{}.json", preset::file_stem(&snap.current.name));
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Crosshair preset", &["json"])
                    .set_file_name(&file_name)
                    .set_title("Export crosshair preset")
                    .save_file()
                {
                    match std::fs::write(&path, preset::export_json(&snap.current)) {
                        Ok(()) => self.status = format!("Exported to {}", path.display()),
                        Err(error) => self.status = format!("Export failed: {error}"),
                    }
                }
            }
            if ui.button("Duplicate current").clicked() {
                let mut copy = snap.current.clone();
                copy.name = format!("{} copy", snap.current.name);
                upsert(&mut snap.presets, copy);
                *dirty = true;
            }
        });
        ui.add_space(6.0);

        let mut to_delete: Option<String> = None;
        let mut to_load: Option<Crosshair> = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for crosshair in &snap.presets {
                    egui::Frame::NONE
                        .fill(CARD)
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new(&crosshair.name).size(14.0).strong());
                                    ui.label(muted(&describe(&crosshair.layers)));
                                });
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.button("Delete").clicked() {
                                            to_delete = Some(crosshair.name.clone());
                                        }
                                        if ui.button("Load").clicked() {
                                            to_load = Some(crosshair.clone());
                                        }
                                    },
                                );
                            });
                        });
                    ui.add_space(4.0);
                }
            });

        if let Some(crosshair) = to_load {
            self.status = format!("Loaded \"{}\"", crosshair.name);
            snap.settings.active_preset = crosshair.name.clone();
            snap.current = crosshair;
            *dirty = true;
        }
        if let Some(name) = to_delete {
            snap.presets.retain(|p| p.name != name);
            self.status = format!("Deleted \"{name}\"");
            *dirty = true;
        }
    }

    fn position(&mut self, ui: &mut egui::Ui, snap: &mut Snapshot, dirty: &mut bool) {
        ui.horizontal(|ui| {
            ui.label(heading("Position & Size"));
            ui.label(muted("centred on the chosen display, nudged with the hotkey"));
        });
        ui.separator();

        let mut changed = false;
        card(ui, "DISPLAY", |ui| {
            let mut selected = snap.settings.monitor;
            egui::ComboBox::from_id_salt("monitor")
                .selected_text(
                    snap.displays
                        .get(selected)
                        .map(|d| d.label())
                        .unwrap_or_else(|| "No display".to_string()),
                )
                .show_ui(ui, |ui| {
                    for display in &snap.displays {
                        changed |= ui
                            .selectable_value(&mut selected, display.index, display.label())
                            .changed();
                    }
                });
            snap.settings.monitor = selected;
            ui.label(muted(
                "The overlay is a physical-pixel layered window, so it works at any resolution.",
            ));
        });

        card(ui, "OFFSET", |ui| {
            ui.horizontal(|ui| {
                ui.label("X");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut snap.settings.offset.x)
                            .speed(1.0)
                            .range(-4000.0..=4000.0)
                            .suffix(" px"),
                    )
                    .changed();
                ui.label("Y");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut snap.settings.offset.y)
                            .speed(1.0)
                            .range(-4000.0..=4000.0)
                            .suffix(" px"),
                    )
                    .changed();
                if ui.button("Center").clicked() {
                    snap.settings.offset = Point::ZERO;
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                if ui.button("<-").clicked() {
                    snap.settings.offset.x -= snap.settings.move_step;
                    changed = true;
                }
                if ui.button("->").clicked() {
                    snap.settings.offset.x += snap.settings.move_step;
                    changed = true;
                }
                if ui.button("^").clicked() {
                    snap.settings.offset.y += snap.settings.move_step;
                    changed = true;
                }
                if ui.button("v").clicked() {
                    snap.settings.offset.y -= snap.settings.move_step;
                    changed = true;
                }
                ui.label(muted(&format!(
                    "or use {} in any app",
                    snap.settings.move_hotkey.describe()
                )));
            });
        });

        card(ui, "GLOBAL SIZE & OPACITY", |ui| {
            changed |= ui
                .add(egui::Slider::new(&mut snap.settings.scale, 0.25..=6.0).text("Size"))
                .changed();
            changed |= ui
                .add(egui::Slider::new(&mut snap.settings.global_opacity, 0.05..=1.0).text("Opacity"))
                .changed();
            changed |= ui
                .checkbox(
                    &mut snap.settings.ignore_dpi,
                    "Ignore display DPI scaling (size in physical pixels)",
                )
                .changed();
            ui.label(muted(
                "Leave unchecked to keep the crosshair the same visual size on every display.",
            ));
        });

        if changed {
            snap.settings.sanitize();
            *dirty = true;
        }
    }

    fn settings(&mut self, ui: &mut egui::Ui, snap: &mut Snapshot, dirty: &mut bool) {
        ui.horizontal(|ui| {
            ui.label(heading("Settings"));
            ui.label(muted("hotkeys are global and work while a game is focused"));
        });
        ui.separator();

        let mut changed = false;
        let mut hotkeys_changed = false;
        card(ui, "TOGGLE HOTKEY", |ui| {
            ui.horizontal(|ui| {
                for (flag, label) in [
                    (&mut snap.settings.toggle_hotkey.ctrl, "Ctrl"),
                    (&mut snap.settings.toggle_hotkey.alt, "Alt"),
                    (&mut snap.settings.toggle_hotkey.shift, "Shift"),
                    (&mut snap.settings.toggle_hotkey.win, "Win"),
                ] {
                    if ui.checkbox(flag, label).changed() {
                        hotkeys_changed = true;
                    }
                }
                ui.separator();
                egui::ComboBox::from_id_salt("toggle-key")
                    .selected_text(&snap.settings.toggle_hotkey.key)
                    .show_ui(ui, |ui| {
                        for key in KEY_CHOICES {
                            if ui
                                .selectable_value(
                                    &mut snap.settings.toggle_hotkey.key,
                                    key.to_string(),
                                    *key,
                                )
                                .changed()
                            {
                                hotkeys_changed = true;
                            }
                        }
                    });
            });
            ui.label(muted(&format!(
                "Currently {}",
                snap.settings.toggle_hotkey.describe()
            )));
        });

        card(ui, "POSITION HOTKEY", |ui| {
            ui.horizontal(|ui| {
                for (flag, label) in [
                    (&mut snap.settings.move_hotkey.ctrl, "Ctrl"),
                    (&mut snap.settings.move_hotkey.alt, "Alt"),
                    (&mut snap.settings.move_hotkey.shift, "Shift"),
                    (&mut snap.settings.move_hotkey.win, "Win"),
                ] {
                    if ui.checkbox(flag, label).changed() {
                        hotkeys_changed = true;
                    }
                }
                ui.label(muted("+ arrow keys"));
            });
            changed |= ui
                .add(egui::Slider::new(&mut snap.settings.move_step, 1.0..=20.0).text("Step (px)"))
                .changed();
        });

        card(ui, "STARTUP", |ui| {
            let mut launch = self.launch_on_startup;
            if ui.checkbox(&mut launch, "Start with Windows").changed() {
                match startup::set_launch_on_startup(launch) {
                    Ok(()) => {
                        self.launch_on_startup = launch;
                        snap.settings.launch_on_startup = launch;
                        changed = true;
                        self.status = if launch {
                            "Will start with Windows".to_string()
                        } else {
                            "Removed from Windows startup".to_string()
                        };
                    }
                    Err(error) => self.status = format!("Startup change failed: {error}"),
                }
            }
            ui.label(muted("Adds a per-user Run entry; no service or admin rights."));
        });

        card(ui, "HOTKEY STATUS", |ui| {
            if snap.hotkey_log.is_empty() {
                ui.label(muted("Waiting for the overlay thread..."));
            }
            for line in &snap.hotkey_log {
                ui.label(format!("• {line}"));
            }
            if ui.button("Re-register hotkeys").clicked() {
                self.overlay.reload_hotkeys();
                self.status = "Hotkeys re-registered".to_string();
            }
        });

        card(ui, "DATA", |ui| {
            let dir = self
                .data
                .lock()
                .map(|d| d.data_dir.display().to_string())
                .unwrap_or_default();
            ui.label(egui::RichText::new(&dir).monospace().size(12.0));
            ui.horizontal(|ui| {
                if ui.button("Open folder").clicked() {
                    let _ = std::process::Command::new("explorer").arg(&dir).spawn();
                }
                if ui.button("Reset settings").clicked() {
                    snap.settings = Settings::default();
                    snap.settings.active_preset = snap.current.name.clone();
                    self.zoom = 3.0;
                    self.status = "Settings reset to defaults".to_string();
                    changed = true;
                    hotkeys_changed = true;
                }
            });
        });

        if changed || hotkeys_changed {
            snap.settings.sanitize();
            *dirty = true;
        }
        if hotkeys_changed {
            self.commit_hotkeys(snap);
        }
    }

    /// Settings must reach the overlay thread before it re-registers hotkeys.
    fn commit_hotkeys(&mut self, snap: &Snapshot) {
        if let Ok(mut data) = self.data.lock() {
            data.settings = snap.settings.clone();
            let _ = data.save_settings();
        }
        self.overlay.reload_hotkeys();
        self.overlay.notify();
    }

    fn about(&mut self, ui: &mut egui::Ui) {
        ui.label(heading("About CrosshairX-Open"));
        ui.add_space(4.0);
        card(ui, "VERSION", |ui| {
            ui.label(format!("CrosshairX-Open {}", crosshair_core::VERSION));
            ui.label(muted("MIT licensed. Built with Rust, egui and tiny-skia."));
        });
        card(ui, "WHAT THIS IS", |ui| {
            ui.label(
                "A free replacement for paid crosshair overlays. It draws a transparent, \
                 click-through, always-on-top crosshair over your desktop and games.",
            );
            ui.label(
                "Every feature is local: no accounts, no ads, no subscriptions, no telemetry, \
                 no network access at runtime.",
            );
        });
        card(ui, "HOW IT WORKS", |ui| {
            ui.label("• Layered Win32 window (WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST).");
            ui.label("• Premultiplied alpha blitted with UpdateLayeredWindow, so no window capture of your desktop is needed.");
            ui.label("• Per-monitor DPI v2 awareness; sizes are device pixels on the selected display.");
            ui.label("• Global hotkeys via RegisterHotKey on the overlay thread; no input hooks.");
            ui.label("• Repaints only when something changes, so idle CPU use is ~0%.");
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.hyperlink_to(
                "Source code",
                "https://github.com/tokyonocturn/CrosshairX-Open",
            );
            ui.label(muted("— not affiliated with any other crosshair product."));
        });
    }
}

const KEY_CHOICES: &[&str] = &[
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S",
    "T", "U", "V", "W", "X", "Y", "Z", "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "F1", "F2",
    "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12", "Space", "Tab", "Insert", "Home",
    "End", "PageUp", "PageDown", "Backquote", "Minus", "Equals",
];

fn describe(layers: &[Layer]) -> String {
    let layer = match layers.first() {
        Some(layer) => layer,
        None => return "empty".to_string(),
    };
    let arms = layer.line.visible.len();
    let dot = if layer.dot.opacity > 0.0 {
        format!("dot {:.0}px {}", layer.dot.diameter, layer.dot.shape.label().to_lowercase())
    } else {
        "no dot".to_string()
    };
    let arms_text = if arms == 0 {
        "no arms".to_string()
    } else {
        format!(
            "{arms} arm(s), {} {:.0}px · gap {:.0}",
            layer.line.shape.label().to_lowercase(),
            layer.line.length,
            layer.line.offset
        )
    };
    format!("{arms_text} · {dot}")
}

/// Register the winit window title and start the UI.
pub fn run(
    snapshot_path: Option<std::path::PathBuf>,
    initial_page: Option<Page>,
) -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 760.0])
            .with_min_inner_size([880.0, 600.0])
            .with_title("CrosshairX-Open"),
        ..Default::default()
    };
    eframe::run_native(
        "CrosshairX-Open",
        options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, snapshot_path, initial_page)))),
    )
}

