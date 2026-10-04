//! Window state, the menu bar, keyboard shortcuts, dialogs, and the bridge to the session.
//! Every change to the font is a `Command`; the window only keeps caches of what it read.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use eframe::egui::{self, Key, KeyboardShortcut, Modifiers, Stroke};
use foundry_api::{Command, Response, Session};
use foundry_app::palette::{ALERT, HAIRLINE, MUTED, PANEL, SIGNAL};
use foundry_app::{Handle, Outline, Pt, Viewport};
use serde_json::{Value, json};

use crate::effects::EffectsState;
use crate::settings::Settings;
use crate::{APP_TITLE, LAST_DIR_KEY, SETTINGS_KEY, color};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Overview,
    Editor,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Select,
    Pen,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Quiet,
    Done,
    Failed,
}

/// What an edit may have changed, so the window knows which caches to drop.
pub enum Scope {
    /// One glyph's outline or advance.
    Glyph(String),
    /// Glyph names, order, or Unicode values, or anything after undo and redo.
    Structure,
    /// Font name or metrics.
    Font,
}

pub struct GlyphEntry {
    pub name: String,
    pub unicode: Option<u32>,
}

pub enum Drag {
    None,
    /// Moving the selection. `last` is the font position already applied.
    Points {
        last: Pt,
    },
    Marquee {
        start: Pt,
        now: Pt,
        additive: bool,
    },
}

pub struct NewFont {
    pub name: String,
    pub upm: u16,
}

pub struct NewGlyph {
    pub name: String,
    pub unicode: String,
    pub advance: f64,
}

#[derive(Default)]
pub struct Dialogs {
    pub new_font: Option<NewFont>,
    pub new_glyph: Option<NewGlyph>,
    pub settings: bool,
    pub shortcuts: bool,
    pub about: bool,
}

pub struct FoundryWindow {
    pub session: Session,
    /// The file the font came from or was last saved to. Held here, not in the session.
    pub path: Option<PathBuf>,
    pub last_dir: Option<PathBuf>,
    pub font_name: String,
    pub upm: u16,
    pub ascender: f64,
    pub descender: f64,
    pub x_height: f64,
    pub cap_height: f64,
    pub glyphs: Vec<GlyphEntry>,
    pub by_unicode: HashMap<u32, String>,
    pub outlines: HashMap<String, Outline>,
    pub thumbs: HashMap<String, egui::TextureHandle>,
    pub mode: Mode,
    pub tool: Tool,
    pub current: Option<String>,
    pub selection: BTreeSet<Handle>,
    pub view: Option<Viewport>,
    pub canvas_rect: egui::Rect,
    pub drag: Drag,
    /// The open contour the pen is adding to.
    pub pen_contour: Option<usize>,
    pub settings: Settings,
    pub effects: EffectsState,
    pub dialogs: Dialogs,
    pub preview_text: String,
    pub filter: String,
    pub status: (String, Tone),
    pub dirty: bool,
    shown_title: String,
    // Inspector text fields, kept between frames while they are being typed in.
    pub font_name_edit: String,
    pub glyph_name_edit: String,
    pub unicode_edit: String,
    pub edits_for: Option<String>,
}

impl FoundryWindow {
    pub fn new(last_dir: Option<PathBuf>, settings: Settings) -> Self {
        Self {
            session: Session::new(),
            path: None,
            last_dir,
            font_name: String::new(),
            upm: 1000,
            ascender: 800.0,
            descender: -200.0,
            x_height: 500.0,
            cap_height: 700.0,
            glyphs: Vec::new(),
            by_unicode: HashMap::new(),
            outlines: HashMap::new(),
            thumbs: HashMap::new(),
            mode: Mode::Overview,
            tool: Tool::Select,
            current: None,
            selection: BTreeSet::new(),
            view: None,
            canvas_rect: egui::Rect::NOTHING,
            drag: Drag::None,
            pen_contour: None,
            settings,
            effects: EffectsState::default(),
            dialogs: Dialogs::default(),
            preview_text: "Hamburgefonstiv 0123".to_string(),
            filter: String::new(),
            status: (
                "File > Open a font, or File > New font. Ctrl+O opens.".to_string(),
                Tone::Quiet,
            ),
            dirty: false,
            shown_title: String::new(),
            font_name_edit: String::new(),
            glyph_name_edit: String::new(),
            unicode_edit: String::new(),
            edits_for: None,
        }
    }

    pub fn has_font(&self) -> bool {
        self.session.font().is_some()
    }

    // ---- The session bridge -------------------------------------------------------------

    /// Run a read-only command. Failures go to the status bar.
    pub fn run(&mut self, command: Command) -> Response {
        let response = self.session.execute(command);
        if !response.ok {
            let message = response
                .error
                .clone()
                .unwrap_or_else(|| "command failed".to_string());
            self.status = (message, Tone::Failed);
        }
        response
    }

