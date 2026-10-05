//! Several open fonts and families: the tab row, styles, and family files. Every change is a
//! session command; the window only keeps which file each font came from.

use eframe::egui;
use foundry_api::Command;
use foundry_app::palette::{ALERT, AMBER, MUTED, SIGNAL};
use serde_json::{Value, json};

use crate::app::{FoundryWindow, Mode, Scope, Tone};
use crate::color;

/// Inspector text fields for the style, kept while they are being typed in.
#[derive(Default)]
pub struct StyleFields {
    pub family: String,
    pub name: String,
    pub loaded_for: Option<u32>,
}

pub struct NewStyle {
    pub style: String,
    pub weight: u16,
    pub italic: bool,
    pub slant: f64,
}

pub struct ExportForm {
    pub format: &'static str,
}

/// The family state the window shows in the check report.
pub struct Report {
    pub title: String,
    pub data: Value,
}

impl FoundryWindow {
    /// Tabs for every open font, grouped by family, plus New style.
    pub fn tab_row(&mut self, ui: &mut egui::Ui) {
        let families: std::collections::BTreeSet<&str> =
            self.tabs.iter().map(|tab| tab.family.as_str()).collect();
        let one_family = families.len() <= 1;
        let mut switch = None;
        let mut close = None;
        ui.horizontal(|ui| {
            if let Some(family) = self.tabs.first().map(|tab| tab.family.clone())
                && one_family
            {
                ui.strong(family);
                ui.separator();
            }
            for tab in &self.tabs {
                let active = Some(tab.id) == self.active;
                let mut label = if one_family {
                    tab.style.clone()
                } else {
                    format!("{} {}", tab.family, tab.style)
                };
                if tab.dirty {
                    label.push_str(" •");
                }
                let response = ui
                    .selectable_label(active, label)
                    .on_hover_text("Click to edit this style. Ctrl+Tab cycles styles.");
                if response.clicked() {
                    switch = Some(tab.id);
                }
                if ui
                    .small_button("×")
                    .on_hover_text("Close this font")
                    .clicked()
                {
                    close = Some(tab.id);
                }
                ui.add_space(6.0);
            }
            ui.separator();
            if ui
                .button("New style…")
                .on_hover_text("Copy this font as another style of the family, such as an Italic")
                .clicked()
            {
                self.open_new_style();
            }
        });
        if let Some(id) = switch {
            self.switch_to(id);
        }
        if let Some(id) = close {
            self.request_close(id);
        }
    }

