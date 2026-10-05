//! View settings. They change how the window draws, never the font, and persist between launches.

use eframe::egui;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub handle_size: f32,
    pub fill: bool,
    pub outline: bool,
    pub metrics: bool,
    pub point_numbers: bool,
    pub coordinates: bool,
    pub snap: bool,
    pub cell_size: f32,
    pub show_glyph_list: bool,
    pub show_inspector: bool,
    pub show_preview: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            handle_size: 4.5,
            fill: true,
            outline: false,
            metrics: true,
            point_numbers: false,
            coordinates: true,
            snap: true,
            cell_size: 96.0,
            show_glyph_list: true,
            show_inspector: true,
            show_preview: true,
        }
    }
}

impl Settings {
    /// The settings window body. Returns true when a change needs thumbnails redrawn.
    pub fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        let before_cell = self.cell_size;
        ui.heading("Canvas");
        ui.checkbox(&mut self.fill, "Fill outlines");
        ui.checkbox(&mut self.outline, "Stroke outlines");
        ui.checkbox(&mut self.metrics, "Metric lines");
        ui.checkbox(&mut self.point_numbers, "Point numbers");
        ui.checkbox(&mut self.coordinates, "Coordinates of the selected point");
        ui.checkbox(&mut self.snap, "Snap moves to whole units");
        ui.add(egui::Slider::new(&mut self.handle_size, 2.5..=9.0).text("Handle size"));
        ui.add_space(8.0);
        ui.heading("Overview");
        ui.add(egui::Slider::new(&mut self.cell_size, 48.0..=200.0).text("Cell size"));
        ui.add_space(8.0);
        ui.heading("Panels");
        ui.checkbox(&mut self.show_glyph_list, "Glyph list");
        ui.checkbox(&mut self.show_inspector, "Inspector");
        ui.checkbox(&mut self.show_preview, "Preview pane");
        ui.add_space(8.0);
        if ui.button("Reset to defaults").clicked() {
            *self = Self::default();
        }
        (self.cell_size - before_cell).abs() > f32::EPSILON
    }
}
