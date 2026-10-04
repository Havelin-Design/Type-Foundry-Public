use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::FoundryError;

pub const FONT_FORMAT: &str = "typefoundry.font";
pub const FONT_VERSION: u32 = 1;
pub const MIN_UPM: u16 = 16;
pub const MAX_UPM: u16 = 16384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PointKind {
    On,
    Off,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub kind: PointKind,
    pub smooth: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Contour {
    pub closed: bool,
    pub points: Vec<Point>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Glyph {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unicode: Option<u32>,
    pub advance: f64,
    pub contours: Vec<Contour>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    pub ascender: f64,
    pub cap_height: f64,
    pub x_height: f64,
    pub baseline: f64,
    pub descender: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Font {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub upm: u16,
    pub metrics: Metrics,
    pub glyphs: Vec<Glyph>,
}

impl Font {
    pub fn new(name: impl Into<String>, upm: u16) -> Result<Self, FoundryError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(FoundryError::Name);
        }
        if !(MIN_UPM..=MAX_UPM).contains(&upm) {
            return Err(FoundryError::Upm(upm));
        }
        let scale = f64::from(upm) / 1000.0;
        Ok(Self {
            format: FONT_FORMAT.to_string(),
            version: FONT_VERSION,
            name,
            upm,
            metrics: Metrics {
                ascender: 800.0 * scale,
                cap_height: 700.0 * scale,
                x_height: 500.0 * scale,
                baseline: 0.0,
                descender: -200.0 * scale,
            },
            glyphs: Vec::new(),
        })
    }

    pub fn glyph(&self, name: &str) -> Option<&Glyph> {
        self.glyphs.iter().find(|glyph| glyph.name == name)
    }

    pub fn glyph_mut(&mut self, name: &str) -> Option<&mut Glyph> {
        self.glyphs.iter_mut().find(|glyph| glyph.name == name)
    }

    pub fn glyph_names(&self) -> Vec<&str> {
        self.glyphs
            .iter()
            .map(|glyph| glyph.name.as_str())
            .collect()
    }

    /// Insert a glyph, or replace the glyph that already uses its name.
    pub fn insert_glyph(&mut self, glyph: Glyph) -> Result<(), FoundryError> {
        validate_glyph(&glyph)?;
        if let Some(existing) = self.glyph_mut(&glyph.name) {
            *existing = glyph;
        } else {
            self.glyphs.push(glyph);
        }
        Ok(())
    }

    pub fn to_json(&self) -> Result<String, FoundryError> {
        let mut text = serde_json::to_string_pretty(self)
            .map_err(|err| FoundryError::Json(err.to_string()))?;
        text.push('\n');
        Ok(text)
    }

    pub fn from_json(text: &str) -> Result<Self, FoundryError> {
        let font: Self =
            serde_json::from_str(text).map_err(|err| FoundryError::Json(err.to_string()))?;
        font.validate()?;
        Ok(font)
    }

    pub fn load(path: &Path) -> Result<Self, FoundryError> {
        if crate::ufo::is_ufo_path(path) {
            return crate::ufo::load_ufo(path);
        }
        let text = fs::read_to_string(path).map_err(|err| FoundryError::Io(err.to_string()))?;
        Self::from_json(&text)
    }

    pub fn save(&self, path: &Path) -> Result<(), FoundryError> {
        self.validate()?;
        if crate::ufo::is_ufo_path(path) {
            return crate::ufo::save_ufo(self, path);
        }
        if crate::ttf::is_ttf_path(path) {
            return crate::ttf::save_ttf(self, path);
        }
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|err| FoundryError::Io(err.to_string()))?;
        }
        fs::write(path, self.to_json()?).map_err(|err| FoundryError::Io(err.to_string()))
    }

    pub fn move_point(
        &mut self,
        name: &str,
        contour: usize,
        index: usize,
        x: f64,
        y: f64,
    ) -> Result<(), FoundryError> {
        if !x.is_finite() || !y.is_finite() {
            return Err(FoundryError::NonFinite);
        }
        let Some(glyph) = self.glyph_mut(name) else {
            return Err(FoundryError::MissingGlyph(name.to_string()));
        };
        let Some(contour) = glyph.contours.get_mut(contour) else {
            return Err(FoundryError::MissingPoint);
        };
        let Some(point) = contour.points.get_mut(index) else {
            return Err(FoundryError::MissingPoint);
        };
        point.x = x;
        point.y = y;
        Ok(())
    }

    fn validate(&self) -> Result<(), FoundryError> {
        if self.format != FONT_FORMAT {
            return Err(FoundryError::Format(self.format.clone()));
        }
        if self.version != FONT_VERSION {
            return Err(FoundryError::Version(self.version));
        }
        if self.name.trim().is_empty() {
            return Err(FoundryError::Name);
        }
        if !(MIN_UPM..=MAX_UPM).contains(&self.upm) {
            return Err(FoundryError::Upm(self.upm));
        }
        if !metrics_are_finite(&self.metrics) {
            return Err(FoundryError::NonFinite);
        }
        let mut seen = BTreeSet::new();
        for glyph in &self.glyphs {
            if !seen.insert(glyph.name.clone()) {
                return Err(FoundryError::DuplicateGlyph(glyph.name.clone()));
            }
            validate_glyph(glyph)?;
        }
        Ok(())
    }
}

pub fn validate_glyph(glyph: &Glyph) -> Result<(), FoundryError> {
    if glyph.name.trim().is_empty() {
        return Err(FoundryError::GlyphName);
    }
    if !glyph.advance.is_finite() {
        return Err(FoundryError::NonFinite);
    }
    for contour in &glyph.contours {
        if contour.points.is_empty() {
            return Err(FoundryError::EmptyContour(glyph.name.clone()));
        }
        for point in &contour.points {
            if !point.x.is_finite() || !point.y.is_finite() {
                return Err(FoundryError::NonFinite);
            }
        }
    }
    Ok(())
}

fn metrics_are_finite(metrics: &Metrics) -> bool {
    metrics.ascender.is_finite()
        && metrics.cap_height.is_finite()
        && metrics.x_height.is_finite()
        && metrics.baseline.is_finite()
        && metrics.descender.is_finite()
}