    /// The compare picker for the toolbar: a font drawn behind the editor.
    pub fn compare_picker(&mut self, ui: &mut egui::Ui) {
        if self.tabs.len() < 2 {
            return;
        }
        let label = |window: &Self, id: Option<u32>| match id {
            None => "Nothing behind".to_string(),
            Some(id) => window
                .tabs
                .iter()
                .find(|tab| tab.id == id)
                .map_or_else(String::new, |tab| format!("Behind: {}", tab.style)),
        };
        let mut choice = self.compare;
        egui::ComboBox::from_id_salt("compare")
            .selected_text(label(self, choice))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut choice, None, "Nothing behind");
                for tab in &self.tabs {
                    if Some(tab.id) != self.active {
                        ui.selectable_value(
                            &mut choice,
                            Some(tab.id),
                            format!("{} {}", tab.family, tab.style),
                        );
                    }
                }
            })
            .response
            .on_hover_text("Draw another style's glyph faintly behind the one you edit");
        self.compare = choice;
    }

    pub fn open_new_style(&mut self) {
        let Some(font) = self.session.font() else {
            return;
        };
        let upright = !font.style.italic;
        self.dialogs.new_style = Some(NewStyle {
            style: if upright {
                if font.style.name == "Regular" {
                    "Italic".to_string()
                } else {
                    format!("{} Italic", font.style.name)
                }
            } else {
                "Bold".to_string()
            },
            weight: if upright { font.style.weight } else { 700 },
            italic: upright,
            slant: if upright { 12.0 } else { 0.0 },
        });
    }

    pub fn request_close(&mut self, id: u32) {
        let dirty = self.tabs.iter().any(|tab| tab.id == id && tab.dirty);
        if dirty {
            self.dialogs.close_confirm = Some(id);
        } else {
            self.close_font(id);
        }
    }

    fn close_font(&mut self, id: u32) {
        if !self.run(Command::CloseFont { id: Some(id) }).ok {
            return;
        }
        self.paths.remove(&id);
        self.outlines.retain(|(font, _), _| *font != id);
        if self.compare == Some(id) {
            self.compare = None;
        }
        if self.has_font() {
            self.font_switched();
        } else {
            self.refresh_tabs();
            self.glyphs.clear();
            self.by_unicode.clear();
            self.thumbs.clear();
            self.current = None;
            self.selection.clear();
            self.font_name.clear();
            self.mode = Mode::Overview;
        }
        self.status = ("Closed the font".into(), Tone::Quiet);
    }

    fn derive_style(&mut self, form: &NewStyle) {
        let response = self.run(Command::DeriveStyle {
            style: form.style.trim().to_string(),
            weight: Some(form.weight),
            italic: Some(form.italic),
            slant: form.slant,
        });
        if response.ok {
            self.font_switched();
            self.status = (
                format!(
                    "Made {}. It is a separate font in the family: edit it, then File > Save family.",
                    form.style.trim()
                ),
                Tone::Done,
            );
        }
    }

    pub fn open_family_dialog(&mut self) {
        let Some(path) = self
            .file_dialog()
            .add_filter("Type Foundry family", &["json"])
            .pick_file()
        else {
            return;
        };
        let response = self.run(Command::OpenFamily {
            path: path.to_string_lossy().into_owned(),
        });
        let Some(data) = response.data.filter(|_| response.ok) else {
            return;
        };
        let mut count = 0;
        for entry in data["opened"].as_array().into_iter().flatten() {
            if let (Some(id), Some(member)) = (entry["id"].as_u64(), entry["path"].as_str()) {
                self.paths.insert(id as u32, member.into());
                count += 1;
            }
        }
        self.load_new_font();
        self.status = (
            format!(
                "Opened the {} family · {count} styles",
                data["family"].as_str().unwrap_or_default()
            ),
            Tone::Done,
        );
    }

    pub fn save_family_dialog(&mut self) {
        let Some(font) = self.session.font() else {
            return;
        };
        let family = font.style.family.clone();
        let stem: String = family
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric())
            .collect();
        let Some(path) = self
            .file_dialog()
            .set_file_name(format!("{stem}.family.json"))
            .add_filter("Type Foundry family", &["json"])
            .save_file()
        else {
            return;
        };
        let members: Vec<u32> = self
            .tabs
            .iter()
            .filter(|tab| tab.family == family)
            .map(|tab| tab.id)
            .collect();
        let response = self.run(Command::SaveFamily {
            path: path.to_string_lossy().into_owned(),
            ids: Some(members.clone()),
        });
        let Some(data) = response.data.filter(|_| response.ok) else {
            return;
        };
        let written: Vec<&str> = data["styles"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        for (id, member) in members.iter().zip(&written) {
            self.paths.insert(*id, (*member).into());
        }
        self.refresh_tabs();
        self.status = (
            format!("Saved the {family} family · {} styles", written.len()),
            Tone::Done,
        );
    }

    fn export_family(&mut self, format: &str) {
        let Some(dir) = self.file_dialog().pick_folder() else {
            return;
        };
        let command =
            json!({ "op": "export_family", "dir": dir.to_string_lossy(), "format": format });
        let Ok(command) = serde_json::from_value::<Command>(command) else {
            return;
        };
        let response = self.run(command);
        if !response.ok {
            if let Some(data) = response.data {
                self.dialogs.report = Some(Report {
                    title: "Export stopped".into(),
                    data,
                });
            }
            self.family_check();
            return;
        }
        let data = response.data.unwrap_or(Value::Null);
        let count = data["files"].as_array().map_or(0, Vec::len);
        self.status = (
            format!("Exported {count} styles as .{format} to {}", dir.display()),
            Tone::Done,
        );
        if data["issues"]
            .as_array()
            .is_some_and(|issues| !issues.is_empty())
        {
            self.dialogs.report = Some(Report {
                title: "Exported, with notes".into(),
                data,
            });
        }
    }

    pub fn family_check(&mut self) {
        let response = self.run(Command::FamilyCheck { ids: None });
        if let Some(data) = response.data {
            self.dialogs.report = Some(Report {
                title: "Family check".into(),
                data,
            });
        }
    }

    fn file_dialog(&self) -> rfd::FileDialog {
        let dialog = rfd::FileDialog::new();
        match &self.last_dir {
            Some(dir) if dir.is_dir() => dialog.set_directory(dir),
            _ => dialog,
        }
    }

    /// The Style section of the inspector.
    pub fn style_section(&mut self, ui: &mut egui::Ui) {
        let Some(font) = self.session.font() else {
            return;
        };
        let style = font.style.clone();
        if self.style.loaded_for != self.active {
            self.style.family = style.family.clone();
            self.style.name = style.name.clone();
            self.style.loaded_for = self.active;
        }
        egui::Grid::new("style_fields")
            .num_columns(2)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                ui.label("Family");
                let response = ui.text_edit_singleline(&mut self.style.family);
                if response.lost_focus() && self.style.family.trim() != style.family {
                    let family = self.style.family.trim().to_string();
                    self.set_style(json!({ "family": family }));
                }
                ui.end_row();

                ui.label("Style");
                let response = ui
                    .text_edit_singleline(&mut self.style.name)
                    .on_hover_text("Regular, Italic, Bold, Light Italic…");
                if response.lost_focus() && self.style.name.trim() != style.name {
                    let name = self.style.name.trim().to_string();
                    self.set_style(json!({ "style": name }));
                }
                ui.end_row();

                ui.label("Weight");
                let mut weight = style.weight;
                let response = ui
                    .add(egui::DragValue::new(&mut weight).range(1..=1000))
                    .on_hover_text("400 Regular, 700 Bold, 300 Light, 900 Black");
                if response.drag_started() || response.gained_focus() {
                    self.checkpoint();
                }
                if response.changed() {
                    self.set_style(json!({ "weight": weight }));
                }
                ui.end_row();

                ui.label("Italic");
                let mut italic = style.italic;
                if ui.checkbox(&mut italic, "").changed() {
                    self.set_style(json!({ "italic": italic }));
                }
                ui.end_row();

                ui.label("Italic angle");
                let mut angle = style.italic_angle;
                let response = ui
                    .add(
                        egui::DragValue::new(&mut angle)
                            .speed(0.25)
                            .range(-45.0..=45.0),
                    )
                    .on_hover_text(
                        "Degrees counter-clockwise. A right-leaning italic is negative, like -12.",
                    );
                if response.drag_started() || response.gained_focus() {
                    self.checkpoint();
                }
                if response.changed() {
                    self.set_style(json!({ "italic_angle": angle }));
                }
                ui.end_row();
            });
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button("New style…").clicked() {
                self.open_new_style();
            }
            if ui.button("Check family").clicked() {
                self.family_check();
            }
        });
    }

    fn set_style(&mut self, mut fields: Value) {
        fields["op"] = json!("set_style");
        if self.edit_json(fields, Scope::Font).is_some() {
            self.style.loaded_for = None;
        }
    }

    /// Dialogs for styles and families.
    pub fn family_dialogs(&mut self, ctx: &egui::Context) {
        if let Some(mut form) = self.dialogs.new_style.take() {
            let mut keep = true;
            let mut make = false;
            egui::Window::new("New style")
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false)
                .resizable(false)
                .open(&mut keep)
                .show(ctx, |ui| {
                    ui.colored_label(
                        color(MUTED),
                        "Copies this font as another style of the family. Both stay open.",
                    );
                    egui::Grid::new("new_style").num_columns(2).show(ui, |ui| {
                        ui.label("Style name");
                        ui.text_edit_singleline(&mut form.style);
                        ui.end_row();
                        ui.label("Weight");
                        ui.add(egui::DragValue::new(&mut form.weight).range(1..=1000));
                        ui.end_row();
                        ui.label("Italic");
                        ui.checkbox(&mut form.italic, "");
                        ui.end_row();
                        ui.label("Slant");
                        ui.add(egui::Slider::new(&mut form.slant, -20.0..=20.0).suffix("°"))
                            .on_hover_text(
                                "Leans every glyph to start an italic. 0 keeps the outlines.",
                            );
                        ui.end_row();
                    });
                    make = ui.button("Make style").clicked();
                });
            if make && !form.style.trim().is_empty() {
                self.derive_style(&form);
            } else if keep {
                self.dialogs.new_style = Some(form);
            }
        }

        if let Some(mut form) = self.dialogs.export.take() {
            let mut keep = true;
            let mut go = false;
            egui::Window::new("Export family")
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false)
                .resizable(false)
                .open(&mut keep)
                .show(ctx, |ui| {
                    let family = self
                        .session
                        .font()
                        .map(|font| font.style.family.clone())
                        .unwrap_or_default();
                    let styles: Vec<String> = self
                        .tabs
                        .iter()
                        .filter(|tab| tab.family == family)
                        .map(|tab| tab.style.clone())
                        .collect();
                    ui.label(format!("{family}: {}", styles.join(", ")));
                    ui.colored_label(
                        color(MUTED),
                        "Each style is written as Family-Style with matching family names, so apps group them.",
                    );
                    ui.horizontal(|ui| {
                        ui.label("Format");
                        ui.selectable_value(&mut form.format, "ttf", "TrueType .ttf");
                        ui.selectable_value(&mut form.format, "ufo", "UFO");
                        ui.selectable_value(&mut form.format, "json", "Type Foundry .json");
                    });
                    go = ui.button("Choose folder and export…").clicked();
                });
            if go {
                self.export_family(form.format);
            } else if keep {
                self.dialogs.export = Some(form);
            }
        }

        if let Some(id) = self.dialogs.close_confirm {
            let mut keep = true;
            let mut choice = None;
            let name = self
                .tabs
                .iter()
                .find(|tab| tab.id == id)
                .map(|tab| format!("{} {}", tab.family, tab.style))
                .unwrap_or_default();
            egui::Window::new("Unsaved changes")
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false)
                .resizable(false)
                .open(&mut keep)
                .show(ctx, |ui| {
                    ui.label(format!("{name} has changes that are not saved."));
                    ui.horizontal(|ui| {
                        if ui.button("Close without saving").clicked() {
                            choice = Some(true);
                        }
                        if ui.button("Keep it open").clicked() {
                            choice = Some(false);
                        }
                    });
                });
            if choice == Some(true) {
                self.dialogs.close_confirm = None;
                self.close_font(id);
            } else if choice == Some(false) || !keep {
                self.dialogs.close_confirm = None;
            }
        }

        if let Some(report) = &self.dialogs.report {
            let mut keep = true;
            let title = report.title.clone();
            let data = report.data.clone();
            egui::Window::new(title)
                .pivot(egui::Align2::CENTER_CENTER)
                .default_pos(ctx.content_rect().center())
                .collapsible(false)
                .default_width(460.0)
                .open(&mut keep)
                .show(ctx, |ui| report_body(ui, &data));
            if !keep {
                self.dialogs.report = None;
            }
        }
    }
}

