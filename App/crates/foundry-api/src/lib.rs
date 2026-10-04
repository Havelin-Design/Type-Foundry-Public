//! One command session. The CLI, plugins, and agents all go through [`Session::execute`].

use std::path::Path;

use foundry_core::{
    Anchor, Contour, Font, FoundryError, Glyph, Matrix, MetricsUpdate, PointKind, blend_fonts,
    compatibility,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Command {
    Create {
        name: String,
        #[serde(default = "default_upm")]
        upm: u16,
    },
    Open {
        path: String,
    },
    Save {
        path: String,
    },
    Info,
    Glyphs,
    Glyph {
        name: String,
    },
    PutGlyph {
        glyph: Glyph,
    },
    SetAdvance {
        name: String,
        advance: f64,
    },
    MovePoint {
        name: String,
        contour: usize,
        point: usize,
        x: f64,
        y: f64,
    },
    MovePoints {
        name: String,
        points: Vec<[usize; 2]>,
        dx: f64,
        dy: f64,
    },
    InsertPoint {
        name: String,
        contour: usize,
        index: usize,
        x: f64,
        y: f64,
        #[serde(default = "default_kind")]
        kind: PointKind,
        #[serde(default)]
        smooth: bool,
    },
    SplitSegment {
        name: String,
        contour: usize,
        point: usize,
        t: f64,
    },
    DeletePoints {
        name: String,
        points: Vec<[usize; 2]>,
    },
    SetPoint {
        name: String,
        contour: usize,
        point: usize,
        #[serde(default)]
        kind: Option<PointKind>,
        #[serde(default)]
        smooth: Option<bool>,
    },
    AddContour {
        name: String,
        contour: Contour,
    },
    SetClosed {
        name: String,
        contour: usize,
        closed: bool,
    },
    ReverseContour {
        name: String,
        contour: usize,
    },
    DeleteGlyph {
        name: String,
    },
    RenameGlyph {
        name: String,
        new_name: String,
    },
    SetUnicode {
        name: String,
        #[serde(default)]
        unicode: Option<u32>,
    },
    RenameFont {
        name: String,
    },
    SetMetrics {
        #[serde(default)]
        ascender: Option<f64>,
        #[serde(default)]
        descender: Option<f64>,
        #[serde(default)]
        cap_height: Option<f64>,
        #[serde(default)]
        x_height: Option<f64>,
    },
    Transform {
        #[serde(default)]
        names: Option<Vec<String>>,
        #[serde(default)]
        points: Option<Vec<[usize; 2]>>,
        matrix: Matrix,
        #[serde(default)]
        advance: bool,
        #[serde(default)]
        anchor: Anchor,
    },
    RoundCoordinates {
        #[serde(default)]
        names: Option<Vec<String>>,
    },
    Index,
    Undo,
    Redo,
    Checkpoint,
    History,
    Check {
        a: String,
        b: String,
    },
    Blend {
        a: String,
        b: String,
        #[serde(default = "default_t")]
        t: f64,
        out: String,
    },
}

fn default_upm() -> u16 {
    1000
}

fn default_t() -> f64 {
    0.5
}

fn default_kind() -> PointKind {
    PointKind::On
}

/// Undo steps kept per session. Older steps are dropped.
pub const UNDO_LIMIT: usize = 200;

impl Command {
    /// True for commands that change the open font in place and can be undone.
    fn is_edit(&self) -> bool {
        matches!(
            self,
            Self::PutGlyph { .. }
                | Self::SetAdvance { .. }
                | Self::MovePoint { .. }
                | Self::MovePoints { .. }
                | Self::InsertPoint { .. }
                | Self::SplitSegment { .. }
                | Self::DeletePoints { .. }
                | Self::SetPoint { .. }
                | Self::AddContour { .. }
                | Self::SetClosed { .. }
                | Self::ReverseContour { .. }
                | Self::DeleteGlyph { .. }
                | Self::RenameGlyph { .. }
                | Self::SetUnicode { .. }
                | Self::RenameFont { .. }
                | Self::SetMetrics { .. }
                | Self::Transform { .. }
                | Self::RoundCoordinates { .. }
        )
    }

    /// Consecutive edits with the same key share one undo step, so a drag or a slider is one
    /// step. `checkpoint` ends the run.
    fn coalesce_key(&self) -> Option<String> {
        match self {
            Self::MovePoint {
                name,
                contour,
                point,
                ..
            } => Some(format!("move:{name}:{:?}", [[*contour, *point]])),
            Self::MovePoints { name, points, .. } => {
                let mut sorted = points.clone();
                sorted.sort_unstable();
                sorted.dedup();
                Some(format!("move:{name}:{sorted:?}"))
            }
            Self::SetAdvance { name, .. } => Some(format!("advance:{name}")),
            Self::SetMetrics { .. } => Some("metrics".to_string()),
            _ => None,
        }
    }
}

fn refs(points: &[[usize; 2]]) -> Vec<(usize, usize)> {
    points.iter().map(|[c, p]| (*c, *p)).collect()
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Response {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl Response {
    fn done(data: Option<Value>) -> Self {
        Self {
            ok: true,
            error: None,
            data,
        }
    }

    fn fail(message: impl Into<String>, data: Option<Value>) -> Self {
        Self {
            ok: false,
            error: Some(message.into()),
            data,
        }
    }
}

#[derive(Debug)]
struct Fail {
    message: String,
    data: Option<Value>,
}

impl From<FoundryError> for Fail {
    fn from(err: FoundryError) -> Self {
        Self {
            message: err.to_string(),
            data: None,
        }
    }
}

#[derive(Debug, Default)]
pub struct Session {
    font: Option<Font>,
    undo: Vec<Font>,
    redo: Vec<Font>,
    last_edit: Option<String>,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn font(&self) -> Option<&Font> {
        self.font.as_ref()
    }

    /// Undo and redo steps available.
    pub fn history(&self) -> (usize, usize) {
        (self.undo.len(), self.redo.len())
    }

    pub fn execute(&mut self, command: Command) -> Response {
        let edit = command.is_edit();
        let key = command.coalesce_key();
        let replaces = matches!(
            command,
            Command::Create { .. } | Command::Open { .. } | Command::Blend { .. }
        );
        let before = if edit && (key.is_none() || key != self.last_edit) {
            self.font.clone()
        } else {
            None
        };
        let result = self.dispatch(command);
        if result.is_ok() {
            if edit {
                if let Some(snapshot) = before {
                    self.undo.push(snapshot);
                    if self.undo.len() > UNDO_LIMIT {
                        self.undo.remove(0);
                    }
                }
                self.redo.clear();
                self.last_edit = key;
            } else if replaces {
                self.undo.clear();
                self.redo.clear();
                self.last_edit = None;
            }
        }
        match result {
            Ok(data) => Response::done(data),
            Err(err) => Response::fail(err.message, err.data),
        }
    }

    /// Parse one command line. Blank lines and `#` comments return `None`.
    pub fn execute_line(&mut self, line: &str) -> Option<Response> {
        let trimmed = line.trim().trim_start_matches('\u{feff}');
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return None;
        }
        let command = match serde_json::from_str::<Command>(trimmed) {
            Ok(command) => command,
            Err(err) => {
                return Some(Response::fail(
                    format!("could not read command: {err}"),
                    None,
                ));
            }
        };
        Some(self.execute(command))
    }

    fn dispatch(&mut self, command: Command) -> Result<Option<Value>, Fail> {
        match command {
            Command::Create { name, upm } => {
                let font = Font::new(name, upm)?;
                let data = summary(&font);
                self.font = Some(font);
                Ok(Some(data))
            }
            Command::Open { path } => {
                let font = Font::load(Path::new(&path))?;
                let data = summary(&font);
                self.font = Some(font);
                Ok(Some(data))
            }
            Command::Save { path } => {
                let font = self.font.as_ref().ok_or(FoundryError::NoFont)?;
                font.save(Path::new(&path))?;
                Ok(Some(json!({ "path": path })))
            }
            Command::Info => Ok(Some(summary(
                self.font.as_ref().ok_or(FoundryError::NoFont)?,
            ))),
            Command::Glyphs => {
                let font = self.font.as_ref().ok_or(FoundryError::NoFont)?;
                let names: Vec<&str> = font.glyph_names();
                Ok(Some(json!({ "glyphs": names })))
            }
            Command::Glyph { name } => {
                let font = self.font.as_ref().ok_or(FoundryError::NoFont)?;
                let glyph = font.glyph(&name).ok_or(FoundryError::MissingGlyph(name))?;
                Ok(Some(
                    serde_json::to_value(glyph)
                        .map_err(|err| FoundryError::Json(err.to_string()))?,
                ))
            }
            Command::PutGlyph { glyph } => {
                let name = glyph.name.clone();
                let font = self.font.as_mut().ok_or(FoundryError::NoFont)?;
                font.insert_glyph(glyph)?;
                Ok(Some(json!({ "name": name })))
            }
            Command::SetAdvance { name, advance } => {
                if !advance.is_finite() {
                    return Err(FoundryError::NonFinite.into());
                }
                let font = self.font.as_mut().ok_or(FoundryError::NoFont)?;
                let glyph = font
                    .glyph_mut(&name)
                    .ok_or_else(|| FoundryError::MissingGlyph(name.clone()))?;
                glyph.advance = advance;
                Ok(Some(json!({ "name": name, "advance": advance })))
            }
            Command::MovePoint {
                name,
                contour,
                point,
                x,
                y,
            } => {
                let font = self.font.as_mut().ok_or(FoundryError::NoFont)?;
                font.move_point(&name, contour, point, x, y)?;
                Ok(Some(json!({
                    "name": name,
                    "contour": contour,
                    "point": point,
                    "x": x,
                    "y": y,
                })))
            }
            Command::MovePoints {
                name,
                points,
                dx,
                dy,
            } => {
                let font = self.font.as_mut().ok_or(FoundryError::NoFont)?;
                font.move_points(&name, &refs(&points), dx, dy)?;
                Ok(Some(
                    json!({ "name": name, "points": points, "dx": dx, "dy": dy }),
                ))
            }
            Command::InsertPoint {
                name,
                contour,
                index,
                x,
                y,
                kind,
                smooth,
            } => {
                let font = self.font.as_mut().ok_or(FoundryError::NoFont)?;
                let point = foundry_core::Point { x, y, kind, smooth };
                font.insert_point(&name, contour, index, point)?;
                Ok(Some(
                    json!({ "name": name, "contour": contour, "point": index }),
                ))
            }
            Command::SplitSegment {
                name,
                contour,
                point,
                t,
            } => {
                let font = self.font.as_mut().ok_or(FoundryError::NoFont)?;
                let index = font.split_segment(&name, contour, point, t)?;
                Ok(Some(
                    json!({ "name": name, "contour": contour, "point": index }),
                ))
            }
            Command::DeletePoints { name, points } => {
                let font = self.font.as_mut().ok_or(FoundryError::NoFont)?;
                font.delete_points(&name, &refs(&points))?;
                Ok(Some(json!({ "name": name, "deleted": points.len() })))
            }
            Command::SetPoint {
                name,
                contour,
                point,
                kind,
                smooth,
            } => {
                let font = self.font.as_mut().ok_or(FoundryError::NoFont)?;
                font.set_point_type(&name, contour, point, kind, smooth)?;
                let glyph = font
                    .glyph(&name)
                    .ok_or_else(|| FoundryError::MissingGlyph(name.clone()))?;
                let set = &glyph.contours[contour].points[point];
                Ok(Some(json!({
                    "name": name, "contour": contour, "point": point,
                    "kind": set.kind, "smooth": set.smooth,
                })))
            }
            Command::AddContour { name, contour } => {
                let font = self.font.as_mut().ok_or(FoundryError::NoFont)?;
                let index = font.add_contour(&name, contour)?;
                Ok(Some(json!({ "name": name, "contour": index })))
            }
            Command::SetClosed {
                name,
                contour,
                closed,
            } => {
                let font = self.font.as_mut().ok_or(FoundryError::NoFont)?;
                font.set_closed(&name, contour, closed)?;
                Ok(Some(
                    json!({ "name": name, "contour": contour, "closed": closed }),
                ))
            }
            Command::ReverseContour { name, contour } => {
                let font = self.font.as_mut().ok_or(FoundryError::NoFont)?;
                font.reverse_contour(&name, contour)?;
                Ok(Some(json!({ "name": name, "contour": contour })))
            }
            Command::DeleteGlyph { name } => {
                let font = self.font.as_mut().ok_or(FoundryError::NoFont)?;
                font.delete_glyph(&name)?;
                Ok(Some(json!({ "name": name })))
            }
            Command::RenameGlyph { name, new_name } => {
                let font = self.font.as_mut().ok_or(FoundryError::NoFont)?;
                font.rename_glyph(&name, &new_name)?;
                Ok(Some(json!({ "name": new_name, "was": name })))
            }
            Command::SetUnicode { name, unicode } => {
                let font = self.font.as_mut().ok_or(FoundryError::NoFont)?;
                font.set_unicode(&name, unicode)?;
                Ok(Some(json!({ "name": name, "unicode": unicode })))
            }
            Command::RenameFont { name } => {
                let font = self.font.as_mut().ok_or(FoundryError::NoFont)?;
                font.rename(&name)?;
                Ok(Some(summary(font)))
            }
            Command::SetMetrics {
                ascender,
                descender,
                cap_height,
                x_height,
            } => {
                let font = self.font.as_mut().ok_or(FoundryError::NoFont)?;
                font.set_metrics(MetricsUpdate {
                    ascender,
                    descender,
                    cap_height,
                    x_height,
                })?;
                Ok(Some(json!({ "metrics": font.metrics })))
            }
            Command::Transform {
                names,
                points,
                matrix,
                advance,
                anchor,
            } => {
                let font = self.font.as_mut().ok_or(FoundryError::NoFont)?;
                let selection = points.as_deref().map(refs);
                let changed = font.transform_anchored(
                    names.as_deref(),
                    selection.as_deref(),
                    matrix,
                    advance,
                    anchor,
                )?;
                Ok(Some(json!({ "glyphs": changed })))
            }
            Command::RoundCoordinates { names } => {
                let font = self.font.as_mut().ok_or(FoundryError::NoFont)?;
                let changed = font.round_coordinates(names.as_deref())?;
                Ok(Some(json!({ "glyphs": changed })))
            }
            Command::Index => {
                let font = self.font.as_ref().ok_or(FoundryError::NoFont)?;
                let glyphs: Vec<Value> = font
                    .glyphs
                    .iter()
                    .map(|glyph| {
                        json!({
                            "name": glyph.name,
                            "unicode": glyph.unicode,
                            "advance": glyph.advance,
                            "contours": glyph.contours.len(),
                            "points": glyph.contours.iter().map(|c| c.points.len()).sum::<usize>(),
                        })
                    })
                    .collect();
                Ok(Some(json!({ "glyphs": glyphs })))
            }
            Command::Undo => {
                let previous = self
                    .undo
                    .pop()
                    .ok_or_else(|| FoundryError::Edit("nothing to undo".into()))?;
                if let Some(current) = self.font.replace(previous) {
                    self.redo.push(current);
                }
                self.last_edit = None;
                self.history_data()
            }
            Command::Redo => {
                let next = self
                    .redo
                    .pop()
                    .ok_or_else(|| FoundryError::Edit("nothing to redo".into()))?;
                if let Some(current) = self.font.replace(next) {
                    self.undo.push(current);
                }
                self.last_edit = None;
                self.history_data()
            }
            Command::Checkpoint => {
                self.last_edit = None;
                self.history_data()
            }
            Command::History => self.history_data(),
            Command::Check { a, b } => compare_files(&a, &b),
            Command::Blend { a, b, t, out } => {
                let left = Font::load(Path::new(&a))?;
                let right = Font::load(Path::new(&b))?;
                match blend_fonts(&left, &right, t) {
                    Ok(font) => {
                        font.save(Path::new(&out))?;
                        let data = json!({
                            "path": out,
                            "name": font.name,
                            "glyphs": font.glyph_names(),
                        });
                        self.font = Some(font);
                        Ok(Some(data))
                    }
                    Err(issues) => {
                        let message = issues
                            .iter()
                            .map(|issue| issue.detail.clone())
                            .collect::<Vec<_>>()
                            .join("; ");
                        Err(Fail {
                            message,
                            data: Some(json!({ "compatible": false, "issues": issues })),
                        })
                    }
                }
            }
        }
    }
}

impl Session {
    fn history_data(&self) -> Result<Option<Value>, Fail> {
        Ok(Some(
            json!({ "undo": self.undo.len(), "redo": self.redo.len() }),
        ))
    }
}

fn summary(font: &Font) -> Value {
    json!({
        "name": font.name,
        "upm": font.upm,
        "metrics": font.metrics,
        "glyphs": font.glyph_names(),
    })
}

fn compare_files(a: &str, b: &str) -> Result<Option<Value>, Fail> {
    let left = Font::load(Path::new(a))?;
    let right = Font::load(Path::new(b))?;
    let issues = compatibility(&left, &right);
    let compatible = issues.is_empty();
    let message = issues
        .iter()
        .map(|issue| issue.detail.clone())
        .collect::<Vec<_>>()
        .join("; ");
    let data = json!({
        "compatible": compatible,
        "issues": issues,
    });
    if compatible {
        Ok(Some(data))
    } else {
        Err(Fail {
            message,
            data: Some(data),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use foundry_core::{Contour, Point, PointKind};
    use serde_json::json;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_IDS: AtomicU64 = AtomicU64::new(0);

    fn temp_dir() -> std::path::PathBuf {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let id = TEMP_IDS.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("typefoundry-{tick}-{id}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn square(name: &str, x: f64, advance: f64) -> Glyph {
        Glyph {
            name: name.to_string(),
            unicode: Some(72),
            advance,
            contours: vec![Contour {
                closed: true,
                points: vec![
                    point(x, 0.0),
                    point(x + 80.0, 0.0),
                    point(x + 80.0, 120.0),
                    point(x, 120.0),
                ],
            }],
        }
    }

    fn point(x: f64, y: f64) -> Point {
        Point {
            x,
            y,
            kind: PointKind::On,
            smooth: false,
        }
    }

    #[test]
    fn create_put_save_open_and_blend() {
        let dir = temp_dir();
        let narrow_path = dir.join("narrow.json");
        let wide_path = dir.join("wide.json");
        let mid_path = dir.join("mid.json");

        let mut session = Session::new();
        assert!(
            session
                .execute(Command::Create {
                    name: "Narrow".into(),
                    upm: 1000
                })
                .ok
        );
        assert!(
            session
                .execute(Command::PutGlyph {
                    glyph: square("H", 40.0, 400.0),
                })
                .ok
        );
        assert!(
            session
                .execute(Command::Save {
                    path: narrow_path.to_string_lossy().into_owned(),
                })
                .ok
        );

        assert!(
            session
                .execute(Command::Create {
                    name: "Wide".into(),
                    upm: 1000
                })
                .ok
        );
        assert!(
            session
                .execute(Command::PutGlyph {
                    glyph: square("H", 140.0, 800.0),
                })
                .ok
        );
        assert!(
            session
                .execute(Command::Save {
                    path: wide_path.to_string_lossy().into_owned(),
                })
                .ok
        );

        let blended = session.execute(Command::Blend {
            a: narrow_path.to_string_lossy().into_owned(),
            b: wide_path.to_string_lossy().into_owned(),
            t: 0.5,
            out: mid_path.to_string_lossy().into_owned(),
        });
        assert!(blended.ok, "{blended:?}");
        let opened = Font::load(&mid_path).unwrap();
        assert_eq!(opened.glyph("H").unwrap().advance, 600.0);
        assert_eq!(opened.glyph("H").unwrap().contours[0].points[0].x, 90.0);
        assert_eq!(session.font().unwrap().name, opened.name);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn moves_a_point_in_an_open_ufo_and_saves_json() {
        let dir = temp_dir();
        let ufo_path = dir.join("wide.ufo");
        let json_path = dir.join("wide.json");
        let mut session = Session::new();
        assert!(
            session
                .execute(Command::Create {
                    name: "Wide".into(),
                    upm: 1000,
                })
                .ok
        );
        assert!(
            session
                .execute(Command::PutGlyph {
                    glyph: square("H", 40.0, 400.0),
                })
                .ok
        );
        let saved = session.execute(Command::Save {
            path: ufo_path.to_string_lossy().into_owned(),
        });
        assert!(saved.ok, "{saved:?}");
        let moved = session.execute(Command::MovePoint {
            name: "H".into(),
            contour: 0,
            point: 0,
            x: 55.0,
            y: 5.0,
        });
        assert!(moved.ok, "{moved:?}");
        let saved_json = session.execute(Command::Save {
            path: json_path.to_string_lossy().into_owned(),
        });
        assert!(saved_json.ok, "{saved_json:?}");
        let opened = Font::load(&json_path).unwrap();
        assert_eq!(opened.glyph("H").unwrap().contours[0].points[0].x, 55.0);
        assert_eq!(opened.glyph("H").unwrap().contours[0].points[0].y, 5.0);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn failed_open_keeps_the_current_font() {
        let mut session = Session::new();
        session.execute(Command::Create {
            name: "Kept".into(),
            upm: 1000,
        });
        let response = session.execute(Command::Open {
            path: "does-not-exist.json".into(),
        });
        assert!(!response.ok);
        assert_eq!(session.font().unwrap().name, "Kept");
    }

    #[test]
    fn command_line_skips_comments_and_reports_bad_json() {
        let mut session = Session::new();
        assert!(session.execute_line("").is_none());
        assert!(session.execute_line("# note").is_none());
        let bad = session.execute_line("{").unwrap();
        assert!(!bad.ok);
        let created = session
            .execute_line("\u{feff}{\"op\":\"create\",\"name\":\"From Line\"}")
            .unwrap();
        assert!(created.ok);
        assert_eq!(
            created.data.unwrap()["upm"],
            json!(1000),
            "omitted upm defaults to 1000"
        );
    }

    fn line(session: &mut Session, text: &str) -> Response {
        session.execute_line(text).unwrap()
    }

    fn first_x(session: &Session) -> f64 {
        session.font().unwrap().glyph("H").unwrap().contours[0].points[0].x
    }

    #[test]
    fn undo_and_redo_with_drags_as_one_step() {
        let mut session = Session::new();
        session.execute(Command::Create {
            name: "Undo".into(),
            upm: 1000,
        });
        session.execute(Command::PutGlyph {
            glyph: square("H", 0.0, 400.0),
        });
        assert_eq!(session.history(), (1, 0));

        // A drag: three moves of the same point are one undo step.
        for x in [10.0, 20.0, 30.0] {
            assert!(
                session
                    .execute(Command::MovePoint {
                        name: "H".into(),
                        contour: 0,
                        point: 0,
                        x,
                        y: 0.0,
                    })
                    .ok
            );
        }
        assert_eq!(session.history(), (2, 0));
        // A checkpoint starts a new step even for the same point.
        assert!(line(&mut session, r#"{"op":"checkpoint"}"#).ok);
        line(
            &mut session,
            r#"{"op":"move_points","name":"H","points":[[0,0]],"dx":5,"dy":0}"#,
        );
        assert_eq!(first_x(&session), 35.0);
        assert_eq!(session.history(), (3, 0));

        assert!(line(&mut session, r#"{"op":"undo"}"#).ok);
        assert_eq!(first_x(&session), 30.0);
        assert!(line(&mut session, r#"{"op":"undo"}"#).ok);
        assert_eq!(first_x(&session), 0.0);
        let history = line(&mut session, r#"{"op":"redo"}"#);
        assert_eq!(history.data.unwrap(), json!({ "undo": 2, "redo": 1 }));
        assert_eq!(first_x(&session), 30.0);

        // A new edit clears redo. A failed edit adds no step.
        line(
            &mut session,
            r#"{"op":"set_unicode","name":"H","unicode":104}"#,
        );
        assert_eq!(session.history(), (3, 0));
        assert!(!line(&mut session, r#"{"op":"delete_glyph","name":"Q"}"#).ok);
        assert_eq!(session.history(), (3, 0));
        assert!(!line(&mut session, r#"{"op":"redo"}"#).ok);

        // Opening or creating a font starts a fresh history.
        session.execute(Command::Create {
            name: "Fresh".into(),
            upm: 1000,
        });
        assert_eq!(session.history(), (0, 0));
        assert!(!line(&mut session, r#"{"op":"undo"}"#).ok);
    }

    #[test]
    fn edit_commands_read_from_json() {
        let mut session = Session::new();
        session.execute(Command::Create {
            name: "Lines".into(),
            upm: 1000,
        });
        session.execute(Command::PutGlyph {
            glyph: square("H", 0.0, 400.0),
        });
        for text in [
            r#"{"op":"insert_point","name":"H","contour":0,"index":1,"x":40,"y":0}"#,
            r#"{"op":"set_point","name":"H","contour":0,"point":1,"kind":"off"}"#,
            r#"{"op":"split_segment","name":"H","contour":0,"point":3,"t":0.5}"#,
            r#"{"op":"add_contour","name":"H","contour":{"closed":false,"points":[{"x":1,"y":2,"kind":"on","smooth":false}]}}"#,
            r#"{"op":"set_closed","name":"H","contour":1,"closed":true}"#,
            r#"{"op":"reverse_contour","name":"H","contour":0}"#,
            r#"{"op":"delete_points","name":"H","points":[[1,0]]}"#,
            r#"{"op":"transform","names":["H"],"matrix":[1,0,0.2,1,0,0]}"#,
            r#"{"op":"transform","matrix":[-1,0,0,1,0,0],"anchor":"advance"}"#,
            r#"{"op":"round_coordinates"}"#,
            r#"{"op":"set_metrics","x_height":510}"#,
            r#"{"op":"rename_glyph","name":"H","new_name":"Eta"}"#,
            r#"{"op":"rename_font","name":"Lines Two"}"#,
        ] {
            let response = line(&mut session, text);
            assert!(response.ok, "{text}: {response:?}");
        }
        let font = session.font().unwrap();
        assert_eq!(font.name, "Lines Two");
        assert_eq!(font.metrics.x_height, 510.0);
        let glyph = font.glyph("Eta").unwrap();
        assert_eq!(glyph.contours.len(), 1, "the one-point contour was deleted");
        assert!(glyph.contours[0].points.iter().all(|p| p.x.fract() == 0.0));

        let index = line(&mut session, r#"{"op":"index"}"#).data.unwrap();
        assert_eq!(index["glyphs"][0]["name"], json!("Eta"));
        assert_eq!(index["glyphs"][0]["unicode"], json!(72));
        assert_eq!(index["glyphs"][0]["contours"], json!(1));
    }
}
