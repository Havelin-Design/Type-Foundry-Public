#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! The Type Foundry window. It reads and changes the font only through `Session::execute`.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, Vec2};
use foundry_api::{Command, Response, Session};
use foundry_app::palette::{
    ALERT, AMBER, BONE, FOCUS, HAIRLINE, HAIRLINE_STRONG, INK, INVERSE, MUTED, PAGE, PANEL, RAISED,
    SIGNAL,
};
use foundry_app::{Bounds, Handle, Outline, Pt, Viewport, flatten, hit_test, scanline_spans};
use serde_json::Value;

const APP_TITLE: &str = "Type Foundry";
const LAST_DIR_KEY: &str = "last_dir";
const HANDLE_RADIUS: f32 = 4.5;
const HIT_RADIUS: f64 = 9.0;
const CURVE_STEPS: usize = 24;

fn color(hex: u32) -> Color32 {
    let [_, r, g, b] = hex.to_be_bytes();
    Color32::from_rgb(r, g, b)
}

fn main() -> eframe::Result {
    #[cfg(windows)]
    shortcut::ensure();

    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_TITLE)
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([640.0, 420.0]),
        ..Default::default()
    };
    eframe::run_native(
        APP_TITLE,
        options,
        Box::new(|cc| {
            apply_chrome(&cc.egui_ctx);
            let last_dir = cc
                .storage
                .and_then(|storage| eframe::get_value::<String>(storage, LAST_DIR_KEY))
                .map(PathBuf::from);
            let mut app = FoundryWindow::new(last_dir);
            if let Some(path) = std::env::args().nth(1) {
                app.open(PathBuf::from(path));
            }
            Ok(Box::new(app))
        }),
    )
}

fn apply_chrome(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.weak_text_color = Some(color(MUTED));
    visuals.hyperlink_color = color(FOCUS);
    visuals.panel_fill = color(PANEL);
    visuals.window_fill = color(PANEL);
    visuals.window_stroke = Stroke::new(1.0, color(HAIRLINE));
    visuals.extreme_bg_color = color(PAGE);
    visuals.faint_bg_color = color(RAISED);
    visuals.code_bg_color = color(RAISED);
    visuals.warn_fg_color = color(AMBER);
    visuals.error_fg_color = color(ALERT);
    visuals.selection.bg_fill = color(FOCUS);
    visuals.selection.stroke = Stroke::new(1.0, color(INVERSE));

    let widgets = &mut visuals.widgets;
    widgets.noninteractive.bg_fill = color(PANEL);
    widgets.noninteractive.weak_bg_fill = color(PANEL);
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, color(HAIRLINE));
    widgets.noninteractive.fg_stroke = Stroke::new(1.0, color(INK));
    for (state, fill, edge) in [
        (&mut widgets.inactive, RAISED, HAIRLINE),
        (&mut widgets.hovered, RAISED, HAIRLINE_STRONG),
        (&mut widgets.active, HAIRLINE, FOCUS),
        (&mut widgets.open, RAISED, HAIRLINE_STRONG),
    ] {
        state.bg_fill = color(fill);
        state.weak_bg_fill = color(fill);
        state.bg_stroke = Stroke::new(1.0, color(edge));
        state.fg_stroke = Stroke::new(1.0, color(INK));
        state.corner_radius = CornerRadius::same(3);
    }
    ctx.set_visuals(visuals);
}

#[derive(Clone, Copy, PartialEq)]
enum Tone {
    Quiet,
    Done,
    Failed,
}

struct FoundryWindow {
    session: Session,
    /// The file the open font came from or was last saved to. Held here, not in the session.
    path: Option<PathBuf>,
    last_dir: Option<PathBuf>,
    font_name: String,
    ascender: f64,
    descender: f64,
    x_height: f64,
    cap_height: f64,
    glyphs: Vec<String>,
    selected: Option<String>,
    outline: Option<Outline>,
    view: Option<Viewport>,
    canvas_size: Vec2,
    dragging: Option<Handle>,
    picked: Option<Handle>,
    status: (String, Tone),
}

