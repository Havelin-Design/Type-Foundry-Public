//! The font overview grid and the preview strip.

use eframe::egui::{self, Color32, ColorImage, CornerRadius, Pos2, Rect, Sense, Stroke, Vec2};
use foundry_app::palette::{BONE, FOCUS, HAIRLINE, HAIRLINE_STRONG, INK, MUTED, RAISED};
use foundry_app::{Bounds, Pt, Viewport, rasterize};

use crate::app::{FoundryWindow, Mode};
use crate::canvas::paint_fill;
use crate::color;

const LABEL_HEIGHT: f32 = 30.0;
const GAP: f32 = 8.0;

impl FoundryWindow {
    pub fn overview(&mut self, ui: &mut egui::Ui) {
        if !self.has_font() {
            ui.centered_and_justified(|ui| {
                ui.weak("File > Open… a font, or File > New font. Ctrl+O opens.");
            });
            return;
        }
        egui::Frame::new()
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.strong(&self.font_name);
                    ui.weak(format!(
                        "{} glyphs · {} units per em",
                        self.glyphs.len(),
                        self.upm
                    ));
                    ui.separator();
                    ui.add(
                        egui::TextEdit::singleline(&mut self.filter)
                            .hint_text("Filter by name or character")
                            .desired_width(200.0),
                    );
                    ui.add(
                        egui::Slider::new(&mut self.settings.cell_size, 48.0..=200.0)
                            .show_value(false)
                            .text("Size"),
                    );
                    if ui.button("New glyph…").clicked() {
                        self.dialogs.new_glyph = Some(crate::app::NewGlyph {
                            name: String::new(),
                            unicode: String::new(),
                            advance: f64::from(self.upm) * 0.6,
                        });
                    }
                });
                ui.add_space(6.0);
                self.grid(ui);
            });
    }

    fn grid(&mut self, ui: &mut egui::Ui) {
        let names: Vec<(String, Option<u32>)> = self
            .glyphs
            .iter()
            .filter(|entry| matches_filter(&self.filter, &entry.name, entry.unicode))
            .map(|entry| (entry.name.clone(), entry.unicode))
            .collect();
        if names.is_empty() {
            ui.weak("No glyphs match.");
            return;
        }
        let cell = self.settings.cell_size.round();
        let columns = ((ui.available_width() + GAP) / (cell + GAP))
            .floor()
            .max(1.0) as usize;
        let rows = names.len().div_ceil(columns);
        let row_height = cell + LABEL_HEIGHT + GAP;
        egui::ScrollArea::vertical().auto_shrink(false).show_rows(
            ui,
            row_height,
            rows,
            |ui, range| {
                for row in range {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = GAP;
                        for (name, unicode) in names.iter().skip(row * columns).take(columns) {
                            self.cell(ui, name, *unicode, cell);
                        }
                    });
                    ui.add_space(GAP);
                }
            },
        );
    }

    fn cell(&mut self, ui: &mut egui::Ui, name: &str, unicode: Option<u32>, cell: f32) {
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(cell, cell + LABEL_HEIGHT), Sense::click());
        let painter = ui.painter_at(rect);
        let selected = self.current.as_deref() == Some(name);
        let edge = if selected {
            color(FOCUS)
        } else if response.hovered() {
            color(HAIRLINE_STRONG)
        } else {
            color(HAIRLINE)
        };
        painter.rect_filled(rect, CornerRadius::same(3), color(RAISED));
        let art = Rect::from_min_size(rect.min, Vec2::splat(cell)).shrink(3.0);
        if let Some(texture) = self.thumb(ui.ctx(), name, art.width() as usize) {
            painter.image(
                texture.id(),
                art,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        let label = unicode
            .and_then(char::from_u32)
            .filter(|c| !c.is_control() && name != c.to_string())
            .map_or_else(|| name.to_string(), |c| format!("{c}  {name}"));
        painter.text(
            Pos2::new(rect.left() + 6.0, rect.bottom() - LABEL_HEIGHT / 2.0),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(11.0),
            if selected { color(INK) } else { color(MUTED) },
        );
        painter.rect_stroke(
            rect,
            CornerRadius::same(3),
            Stroke::new(if selected { 2.0 } else { 1.0 }, edge),
            egui::StrokeKind::Inside,
        );
        let response = response.on_hover_text(match unicode {
            Some(code) => format!("{name} · U+{code:04X}"),
            None => name.to_string(),
        });
        if response.clicked() {
            self.select_glyph(Some(name.to_string()));
        }
        if response.double_clicked() {
            self.open_editor(name.to_string());
        }
    }

    /// A cached thumbnail at the shared font scale, so glyph sizes compare across the grid.
    fn thumb(
        &mut self,
        ctx: &egui::Context,
        name: &str,
        size: usize,
    ) -> Option<egui::TextureHandle> {
        if let Some(found) = self.thumbs.get(name)
            && found.size()[0] == size
        {
            return Some(found.clone());
        }
        let outline = self.outline(name)?;
        let height = (self.ascender - self.descender).abs().max(1.0) * 1.2;
        let middle = outline.advance / 2.0;
        let bottom = self.descender - height * 0.08;
        let frame = Bounds {
            min: Pt::new(middle - height / 2.0, bottom),
            max: Pt::new(middle + height / 2.0, bottom + height),
        };
        let mask = rasterize(&outline, frame, size, size);
        let [br, bg, bb, _] = color(BONE).to_array();
        let pixels: Vec<Color32> = mask
            .iter()
            .map(|alpha| {
                let keep = 255 - u16::from(*alpha);
                let mix = |channel: u8| ((u16::from(channel) * keep) / 255) as u8;
                Color32::from_rgb(mix(br), mix(bg), mix(bb))
            })
            .collect();
        let image = ColorImage::new([size, size], pixels);
        let texture =
            ctx.load_texture(format!("thumb:{name}"), image, egui::TextureOptions::LINEAR);
        self.thumbs.insert(name.to_string(), texture.clone());
        Some(texture)
    }

    pub fn preview_strip(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.weak("Preview");
            ui.add(
                egui::TextEdit::singleline(&mut self.preview_text)
                    .hint_text("Type to preview")
                    .desired_width(320.0),
            );
            ui.add(
                egui::Slider::new(&mut self.settings.preview_size, 24.0..=160.0)
                    .show_value(false)
                    .text("Size"),
            );
        });
        let size = self.settings.preview_size;
        let height = size * 1.45;
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, CornerRadius::same(3), color(BONE));
        let span = (self.ascender - self.descender).abs().max(1.0);
        let scale = f64::from(size) / span;
        let baseline =
            f64::from(rect.top()) + f64::from(height - size) / 2.0 + self.ascender * scale;
        let mut x = f64::from(rect.left()) + 12.0;
        let mut clicked = None;
        let pointer = response
            .clicked()
            .then(|| response.interact_pointer_pos())
            .flatten();
        let text = self.preview_text.clone();
        for ch in text.chars() {
            if x > f64::from(rect.right()) {
                break;
            }
            let Some(name) = self.by_unicode.get(&u32::from(ch)).cloned() else {
                // A missing character shows as a hollow box.
                let width = span * 0.5 * scale;
                let missing = Rect::from_min_size(
                    Pos2::new(x as f32 + 2.0, (baseline - self.cap_height * scale) as f32),
                    Vec2::new(width as f32 - 4.0, (self.cap_height * scale) as f32),
                );
                painter.rect_stroke(
                    missing,
                    CornerRadius::ZERO,
                    Stroke::new(1.0, color(MUTED)),
                    egui::StrokeKind::Inside,
                );
                x += width;
                continue;
            };
            let Some(outline) = self.outline(&name) else {
                continue;
            };
            let view = Viewport {
                scale,
                origin: Pt::new(x, baseline),
            };
            paint_fill(&painter, &view, rect, &outline, Color32::BLACK);
            let advance = outline.advance * scale;
            if self.current.as_deref() == Some(name.as_str()) {
                // Underline the glyph being edited.
                let y = (baseline - self.descender * scale * 0.5) as f32;
                painter.hline(
                    (x as f32)..=((x + advance) as f32),
                    y,
                    Stroke::new(2.0, color(FOCUS)),
                );
            }
            if let Some(press) = pointer
                && f64::from(press.x) >= x
                && f64::from(press.x) < x + advance
            {
                clicked = Some(name.clone());
            }
            x += advance;
        }
        if let Some(name) = clicked {
            self.select_glyph(Some(name));
            if self.mode == Mode::Editor {
                self.view = None;
            }
        }
    }
}

fn matches_filter(filter: &str, name: &str, unicode: Option<u32>) -> bool {
    let filter = filter.trim();
    if filter.is_empty() {
        return true;
    }
    let mut chars = filter.chars();
    if let (Some(only), None) = (chars.next(), chars.next())
        && unicode == Some(u32::from(only))
    {
        return true;
    }
    name.to_lowercase().contains(&filter.to_lowercase())
}
