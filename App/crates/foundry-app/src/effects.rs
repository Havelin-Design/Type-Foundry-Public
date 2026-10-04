//! The Effects dialog: affine effects with a live preview, applied as one `transform` command.

use std::collections::BTreeSet;

use eframe::egui;
use foundry_app::palette::MUTED;
use foundry_app::{Anchor, Handle, Matrix, Outline, matrix};
use serde_json::json;

use crate::app::{FoundryWindow, Scope, Tone};
use crate::color;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Effect {
    Slant,
    Scale,
    Rotate,
    Move,
    FlipHorizontal,
    FlipVertical,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    Selection,
    Glyph,
    AllGlyphs,
}

/// Menu shortcuts that open the dialog on one effect.
pub const QUICK: [(&str, Effect); 6] = [
    ("Slant…", Effect::Slant),
    ("Scale…", Effect::Scale),
    ("Rotate…", Effect::Rotate),
    ("Move…", Effect::Move),
    ("Flip horizontal", Effect::FlipHorizontal),
    ("Flip vertical", Effect::FlipVertical),
];

pub struct EffectsState {
    pub open: bool,
    pub effect: Effect,
    pub scope: Target,
    pub slant: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    pub uniform: bool,
    pub scale_advance: bool,
    pub from_center: bool,
    pub rotate: f64,
    pub dx: f64,
    pub dy: f64,
}

impl Default for EffectsState {
    fn default() -> Self {
        Self {
            open: false,
            effect: Effect::Slant,
            scope: Target::Glyph,
            slant: 12.0,
            scale_x: 100.0,
            scale_y: 100.0,
            uniform: true,
            scale_advance: true,
            from_center: false,
            rotate: 15.0,
            dx: 0.0,
            dy: 0.0,
        }
    }
}

impl EffectsState {
    pub fn quick(&mut self, effect: Effect) {
        self.effect = effect;
    }

    /// The matrix about the origin, and where to center it.
    pub fn matrix(&self) -> (Matrix, Anchor) {
        let origin = foundry_app::Pt::new(0.0, 0.0);
        match self.effect {
            Effect::Slant => (matrix::slant(self.slant), Anchor::Origin),
            Effect::Scale => {
                let sy = if self.uniform {
                    self.scale_x
                } else {
                    self.scale_y
                };
                let anchor = if self.from_center {
                    Anchor::Center
                } else {
                    Anchor::Origin
                };
                (
                    matrix::scale(self.scale_x / 100.0, sy / 100.0, origin),
                    anchor,
                )
            }
            Effect::Rotate => (matrix::rotate(self.rotate, origin), Anchor::Center),
            Effect::Move => (matrix::translate(self.dx, self.dy), Anchor::Origin),
            Effect::FlipHorizontal => (matrix::flip_horizontal(0.0), Anchor::Advance),
            Effect::FlipVertical => (matrix::flip_vertical(0.0), Anchor::Center),
        }
    }

    fn scales_advance(&self) -> bool {
        self.effect == Effect::Scale && self.scale_advance && !self.from_center
    }