impl FoundryWindow {
    fn new(last_dir: Option<PathBuf>) -> Self {
        Self {
            session: Session::new(),
            path: None,
            last_dir,
            font_name: String::new(),
            ascender: 800.0,
            descender: -200.0,
            x_height: 500.0,
            cap_height: 700.0,
            glyphs: Vec::new(),
            selected: None,
            outline: None,
            view: None,
            canvas_size: Vec2::ZERO,
            dragging: None,
            picked: None,
            status: (
                "Open a font file or a .ufo folder.".to_string(),
                Tone::Quiet,
            ),
        }
    }

    fn run(&mut self, command: Command) -> Response {
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

    fn open(&mut self, path: PathBuf) {
        let response = self.run(Command::Open {
            path: path_text(&path),
        });
        if !response.ok {
            return;
        }
        self.remember_dir(&path);
        self.path = Some(path.clone());
        self.read_font(response.data.as_ref());
        self.status = (format!("Opened {}", path.display()), Tone::Done);
    }

    fn save_to(&mut self, path: PathBuf) {
        let response = self.run(Command::Save {
            path: path_text(&path),
        });
        if response.ok {
            self.remember_dir(&path);
            self.status = (format!("Saved {}", path.display()), Tone::Done);
            self.path = Some(path);
        }
    }

    fn remember_dir(&mut self, path: &Path) {
        if let Some(parent) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            self.last_dir = Some(parent.to_path_buf());
        }
    }

