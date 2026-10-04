//! One command session. The CLI, plugins, and agents all go through [`Session::execute`].

use std::path::Path;

use foundry_core::{Font, FoundryError, Glyph, blend_fonts, compatibility};
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
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn font(&self) -> Option<&Font> {
        self.font.as_ref()
    }

    pub fn execute(&mut self, command: Command) -> Response {
        match self.dispatch(command) {
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
}