    /// The current glyph as the effect would leave it.
    pub fn preview(&self, outline: &Outline, selection: &BTreeSet<Handle>) -> Option<Outline> {
        let only: Option<Vec<Handle>> = match self.scope {
            Target::Selection if selection.is_empty() => return None,
            Target::Selection => Some(selection.iter().copied().collect()),
            _ => None,
        };
        let (m, anchor) = self.matrix();
        let pivot = anchor.point(outline, only.as_deref());
        let centered = matrix::around(&m, pivot);
        let mut ghost = outline.transformed(&centered, only.as_deref());
        if self.scales_advance() {
            ghost.advance *= m[0].abs();
        }
        Some(ghost)
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        egui::ComboBox::from_label("Effect")
            .selected_text(format!("{:?}", self.effect).replace("Flip", "Flip "))
            .show_ui(ui, |ui| {
                for (label, effect) in [
                    ("Slant", Effect::Slant),
                    ("Scale", Effect::Scale),
                    ("Rotate", Effect::Rotate),
                    ("Move", Effect::Move),
                    ("Flip horizontal", Effect::FlipHorizontal),
                    ("Flip vertical", Effect::FlipVertical),
                ] {
                    ui.selectable_value(&mut self.effect, effect, label);
                }
            });
        ui.add_space(6.0);
        match self.effect {
            Effect::Slant => {
                ui.add(egui::Slider::new(&mut self.slant, -30.0..=30.0).text("Angle °"));
                ui.colored_label(
                    color(MUTED),
                    "Leans the outline about the baseline, like an oblique.",
                );
            }
            Effect::Scale => {
                ui.add(
                    egui::Slider::new(&mut self.scale_x, 10.0..=300.0).text(if self.uniform {
                        "Scale %"
                    } else {
                        "Width %"
                    }),
                );
                if !self.uniform {
                    ui.add(egui::Slider::new(&mut self.scale_y, 10.0..=300.0).text("Height %"));
                }
                ui.checkbox(&mut self.uniform, "Keep proportions");
                ui.checkbox(&mut self.from_center, "From the glyph center");
                ui.add_enabled(
                    !self.from_center,
                    egui::Checkbox::new(&mut self.scale_advance, "Scale the advance too"),
                );
            }
            Effect::Rotate => {
                ui.add(egui::Slider::new(&mut self.rotate, -180.0..=180.0).text("Angle °"));
                ui.colored_label(color(MUTED), "Turns about the center of the points.");
            }
            Effect::Move => {
                ui.horizontal(|ui| {
                    ui.label("X");
                    ui.add(egui::DragValue::new(&mut self.dx).speed(1.0));
                    ui.label("Y");
                    ui.add(egui::DragValue::new(&mut self.dy).speed(1.0));
                });
            }
            Effect::FlipHorizontal => {
                ui.colored_label(
                    color(MUTED),
                    "Mirrors left to right about the middle of the advance.",
                );
            }
            Effect::FlipVertical => {
                ui.colored_label(
                    color(MUTED),
                    "Mirrors top to bottom about the center of the points.",
                );
            }
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Apply to");
            ui.selectable_value(&mut self.scope, Target::Selection, "Selected points");
            ui.selectable_value(&mut self.scope, Target::Glyph, "This glyph");
            ui.selectable_value(&mut self.scope, Target::AllGlyphs, "All glyphs");
        });
    }
}

impl FoundryWindow {
    pub fn effects_window(&mut self, ctx: &egui::Context) {
        if !self.effects.open {
            return;
        }
        let mut open = true;
        let mut apply = false;
        egui::Window::new("Effects")
            .open(&mut open)
            .resizable(false)
            .default_pos(ctx.content_rect().right_top() + egui::vec2(-560.0, 90.0))
            .show(ctx, |ui| {
                self.effects.controls(ui);
                ui.add_space(6.0);
                ui.colored_label(
                    color(MUTED),
                    "The amber outline previews the result. Apply is one undo step.",
                );
                ui.horizontal(|ui| {
                    apply = ui.button("Apply").clicked();
                    if ui.button("Reset").clicked() {
                        let keep = (self.effects.effect, self.effects.scope);
                        self.effects = EffectsState {
                            open: true,
                            effect: keep.0,
                            scope: keep.1,
                            ..EffectsState::default()
                        };
                    }
                });
            });
        self.effects.open = open;
        if apply {
            self.apply_effect();
        }
    }

    fn apply_effect(&mut self) {
        let (m, anchor) = self.effects.matrix();
        let advance = self.effects.scales_advance();
        let (names, points, scope) = match self.effects.scope {
            Target::AllGlyphs => (json!(null), json!(null), Scope::Structure),
            target => {
                let Some(name) = self.current.clone() else {
                    self.status = ("Pick a glyph first".into(), Tone::Failed);
                    return;
                };
                let points = if target == Target::Selection {
                    if self.selection.is_empty() {
                        self.status = ("Select some points first".into(), Tone::Failed);
                        return;
                    }
                    json!(
                        self.selection
                            .iter()
                            .map(|handle| [handle.contour, handle.point])
                            .collect::<Vec<_>>()
                    )
                } else {
                    json!(null)
                };
                (json!([name.clone()]), points, Scope::Glyph(name))
            }
        };
        if let Some(data) = self.edit_json(
            json!({
                "op": "transform", "names": names, "points": points, "matrix": m,
                "advance": advance, "anchor": anchor.as_str(),
            }),
            scope,
        ) {
            let count = data["glyphs"].as_array().map_or(0, Vec::len);
            self.status = (
                format!("Applied {:?} to {count} glyph(s)", self.effects.effect),
                Tone::Done,
            );
            self.effects.open = false;
        }
    }
}