    fn read_font(&mut self, summary: Option<&Value>) {
        let Some(summary) = summary else {
            return;
        };
        self.font_name = summary["name"].as_str().unwrap_or_default().to_string();
        let metric =
            |key: &str, fallback: f64| summary["metrics"][key].as_f64().unwrap_or(fallback);
        self.ascender = metric("ascender", 800.0);
        self.descender = metric("descender", -200.0);
        self.x_height = metric("x_height", 500.0);
        self.cap_height = metric("cap_height", 700.0);
        self.glyphs = summary["glyphs"]
            .as_array()
            .map(|names| {
                names
                    .iter()
                    .filter_map(|name| name.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let first = self.glyphs.first().cloned();
        self.select(first);
    }

    fn select(&mut self, name: Option<String>) {
        self.selected = name;
        self.view = None;
        self.dragging = None;
        self.picked = None;
        self.reload_glyph();
    }

    fn reload_glyph(&mut self) {
        self.outline = None;
        let Some(name) = self.selected.clone() else {
            return;
        };
        let response = self.run(Command::Glyph { name });
        self.outline = response.data.as_ref().and_then(Outline::from_json);
    }

    fn move_handle(&mut self, handle: Handle, to: Pt) {
        let Some(name) = self.selected.clone() else {
            return;
        };
        let response = self.run(Command::MovePoint {
            name,
            contour: handle.contour,
            point: handle.point,
            x: to.x.round(),
            y: to.y.round(),
        });
        if response.ok {
            self.reload_glyph();
        }
    }

    fn dialog(&self) -> rfd::FileDialog {
        let dialog = rfd::FileDialog::new();
        match &self.last_dir {
            Some(dir) if dir.is_dir() => dialog.set_directory(dir),
            _ => dialog,
        }
    }

    fn open_file_dialog(&mut self) {
        if let Some(path) = self
            .dialog()
            .add_filter("Fonts", &["json", "ttf", "otf", "ttc", "otc"])
            .add_filter("Type Foundry font", &["json"])
            .add_filter("TrueType or OpenType", &["ttf", "otf", "ttc", "otc"])
            .pick_file()
        {
            self.open(path);
        }
    }

    fn open_ufo_dialog(&mut self) {
        if let Some(path) = self.dialog().pick_folder() {
            if foundry_is_ufo(&path) {
                self.open(path);
            } else {
                self.status = (
                    format!("{} is not a .ufo folder", path.display()),
                    Tone::Failed,
                );
            }
        }
    }

    fn save_as_dialog(&mut self) {
        let stem = self
            .path
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
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

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Open…").clicked() {
                self.open_file_dialog();
            }
            if ui.button("Open UFO…").clicked() {
                self.open_ufo_dialog();
            }
            let has_font = self.session.font().is_some();
            if ui
                .add_enabled(has_font, egui::Button::new("Save"))
                .clicked()
            {
                match self.path.clone() {
                    Some(path) => self.save_to(path),
                    None => self.save_as_dialog(),
                }
            }
            if ui
                .add_enabled(has_font, egui::Button::new("Save As…"))
                .clicked()
            {
                self.save_as_dialog();
            }
            ui.separator();
            if !self.font_name.is_empty() {
                ui.strong(&self.font_name);
            }
            if let Some(path) = &self.path {
                ui.weak(path.display().to_string());
            }
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
            if let (Some(outline), Some(handle)) = (&self.outline, self.picked)
                && let Some(point) = outline.point(handle)
            {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.weak(format!(
                        "contour {} · point {} · {} {}",
                        handle.contour, handle.point, point.at.x, point.at.y
                    ));
                });
            }
        });
    }

    fn glyph_list(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.weak(format!("Glyphs · {}", self.glyphs.len()));
        ui.add_space(4.0);
        let mut chosen = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for name in &self.glyphs {
                let selected = self.selected.as_deref() == Some(name.as_str());
                if ui.selectable_label(selected, name).clicked() && !selected {
                    chosen = Some(name.clone());
                }
            }
        });
        if let Some(name) = chosen {
            self.select(Some(name));
        }
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = response.rect;
        painter.rect_filled(rect, CornerRadius::ZERO, color(BONE));

        let Some(outline) = self.outline.clone() else {
            let hint = if self.session.font().is_some() {
                "This font has no glyphs."
            } else {
                "Open a font file or a .ufo folder."
            };
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                hint,
                egui::FontId::proportional(15.0),
                color(MUTED),
            );
            return;
        };

        if self.view.is_none() || self.canvas_size != rect.size() {
            self.canvas_size = rect.size();
            self.view = Some(Viewport::fit(
                bounds_of(rect),
                outline.bounds(self.descender, self.ascender),
                48.0,
            ));
        }
        let Some(mut view) = self.view else {
            return;
        };

        if response.hovered() {
            let (scroll, zoom, pointer) = ui.input(|input| {
                (
                    input.smooth_scroll_delta.y,
                    input.zoom_delta(),
                    input.pointer.hover_pos(),
                )
            });
            let factor = f64::from(zoom) * f64::from(scroll * 0.0025).exp();
            if let Some(pointer) = pointer
                && (factor - 1.0).abs() > f64::EPSILON
            {
                view.zoom_at(pt(pointer), factor);
            }
        }
        if response.dragged_by(egui::PointerButton::Secondary)
            || response.dragged_by(egui::PointerButton::Middle)
        {
            let delta = response.drag_delta();
            view.pan(f64::from(delta.x), f64::from(delta.y));
        }
        if response.double_clicked() {
            view = Viewport::fit(
                bounds_of(rect),
                outline.bounds(self.descender, self.ascender),
                48.0,
            );
        }

        // A drag starts once the pointer passes egui's threshold, so hit-test where it was pressed.
        if response.drag_started_by(egui::PointerButton::Primary)
            && let Some(press) = ui.input(|input| input.pointer.press_origin())
        {
            self.dragging = hit_test(&outline, &view, pt(press), HIT_RADIUS);
            if self.dragging.is_some() {
                self.picked = self.dragging;
            }
        }
        if response.clicked()
            && let Some(press) = response.interact_pointer_pos()
        {
            self.picked = hit_test(&outline, &view, pt(press), HIT_RADIUS);
        }
        if let Some(handle) = self.dragging {
            if response.dragged_by(egui::PointerButton::Primary) {
                if let Some(pointer) = response.interact_pointer_pos() {
                    let target = view.to_font(pt(pointer));
                    let current = outline.point(handle).map(|point| point.at);
                    if current != Some(Pt::new(target.x.round(), target.y.round())) {
                        self.move_handle(handle, target);
                    }
                }
            } else {
                self.dragging = None;
            }
        }
        self.view = Some(view);

        let outline = self.outline.clone().unwrap_or(outline);
        let painter = painter.with_clip_rect(rect);
        self.paint_metrics(&painter, &view, rect, outline.advance);
        paint_fill(&painter, &view, rect, &outline);
        paint_handles(&painter, &view, &outline, self.picked);

        if response.hovered() && hit_hover(ui, &outline, &view) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
    }

    fn paint_metrics(&self, painter: &egui::Painter, view: &Viewport, rect: Rect, advance: f64) {
        let line = Stroke::new(1.0, color(MUTED));
        let faint = Stroke::new(1.0, color(MUTED).gamma_multiply(0.45));
        for (value, stroke) in [
            (0.0, line),
            (self.ascender, faint),
            (self.descender, faint),
            (self.x_height, faint),
            (self.cap_height, faint),
        ] {
            let y = view.to_screen(Pt::new(0.0, value)).y as f32;
            painter.hline(rect.x_range(), y, stroke);
        }
        for x in [0.0, advance] {
            let x = view.to_screen(Pt::new(x, 0.0)).x as f32;
            painter.vline(x, rect.y_range(), faint);
        }
    }
}