    /// Run a command that changes the font, then drop what it made stale.
    pub fn edit(&mut self, command: Command, scope: Scope) -> Option<Value> {
        let response = self.run(command);
        if !response.ok {
            return None;
        }
        self.dirty = true;
        match scope {
            Scope::Glyph(name) => self.forget(&name),
            Scope::Structure => {
                self.refresh_index();
                self.refresh_info();
                self.outlines.clear();
                self.thumbs.clear();
                if self
                    .current
                    .as_ref()
                    .is_some_and(|name| !self.glyphs.iter().any(|entry| &entry.name == name))
                {
                    self.current = self.glyphs.first().map(|entry| entry.name.clone());
                    self.selection.clear();
                    self.view = None;
                }
                self.pen_contour = None;
                self.prune_selection();
                self.edits_for = None;
            }
            Scope::Font => {
                self.refresh_info();
                self.thumbs.clear();
            }
        }
        Some(response.data.unwrap_or(Value::Null))
    }

    /// Edit with a JSON command, for the many small edits the panels make.
    pub fn edit_json(&mut self, command: Value, scope: Scope) -> Option<Value> {
        match serde_json::from_value::<Command>(command) {
            Ok(command) => self.edit(command, scope),
            Err(err) => {
                self.status = (format!("internal command error: {err}"), Tone::Failed);
                None
            }
        }
    }

    pub fn checkpoint(&mut self) {
        self.session.execute(Command::Checkpoint);
    }

    fn forget(&mut self, name: &str) {
        self.outlines.remove(name);
        self.thumbs.remove(name);
        self.prune_selection();
    }

    fn prune_selection(&mut self) {
        let Some(outline) = self.current_outline() else {
            self.selection.clear();
            return;
        };
        self.selection
            .retain(|handle| outline.point(*handle).is_some());
        if let Some(contour) = self.pen_contour
            && outline
                .contours
                .get(contour)
                .is_none_or(|found| found.closed)
        {
            self.pen_contour = None;
        }
    }

    pub fn refresh_info(&mut self) {
        let response = self.session.execute(Command::Info);
        let Some(info) = response.data else {
            return;
        };
        self.font_name = info["name"].as_str().unwrap_or_default().to_string();
        self.upm = info["upm"]
            .as_u64()
            .and_then(|upm| u16::try_from(upm).ok())
            .unwrap_or(1000);
        let metric = |key: &str, fallback: f64| info["metrics"][key].as_f64().unwrap_or(fallback);
        self.ascender = metric("ascender", 800.0);
        self.descender = metric("descender", -200.0);
        self.x_height = metric("x_height", 500.0);
        self.cap_height = metric("cap_height", 700.0);
        self.font_name_edit = self.font_name.clone();
    }

    pub fn refresh_index(&mut self) {
        let response = self.session.execute(Command::Index);
        let entries = response
            .data
            .as_ref()
            .and_then(|data| data["glyphs"].as_array().cloned())
            .unwrap_or_default();
        self.glyphs = entries
            .iter()
            .filter_map(|entry| {
                Some(GlyphEntry {
                    name: entry["name"].as_str()?.to_string(),
                    unicode: entry["unicode"]
                        .as_u64()
                        .and_then(|code| u32::try_from(code).ok()),
                })
            })
            .collect();
        self.by_unicode = self
            .glyphs
            .iter()
            .filter_map(|entry| entry.unicode.map(|code| (code, entry.name.clone())))
            .collect();
    }

    /// The outline of a glyph, read through the `glyph` command and cached.
    pub fn outline(&mut self, name: &str) -> Option<Outline> {
        if let Some(found) = self.outlines.get(name) {
            return Some(found.clone());
        }
        let response = self.session.execute(Command::Glyph {
            name: name.to_string(),
        });
        let outline = response.data.as_ref().and_then(Outline::from_json)?;
        self.outlines.insert(name.to_string(), outline.clone());
        Some(outline)
    }

    pub fn current_outline(&mut self) -> Option<Outline> {
        let name = self.current.clone()?;
        self.outline(&name)
    }

    pub fn select_glyph(&mut self, name: Option<String>) {
        if self.current != name {
            self.selection.clear();
            self.pen_contour = None;
            self.view = None;
            self.drag = Drag::None;
        }
        self.current = name;
    }

    pub fn open_editor(&mut self, name: String) {
        self.select_glyph(Some(name));
        self.mode = Mode::Editor;
    }

    pub fn step_glyph(&mut self, by: isize) {
        if self.glyphs.is_empty() {
            return;
        }
        let index = self
            .current
            .as_ref()
            .and_then(|name| self.glyphs.iter().position(|entry| &entry.name == name))
            .unwrap_or(0) as isize;
        let next = (index + by).rem_euclid(self.glyphs.len() as isize) as usize;
        let name = self.glyphs[next].name.clone();
        self.select_glyph(Some(name));
    }