fn report_body(ui: &mut egui::Ui, data: &Value) {
    if let Some(styles) = data["styles"].as_array() {
        let names: Vec<&str> = styles.iter().filter_map(Value::as_str).collect();
        ui.label(format!("Styles: {}", names.join(", ")));
    }
    let issues = data["issues"].as_array().cloned().unwrap_or_default();
    let blocking = issues
        .iter()
        .filter(|issue| issue["blocking"].as_bool() == Some(true))
        .count();
    if issues.is_empty() {
        ui.colored_label(color(SIGNAL), "Ready to export. No issues found.");
        return;
    }
    if blocking > 0 {
        ui.colored_label(
            color(ALERT),
            format!("{blocking} issue(s) block an export. Fix these first."),
        );
    } else {
        ui.colored_label(color(SIGNAL), "Ready to export. Notes below.");
    }
    ui.add_space(4.0);
    egui::ScrollArea::vertical()
        .max_height(320.0)
        .show(ui, |ui| {
            for issue in &issues {
                let tint = if issue["blocking"].as_bool() == Some(true) {
                    color(ALERT)
                } else if issue["code"] == "missing" {
                    color(AMBER)
                } else {
                    color(MUTED)
                };
                ui.horizontal_wrapped(|ui| {
                    ui.colored_label(tint, issue["code"].as_str().unwrap_or("note"));
                    ui.label(issue["detail"].as_str().unwrap_or_default());
                });
            }
        });
}