impl eframe::App for FoundryWindow {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let bar = egui::Frame::new()
            .fill(color(PANEL))
            .stroke(Stroke::new(1.0, color(HAIRLINE)))
            .inner_margin(egui::Margin::symmetric(10, 6));
        egui::Panel::top("toolbar")
            .frame(bar)
            .show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("status")
            .frame(bar)
            .show(ui, |ui| self.status_bar(ui));
        egui::Panel::left("glyphs")
            .resizable(true)
            .default_size(160.0)
            .frame(
                egui::Frame::new()
                    .fill(color(PANEL))
                    .stroke(Stroke::new(1.0, color(HAIRLINE)))
                    .inner_margin(egui::Margin::symmetric(10, 6)),
            )
            .show(ui, |ui| self.glyph_list(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(color(PAGE)))
            .show(ui, |ui| self.canvas(ui));
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if let Some(dir) = &self.last_dir {
            eframe::set_value(storage, LAST_DIR_KEY, &dir.to_string_lossy().into_owned());
        }
    }
}

fn paint_fill(painter: &egui::Painter, view: &Viewport, rect: Rect, outline: &Outline) {
    let polygons: Vec<Vec<Pt>> = outline
        .contours
        .iter()
        .filter(|contour| contour.closed)
        .map(|contour| {
            flatten(contour, CURVE_STEPS)
                .into_iter()
                .map(|point| view.to_screen(point))
                .collect()
        })
        .collect();
    let ink = Color32::BLACK;
    let mut y = rect.top().floor();
    while y < rect.bottom() {
        for (left, right) in scanline_spans(&polygons, f64::from(y) + 0.5) {
            let span =
                Rect::from_min_max(Pos2::new(left as f32, y), Pos2::new(right as f32, y + 1.0));
            painter.rect_filled(span, CornerRadius::ZERO, ink);
        }
        y += 1.0;
    }
    for contour in outline.contours.iter().filter(|contour| !contour.closed) {
        let line: Vec<Pos2> = flatten(contour, CURVE_STEPS)
            .into_iter()
            .map(|point| pos(view.to_screen(point)))
            .collect();
        painter.add(egui::Shape::line(line, Stroke::new(1.5, ink)));
    }
}