    // ---- Files ------------------------------------------------------------------------

    fn load_new_font(&mut self, data: Option<&Value>) {
        self.refresh_info();
        self.refresh_index();
        self.outlines.clear();
        self.thumbs.clear();
        self.selection.clear();
        self.pen_contour = None;
        self.view = None;
        self.current = data
            .and_then(|data| data["glyphs"].get(0))
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| self.glyphs.first().map(|entry| entry.name.clone()));
        self.mode = Mode::Overview;
        self.dirty = false;
        self.edits_for = None;
    }

    pub fn open(&mut self, path: PathBuf) {
        let response = self.run(Command::Open {
            path: path.to_string_lossy().into_owned(),
        });
        if !response.ok {
            return;
        }
        self.remember_dir(&path);
        self.path = Some(path.clone());
        self.load_new_font(response.data.as_ref());
        self.status = (
            format!("Opened {} · {} glyphs", path.display(), self.glyphs.len()),
            Tone::Done,
        );
    }

    pub fn create(&mut self, name: String, upm: u16) {
        let response = self.run(Command::Create { name, upm });
        if response.ok {
            self.path = None;
            self.load_new_font(response.data.as_ref());
            self.status = (
                "New font. Glyph > New glyph adds the first one.".to_string(),
                Tone::Done,
            );
        }
    }

    pub fn save_to(&mut self, path: PathBuf) {
        let response = self.run(Command::Save {
            path: path.to_string_lossy().into_owned(),
        });
        if response.ok {
            self.remember_dir(&path);
            self.status = (format!("Saved {}", path.display()), Tone::Done);
            self.path = Some(path);
            self.dirty = false;
        }
    }

    pub fn save(&mut self) {
        match self.path.clone() {
            Some(path) if is_writable(&path) => self.save_to(path),
            _ => self.save_as_dialog(),
        }
    }

    fn remember_dir(&mut self, path: &Path) {
        if let Some(parent) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            self.last_dir = Some(parent.to_path_buf());
        }
    }

    fn dialog(&self) -> rfd::FileDialog {
        let dialog = rfd::FileDialog::new();
        match &self.last_dir {
            Some(dir) if dir.is_dir() => dialog.set_directory(dir),
            _ => dialog,
        }
    }

    pub fn open_file_dialog(&mut self) {
        if let Some(path) = self
            .dialog()
            .add_filter(
                "All fonts",
                &[
                    "json", "js", "ttf", "otf", "ttc", "otc", "woff", "woff2", "eot",
                ],
            )
            .add_filter("Type Foundry or typeface JSON", &["json", "js"])
            .add_filter("TrueType or OpenType", &["ttf", "otf", "ttc", "otc"])
            .add_filter("Web fonts", &["woff", "woff2", "eot"])
            .pick_file()
        {
            self.open(path);
        }
    }

    pub fn open_ufo_dialog(&mut self) {
        if let Some(path) = self.dialog().pick_folder() {
            if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("ufo"))
            {
                self.open(path);
            } else {
                self.status = (
                    format!("{} is not a .ufo folder", path.display()),
                    Tone::Failed,
                );
            }
        }
    }

    pub fn save_as_dialog(&mut self) {
        let stem = self
            .path
            .as_ref()
            .and_then(|path| path.file_stem())
            .map(|name| format!("{}.json", name.to_string_lossy()))
            .unwrap_or_else(|| format!("{}.json", self.font_name));
        if let Some(path) = self
            .dialog()
            .set_file_name(stem)
            .add_filter("Type Foundry font", &["json"])
            .add_filter("UFO", &["ufo"])
            .add_filter("TrueType", &["ttf"])
            .save_file()
        {
            self.save_to(path);
        }
    }

    // ---- Shared edits used by menus, shortcuts, and panels -----------------------------

    pub fn undo(&mut self) {
        if self.edit(Command::Undo, Scope::Structure).is_some() {
            self.status = ("Undone".into(), Tone::Quiet);
        }
    }

    pub fn redo(&mut self) {
        if self.edit(Command::Redo, Scope::Structure).is_some() {
            self.status = ("Redone".into(), Tone::Quiet);
        }
    }

    pub fn select_all_points(&mut self) {
        if let Some(outline) = self.current_outline() {
            self.selection = outline.handles().into_iter().collect();
        }
    }

    pub fn delete_selection(&mut self) {
        let Some(name) = self.current.clone() else {
            return;
        };
        if self.selection.is_empty() {
            return;
        }
        let points: Vec<[usize; 2]> = self
            .selection
            .iter()
            .map(|handle| [handle.contour, handle.point])
            .collect();
        let count = points.len();
        if self
            .edit_json(
                json!({ "op": "delete_points", "name": name, "points": points }),
                Scope::Glyph(name.clone()),
            )
            .is_some()
        {
            self.selection.clear();
            self.pen_contour = None;
            self.status = (format!("Deleted {count} points"), Tone::Quiet);
        }
    }

    pub fn nudge(&mut self, dx: f64, dy: f64) {
        let Some(name) = self.current.clone() else {
            return;
        };
        if self.selection.is_empty() {
            return;
        }
        let points: Vec<[usize; 2]> = self
            .selection
            .iter()
            .map(|handle| [handle.contour, handle.point])
            .collect();
        self.edit_json(
            json!({ "op": "move_points", "name": name, "points": points, "dx": dx, "dy": dy }),
            Scope::Glyph(name),
        );
    }

    /// Set the smooth flag on every selected on-curve point.
    pub fn set_smooth(&mut self, smooth: bool) {
        let Some(name) = self.current.clone() else {
            return;
        };
        let Some(outline) = self.current_outline() else {
            return;
        };
        let targets: Vec<Handle> = self
            .selection
            .iter()
            .copied()
            .filter(|handle| outline.point(*handle).is_some_and(|point| point.on))
            .collect();
        for handle in targets {
            self.edit_json(
                json!({
                    "op": "set_point", "name": name, "contour": handle.contour,
                    "point": handle.point, "smooth": smooth,
                }),
                Scope::Glyph(name.clone()),
            );
        }
    }

    pub fn set_kind(&mut self, on: bool) {
        let Some(name) = self.current.clone() else {
            return;
        };
        let targets: Vec<Handle> = self.selection.iter().copied().collect();
        for handle in targets {
            self.edit_json(
                json!({
                    "op": "set_point", "name": name, "contour": handle.contour,
                    "point": handle.point, "kind": if on { "on" } else { "off" },
                }),
                Scope::Glyph(name.clone()),
            );
        }
    }

    /// Reverse the contours that hold a selected point, or every contour with none selected.
    pub fn reverse_contours(&mut self) {
        let Some(name) = self.current.clone() else {
            return;
        };
        let Some(outline) = self.current_outline() else {
            return;
        };
        let contours: BTreeSet<usize> = if self.selection.is_empty() {
            (0..outline.contours.len()).collect()
        } else {
            self.selection.iter().map(|handle| handle.contour).collect()
        };
        for contour in contours {
            self.edit_json(
                json!({ "op": "reverse_contour", "name": name, "contour": contour }),
                Scope::Glyph(name.clone()),
            );
        }
        self.selection.clear();
    }

    pub fn round_glyphs(&mut self, all: bool) {
        let names = if all {
            Value::Null
        } else {
            match &self.current {
                Some(name) => json!([name]),
                None => return,
            }
        };
        if let Some(data) = self.edit_json(
            json!({ "op": "round_coordinates", "names": names }),
            Scope::Structure,
        ) {
            let count = data["glyphs"].as_array().map_or(0, Vec::len);
            self.status = (format!("Rounded {count} glyphs"), Tone::Done);
        }
    }

    pub fn delete_current_glyph(&mut self) {
        let Some(name) = self.current.clone() else {
            return;
        };
        if self
            .edit_json(
                json!({ "op": "delete_glyph", "name": name }),
                Scope::Structure,
            )
            .is_some()
        {
            self.status = (format!("Deleted glyph {name}"), Tone::Quiet);
        }
    }

    pub fn zoom(&mut self, factor: f64) {
        if let Some(view) = &mut self.view {
            let center = self.canvas_rect.center();
            view.zoom_at(Pt::new(f64::from(center.x), f64::from(center.y)), factor);
        }
    }

    // ---- Menus --------------------------------------------------------------------------

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let has_font = self.has_font();
        let has_glyph = self.current.is_some();
        let has_selection = !self.selection.is_empty();
        let (undo, redo) = self.session.history();
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if item(ui, "New font…", "Ctrl+N", true) {
                    self.dialogs.new_font = Some(NewFont {
                        name: "Untitled".into(),
                        upm: 1000,
                    });
                }
                if item(ui, "Open…", "Ctrl+O", true) {
                    self.open_file_dialog();
                }
                if item(ui, "Open UFO folder…", "", true) {
                    self.open_ufo_dialog();
                }
                ui.separator();
                if item(ui, "Save", "Ctrl+S", has_font) {
                    self.save();
                }
                if item(ui, "Save As…", "Ctrl+Shift+S", has_font) {
                    self.save_as_dialog();
                }
                ui.separator();
                if item(ui, "Quit", "Ctrl+Q", true) {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Edit", |ui| {
                if item(ui, &format!("Undo ({undo})"), "Ctrl+Z", undo > 0) {
                    self.undo();
                }
                if item(ui, &format!("Redo ({redo})"), "Ctrl+Shift+Z", redo > 0) {
                    self.redo();
                }
                ui.separator();
                if item(ui, "Select all points", "Ctrl+A", has_glyph) {
                    self.select_all_points();
                }
                if item(ui, "Deselect", "Esc", has_selection) {
                    self.selection.clear();
                }
                if item(ui, "Delete points", "Del", has_selection) {
                    self.delete_selection();
                }
                ui.separator();
                if item(ui, "Make smooth", "", has_selection) {
                    self.set_smooth(true);
                }
                if item(ui, "Make corner", "", has_selection) {
                    self.set_smooth(false);
                }
                if item(ui, "Make on-curve", "", has_selection) {
                    self.set_kind(true);
                }
                if item(ui, "Make off-curve", "", has_selection) {
                    self.set_kind(false);
                }
                if item(ui, "Reverse contour direction", "", has_glyph) {
                    self.reverse_contours();
                }
            });
            ui.menu_button("View", |ui| {
                if item(ui, "Font overview", "Ctrl+1", has_font) {
                    self.mode = Mode::Overview;
                }
                if item(ui, "Glyph editor", "Ctrl+2", has_glyph) {
                    self.mode = Mode::Editor;
                }
                ui.separator();
                if item(ui, "Zoom in", "Ctrl+=", has_glyph) {
                    self.zoom(1.25);
                }
                if item(ui, "Zoom out", "Ctrl+-", has_glyph) {
                    self.zoom(0.8);
                }
                if item(ui, "Fit glyph", "Ctrl+0", has_glyph) {
                    self.view = None;
                }
                ui.separator();
                ui.checkbox(&mut self.settings.show_glyph_list, "Glyph list");
                ui.checkbox(&mut self.settings.show_inspector, "Inspector");
                ui.checkbox(&mut self.settings.show_preview, "Preview strip");
                ui.separator();
                ui.checkbox(&mut self.settings.fill, "Fill");
                ui.checkbox(&mut self.settings.outline, "Outline stroke");
                ui.checkbox(&mut self.settings.metrics, "Metrics");
                ui.checkbox(&mut self.settings.point_numbers, "Point numbers");
                ui.separator();
                if item(ui, "Settings…", "Ctrl+,", true) {
                    self.dialogs.settings = true;
                }
            });
            ui.menu_button("Glyph", |ui| {
                if item(ui, "New glyph…", "Ctrl+Shift+N", has_font) {
                    self.dialogs.new_glyph = Some(NewGlyph {
                        name: String::new(),
                        unicode: String::new(),
                        advance: f64::from(self.upm) * 0.6,
                    });
                }
                if item(ui, "Delete glyph", "", has_glyph) {
                    self.delete_current_glyph();
                }
                ui.separator();
                if item(ui, "Previous glyph", "[", has_font) {
                    self.step_glyph(-1);
                }
                if item(ui, "Next glyph", "]", has_font) {
                    self.step_glyph(1);
                }
            });
            ui.menu_button("Tools", |ui| {
                if ui.radio(self.tool == Tool::Select, "Select   V").clicked() {
                    self.tool = Tool::Select;
                    ui.close();
                }
                if ui.radio(self.tool == Tool::Pen, "Pen   P").clicked() {
                    self.tool = Tool::Pen;
                    self.mode = Mode::Editor;
                    ui.close();
                }
            });
            ui.menu_button("Effects", |ui| {
                if item(ui, "Transform…", "Ctrl+E", has_font) {
                    self.effects.open = true;
                }
                ui.separator();
                for (label, effect) in crate::effects::QUICK {
                    if item(ui, label, "", has_glyph) {
                        self.effects.quick(effect);
                        self.effects.open = true;
                    }
                }
                ui.separator();
                if item(ui, "Round coordinates (glyph)", "", has_glyph) {
                    self.round_glyphs(false);
                }
                if item(ui, "Round coordinates (all glyphs)", "", has_font) {
                    self.round_glyphs(true);
                }
            });
            ui.menu_button("Help", |ui| {
                if item(ui, "Keyboard shortcuts", "F1", true) {
                    self.dialogs.shortcuts = true;
                }
                if item(ui, "About Type Foundry", "", true) {
                    self.dialogs.about = true;
                }
            });
        });
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.mode, Mode::Overview, "Overview");
            if ui
                .add_enabled(
                    self.current.is_some(),
                    egui::Button::selectable(self.mode == Mode::Editor, "Editor"),
                )
                .clicked()
            {
                self.mode = Mode::Editor;
            }
            ui.separator();
            ui.selectable_value(&mut self.tool, Tool::Select, "Select")
                .on_hover_text("V · drag points, drag empty space to box-select, Alt-click an outline to add a point");
            if ui
                .selectable_value(&mut self.tool, Tool::Pen, "Pen")
                .on_hover_text("P · click to add corner points, Shift-click for off-curve, click the first point to close")
                .clicked()
            {
                self.mode = Mode::Editor;
            }
            ui.separator();
            let (undo, redo) = self.session.history();
            if ui
                .add_enabled(undo > 0, egui::Button::new("Undo"))
                .clicked()
            {
                self.undo();
            }
            if ui
                .add_enabled(redo > 0, egui::Button::new("Redo"))
                .clicked()
            {
                self.redo();
            }
            ui.separator();
            if ui
                .add_enabled(self.has_font(), egui::Button::new("Effects…"))
                .clicked()
            {
                self.effects.open = true;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(name) = &self.current {
                    ui.weak(name);
                }
                if !self.font_name.is_empty() {
                    let marker = if self.dirty { " •" } else { "" };
                    ui.strong(format!("{}{marker}", self.font_name));
                }
            });
        });
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let (text, tone) = &self.status;
            let tint = match tone {
                Tone::Quiet => color(MUTED),
                Tone::Done => color(SIGNAL),
                Tone::Failed => color(ALERT),
            };
            ui.colored_label(tint, text);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(view) = &self.view
                    && self.mode == Mode::Editor
                {
                    ui.weak(format!("{:.0}%", view.scale * 100.0));
                }
                if !self.selection.is_empty() {
                    ui.weak(format!("{} selected", self.selection.len()));
                }
                ui.weak(match self.tool {
                    Tool::Select => "Select",
                    Tool::Pen => "Pen",
                });
            });
        });
    }

    // ---- Keyboard ---------------------------------------------------------------------

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let command = |key| KeyboardShortcut::new(Modifiers::COMMAND, key);
        let command_shift = |key| KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, key);
        let pressed = |shortcut: KeyboardShortcut| ctx.input_mut(|i| i.consume_shortcut(&shortcut));

        // Check the shifted forms first, so Ctrl+Shift+S is not taken as Ctrl+S.
        if pressed(command_shift(Key::S)) && self.has_font() {
            self.save_as_dialog();
        }
        if pressed(command_shift(Key::Z)) {
            self.redo();
        }
        if pressed(command_shift(Key::N)) && self.has_font() {
            self.dialogs.new_glyph = Some(NewGlyph {
                name: String::new(),
                unicode: String::new(),
                advance: f64::from(self.upm) * 0.6,
            });
        }
        if pressed(command(Key::N)) {
            self.dialogs.new_font = Some(NewFont {
                name: "Untitled".into(),
                upm: 1000,
            });
        }
        if pressed(command(Key::O)) {
            self.open_file_dialog();
        }
        if pressed(command(Key::S)) && self.has_font() {
            self.save();
        }
        if pressed(command(Key::Z)) {
            self.undo();
        }
        if pressed(command(Key::Y)) {
            self.redo();
        }
        if pressed(command(Key::Q)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if pressed(command(Key::E)) && self.has_font() {
            self.effects.open = true;
        }
        if pressed(command(Key::Comma)) {
            self.dialogs.settings = true;
        }
        if pressed(command(Key::Num1)) {
            self.mode = Mode::Overview;
        }
        if pressed(command(Key::Num2)) && self.current.is_some() {
            self.mode = Mode::Editor;
        }
        if pressed(command(Key::Equals)) || pressed(command(Key::Plus)) {
            self.zoom(1.25);
        }
        if pressed(command(Key::Minus)) {
            self.zoom(0.8);
        }
        if pressed(command(Key::Num0)) {
            self.view = None;
        }

        // Plain keys only when no text field has focus.
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        if pressed(command(Key::A)) {
            self.select_all_points();
        }
        let plain = |key| KeyboardShortcut::new(Modifiers::NONE, key);
        let shift = |key| KeyboardShortcut::new(Modifiers::SHIFT, key);
        if pressed(plain(Key::F1)) {
            self.dialogs.shortcuts = true;
        }
        if pressed(plain(Key::V)) {
            self.tool = Tool::Select;
        }
        if pressed(plain(Key::P)) {
            self.tool = Tool::Pen;
            self.mode = Mode::Editor;
        }
        if pressed(plain(Key::OpenBracket)) {
            self.step_glyph(-1);
        }
        if pressed(plain(Key::CloseBracket)) {
            self.step_glyph(1);
        }
        if pressed(plain(Key::Escape)) {
            self.selection.clear();
            self.pen_contour = None;
        }
        if pressed(plain(Key::Tab)) {
            self.mode = match self.mode {
                Mode::Overview if self.current.is_some() => Mode::Editor,
                _ => Mode::Overview,
            };
        }
        if self.mode == Mode::Editor {
            if pressed(plain(Key::Delete)) || pressed(plain(Key::Backspace)) {
                self.delete_selection();
            }
            for (key, dx, dy) in [
                (Key::ArrowLeft, -1.0, 0.0),
                (Key::ArrowRight, 1.0, 0.0),
                (Key::ArrowUp, 0.0, 1.0),
                (Key::ArrowDown, 0.0, -1.0),
            ] {
                if pressed(shift(key)) {
                    self.nudge(dx * 10.0, dy * 10.0);
                } else if pressed(plain(key)) {
                    self.nudge(dx, dy);
                }
            }
        } else if self.mode == Mode::Overview
            && pressed(plain(Key::Enter))
            && let Some(name) = self.current.clone()
        {
            self.open_editor(name);
        }
    }

    // ---- Dialogs ----------------------------------------------------------------------

    fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(mut form) = self.dialogs.new_font.take() {
            let mut keep = true;
            let mut create = false;
            egui::Window::new("New font")
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false)
                .resizable(false)
                .open(&mut keep)
                .show(ctx, |ui| {
                    egui::Grid::new("new_font").num_columns(2).show(ui, |ui| {
                        ui.label("Name");
                        ui.text_edit_singleline(&mut form.name);
                        ui.end_row();
                        ui.label("Units per em");
                        ui.add(egui::DragValue::new(&mut form.upm).range(16..=16384));
                        ui.end_row();
                    });
                    create = ui.button("Create").clicked();
                });
            if create {
                self.create(form.name.clone(), form.upm);
            } else if keep {
                self.dialogs.new_font = Some(form);
            }
        }

        if let Some(mut form) = self.dialogs.new_glyph.take() {
            let mut keep = true;
            let mut add = false;
            egui::Window::new("New glyph")
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false)
                .resizable(false)
                .open(&mut keep)
                .show(ctx, |ui| {
                    egui::Grid::new("new_glyph").num_columns(2).show(ui, |ui| {
                        ui.label("Name");
                        ui.text_edit_singleline(&mut form.name)
                            .on_hover_text("For example A, a, zero, or uni00E9");
                        ui.end_row();
                        ui.label("Character or hex");
                        ui.text_edit_singleline(&mut form.unicode).on_hover_text(
                            "Type the character, or a hex code like 00E9. Leave empty for none.",
                        );
                        ui.end_row();
                        ui.label("Advance");
                        ui.add(egui::DragValue::new(&mut form.advance).speed(1.0));
                        ui.end_row();
                    });
                    add = ui.button("Add glyph").clicked();
                });
            if add {
                self.add_glyph(&form);
            } else if keep {
                self.dialogs.new_glyph = Some(form);
            }
        }

        let mut settings_open = self.dialogs.settings;
        egui::Window::new("Settings")
            .open(&mut settings_open)
            .resizable(false)
            .show(ctx, |ui| {
                if self.settings.ui(ui) {
                    self.thumbs.clear();
                }
            });
        self.dialogs.settings = settings_open;

        let mut shortcuts_open = self.dialogs.shortcuts;
        egui::Window::new("Keyboard shortcuts")
            .open(&mut shortcuts_open)
            .resizable(false)
            .show(ctx, |ui| {
                egui::Grid::new("keys")
                    .num_columns(2)
                    .striped(true)
                    .show(ui, |ui| {
                        for (keys, action) in SHORTCUTS {
                            ui.strong(*keys);
                            ui.label(*action);
                            ui.end_row();
                        }
                    });
            });
        self.dialogs.shortcuts = shortcuts_open;

        let mut about_open = self.dialogs.about;
        egui::Window::new("About Type Foundry")
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut about_open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(format!("Type Foundry {}", env!("CARGO_PKG_VERSION")));
                ui.weak("A local type kit. Every edit is a command on one session, the same one the CLI, plugins, and the MCP server use. Nothing is uploaded.");
            });
        self.dialogs.about = about_open;
    }

    fn add_glyph(&mut self, form: &NewGlyph) {
        let name = form.name.trim().to_string();
        if name.is_empty() {
            self.status = ("A glyph needs a name".into(), Tone::Failed);
            self.dialogs.new_glyph = Some(NewGlyph {
                name: String::new(),
                unicode: form.unicode.clone(),
                advance: form.advance,
            });
            return;
        }
        let unicode = parse_unicode(&form.unicode);
        if self
            .edit_json(
                json!({
                    "op": "put_glyph",
                    "glyph": { "name": name, "unicode": unicode, "advance": form.advance, "contours": [] },
                }),
                Scope::Structure,
            )
            .is_some()
        {
            self.open_editor(name.clone());
            self.tool = Tool::Pen;
            self.status = (
                format!("Added {name}. The pen is ready: click to place points."),
                Tone::Done,
            );
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let title = if self.font_name.is_empty() {
            APP_TITLE.to_string()
        } else {
            let marker = if self.dirty { "• " } else { "" };
            format!("{marker}{} — {APP_TITLE}", self.font_name)
        };
        if title != self.shown_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.shown_title = title;
        }
    }
}

impl eframe::App for FoundryWindow {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.shortcuts(&ctx);
        self.update_title(&ctx);

        let bar = egui::Frame::new()
            .fill(color(PANEL))
            .stroke(Stroke::new(1.0, color(HAIRLINE)))
            .inner_margin(egui::Margin::symmetric(8, 4));
        egui::Panel::top("menu")
            .frame(bar)
            .show(ui, |ui| self.menu_bar(ui));
        egui::Panel::top("toolbar")
            .frame(bar)
            .show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("status")
            .frame(bar)
            .show(ui, |ui| self.status_bar(ui));
        if self.settings.show_preview && self.has_font() {
            egui::Panel::bottom("preview")
                .frame(bar)
                .resizable(false)
                .show(ui, |ui| self.preview_strip(ui));
        }
        if self.settings.show_glyph_list && self.has_font() {
            egui::Panel::left("glyphs")
                .resizable(true)
                .default_size(170.0)
                .size_range(120.0..=320.0)
                .frame(bar)
                .show(ui, |ui| self.glyph_list(ui));
        }
        if self.settings.show_inspector && self.has_font() {
            egui::Panel::right("inspector")
                .resizable(true)
                .default_size(270.0)
                .size_range(240.0..=420.0)
                .frame(bar)
                .show(ui, |ui| self.inspector(ui));
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(color(foundry_app::palette::PAGE)))
            .show(ui, |ui| match self.mode {
                Mode::Overview => self.overview(ui),
                Mode::Editor => self.canvas(ui),
            });