fn paint_handles(
    painter: &egui::Painter,
    view: &Viewport,
    outline: &Outline,
    picked: Option<Handle>,
) {
    let tether = Stroke::new(1.0, color(AMBER));
    for contour in &outline.contours {
        let count = contour.points.len();
        for (index, point) in contour.points.iter().enumerate() {
            if point.on {
                continue;
            }
            let neighbours = [(index + count - 1) % count, (index + 1) % count];
            for other in neighbours {
                let wraps =
                    (other == count - 1 && index == 0) || (other == 0 && index == count - 1);
                if !contour.closed && wraps {
                    continue;
                }
                let neighbour = &contour.points[other];
                if neighbour.on {
                    painter.line_segment(
                        [
                            pos(view.to_screen(point.at)),
                            pos(view.to_screen(neighbour.at)),
                        ],
                        tether,
                    );
                }
            }
        }
    }
    let rim = Stroke::new(1.0, Color32::BLACK);
    for (contour_index, contour) in outline.contours.iter().enumerate() {
        for (point_index, point) in contour.points.iter().enumerate() {
            let at = pos(view.to_screen(point.at));
            let is_picked = picked
                == Some(Handle {
                    contour: contour_index,
                    point: point_index,
                });
            let fill = if is_picked {
                color(SIGNAL)
            } else if point.on {
                color(FOCUS)
            } else {
                color(AMBER)
            };
            if point.on {
                painter.circle(at, HANDLE_RADIUS, fill, rim);
            } else {
                let half = Vec2::splat(HANDLE_RADIUS * 0.85);
                painter.rect(
                    Rect::from_center_size(at, half * 2.0),
                    CornerRadius::ZERO,
                    fill,
                    rim,
                    egui::StrokeKind::Middle,
                );
            }
        }
    }
}

fn hit_hover(ui: &egui::Ui, outline: &Outline, view: &Viewport) -> bool {
    ui.input(|input| input.pointer.hover_pos())
        .is_some_and(|pointer| hit_test(outline, view, pt(pointer), HIT_RADIUS).is_some())
}

fn foundry_is_ufo(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("ufo"))
}

fn bounds_of(rect: Rect) -> Bounds {
    Bounds {
        min: pt(rect.min),
        max: pt(rect.max),
    }
}

fn pt(pos: Pos2) -> Pt {
    Pt::new(f64::from(pos.x), f64::from(pos.y))
}

fn pos(point: Pt) -> Pos2 {
    Pos2::new(point.x as f32, point.y as f32)
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// The Start menu shortcut, created on the first launch of the window.
#[cfg(windows)]
mod shortcut {
    use std::os::windows::process::CommandExt;
    use std::path::PathBuf;
    use std::process::Command;

    const TARGET: &str =
        r"C:\Users\Troy Havelin\AppData\Local\typefoundry-target\release\typefoundry.exe";
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    pub fn ensure() {
        let Some(appdata) = std::env::var_os("APPDATA") else {
            return;
        };
        let link = PathBuf::from(appdata)
            .join(r"Microsoft\Windows\Start Menu\Programs")
            .join("Type Foundry.lnk");
        if link.exists() {
            return;
        }
        let quote = |text: &str| format!("'{}'", text.replace('\'', "''"));
        let script = format!(
            "$s = (New-Object -ComObject WScript.Shell).CreateShortcut({link}); \
             $s.TargetPath = {target}; \
             $s.WorkingDirectory = {dir}; \
             $s.Description = 'Type Foundry'; \
             $s.Save()",
            link = quote(&link.to_string_lossy()),
            target = quote(TARGET),
            dir = quote(r"C:\Users\Troy Havelin\AppData\Local\typefoundry-target\release"),
        );
        let result = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
        match result {
            Ok(status) if status.success() => {}
            Ok(status) => eprintln!("Start menu shortcut was not created: {status}"),
            Err(err) => eprintln!("Start menu shortcut was not created: {err}"),
        }
    }
}