        self.dialogs(&ctx);
        self.effects_window(&ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if let Some(dir) = &self.last_dir {
            eframe::set_value(storage, LAST_DIR_KEY, &dir.to_string_lossy().into_owned());
        }
        eframe::set_value(storage, SETTINGS_KEY, &self.settings);
    }
}

/// A menu entry with a shortcut hint. Returns true when clicked, and closes the menu.
fn item(ui: &mut egui::Ui, label: &str, shortcut: &str, enabled: bool) -> bool {
    let clicked = ui
        .add_enabled(enabled, egui::Button::new(label).shortcut_text(shortcut))
        .clicked();
    if clicked {
        ui.close();
    }
    clicked
}

fn is_writable(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| matches!(ext.to_ascii_lowercase().as_str(), "json" | "ufo" | "ttf"))
}

/// One typed character, or a hex code of two or more digits with or without `U+`. Empty means
/// none.
pub fn parse_unicode(text: &str) -> Option<u32> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let mut chars = text.chars();
    if let (Some(only), None) = (chars.next(), chars.next()) {
        return Some(u32::from(only));
    }
    let hex = text
        .trim_start_matches("U+")
        .trim_start_matches("u+")
        .trim_start_matches("0x");
    u32::from_str_radix(hex, 16)
        .ok()
        .filter(|code| char::from_u32(*code).is_some())
}

const SHORTCUTS: &[(&str, &str)] = &[
    ("Ctrl+N", "New font"),
    ("Ctrl+O", "Open"),
    ("Ctrl+S / Ctrl+Shift+S", "Save / Save As"),
    ("Ctrl+Z / Ctrl+Shift+Z, Ctrl+Y", "Undo / Redo"),
    ("Ctrl+1 / Ctrl+2, Tab", "Overview / Editor"),
    ("Enter (overview)", "Edit the selected glyph"),
    ("[ and ]", "Previous / next glyph"),
    ("V / P", "Select tool / Pen tool"),
    ("Click, Shift+click", "Select a point, add to the selection"),
    ("Drag empty space", "Box select (Shift adds)"),
    ("Alt+click an outline", "Add a point on the segment"),
    ("Double-click a point", "Toggle smooth"),
    ("Arrows, Shift+arrows", "Nudge 1 or 10 units"),
    ("Delete / Backspace", "Delete selected points"),
    ("Ctrl+A / Esc", "Select all points / Deselect"),
    (
        "Pen: click, Shift+click",
        "Add an on-curve / off-curve point",
    ),
    ("Pen: click the first point", "Close the contour"),
    ("Scroll, Ctrl+= / Ctrl+-", "Zoom"),
    ("Right or middle drag", "Pan"),
    ("Ctrl+0, double-click empty", "Fit the glyph"),
    ("Ctrl+E", "Effects"),
    ("Ctrl+,", "Settings"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_characters_and_hex_codes() {
        assert_eq!(parse_unicode("é"), Some(0xE9));
        assert_eq!(parse_unicode("00E9"), Some(0xE9));
        assert_eq!(parse_unicode("U+0041"), Some(0x41));
        assert_eq!(
            parse_unicode("A"),
            Some(0x41),
            "one character is that character"
        );
        assert_eq!(parse_unicode("Z"), Some(u32::from('Z')));
        assert_eq!(parse_unicode(""), None);
        assert_eq!(parse_unicode("D800"), None);
    }
}
