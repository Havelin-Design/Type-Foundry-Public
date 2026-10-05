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

/// Where a font sits in its family. Files without this block load as the Regular of a family
/// named after the font.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Style {
    /// The family name shared by every style, such as `Wide`.
    pub family: String,
    /// The style name, such as `Regular`, `Italic`, or `Bold Italic`.
    pub name: String,
    /// OS/2 weight class, 1 to 1000. 400 is Regular, 700 is Bold.
    pub weight: u16,
    pub italic: bool,
    /// Degrees counter-clockwise from vertical, as in UFO and the `post` table. A typical
    /// italic that leans right is negative, such as -12.
    pub italic_angle: f64,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            family: String::new(),
            name: "Regular".to_string(),
            weight: 400,
            italic: false,
            italic_angle: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Font {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub upm: u16,
    pub metrics: Metrics,
    #[serde(default)]
    pub style: Style,
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
        let family = name.clone();
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
            style: Style {
                family,
                ..Style::default()
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
        let mut font: Self =
            serde_json::from_str(text).map_err(|err| FoundryError::Json(err.to_string()))?;
        font.fill_style();
        font.validate()?;
        Ok(font)
    }

    /// Read `typefoundry.font` JSON, a `.ufo` directory, a folder of SVG glyphs, face 0 of
    /// a `.ttf`, `.otf`, `.ttc`, or `.otc` file, or a WOFF 1 `.woff`.
    pub fn load(path: &Path) -> Result<Self, FoundryError> {
        if path.is_dir() {
            if crate::ufo::is_ufo_path(path) {
                return crate::ufo::load_ufo(path);
            }
            if crate::svgfont::is_svg_font_dir(path) {
                return crate::svgfont::load_svg_dir(path);
            }
            return Err(FoundryError::Import(format!(
                "{} is not a .ufo folder or a folder of SVG glyphs. Name each file with four hex digits, like 0041.svg for A.",
                path.display()
            )));
        }
        if crate::ufo::is_ufo_path(path) {
            return crate::ufo::load_ufo(path);
        }
        if crate::sfnt::is_font_binary_path(path) {
            return crate::sfnt::load_font_binary(path);
        }
        if crate::webfont::is_binary_font_path(path) {
            let bytes = fs::read(path).map_err(|err| FoundryError::Io(err.to_string()))?;
            return crate::webfont::load_binary_font(&bytes, None);
        }
        let text = fs::read_to_string(path).map_err(|err| FoundryError::Io(err.to_string()))?;
        crate::import::load_text(&text)
    }

    pub fn save(&self, path: &Path) -> Result<(), FoundryError> {
        self.validate()?;
        if crate::sfnt::is_read_only_path(path) {
            return Err(FoundryError::Import(format!(
                "{} can be opened but not written; save to .ttf, .ufo, or .json",
                path.display()
            )));
        }
        if crate::webfont::is_readonly_web_font_path(path) {
            return Err(FoundryError::Import(
                "save writes .json, .ufo, or .ttf. Use one of those paths after opening a web font."
                    .to_string(),
            ));
        }
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

    /// Give a font with no family name its own name as the family, and Regular as the style.
    pub(crate) fn fill_style(&mut self) {
        if self.style.family.trim().is_empty() {
            self.style.family = self.name.clone();
        }
        if self.style.name.trim().is_empty() {
            self.style.name = "Regular".to_string();
        }
    }

    /// `Family Style`, for display and the full-name record.
    pub fn full_name(&self) -> String {
        format!("{} {}", self.style.family, self.style.name)
    }

    /// `Family-Style` with spaces and punctuation removed, for file names and PostScript names.
    pub fn file_stem(&self) -> String {
        let squash = |text: &str| -> String {
            text.chars()
                .filter(|ch| ch.is_ascii_alphanumeric())
                .collect()
        };
        let family = squash(&self.style.family);
        let style = squash(&self.style.name);
        match (family.is_empty(), style.is_empty()) {
            (true, _) => "Font".to_string(),
            (false, true) => family,
            (false, false) => format!("{family}-{style}"),
        }
    }

    /// The four-style name pair older apps group by: a family of up to Regular, Italic, Bold,
    /// and Bold Italic. Other weights become their own legacy family, like `Wide Light`.
    pub fn legacy_names(&self) -> (String, String) {
        let bold = self.style.weight == 700;
        let ribbi = self.style.weight == 400 || bold;
        let style = match (bold, self.style.italic) {
            (true, true) => "Bold Italic",
            (true, false) => "Bold",
            (false, true) => "Italic",
            (false, false) => "Regular",
        };
        if ribbi {
            return (self.style.family.clone(), style.to_string());
        }
        let extra: Vec<&str> = self
            .style
            .name
            .split_whitespace()
            .filter(|word| !word.eq_ignore_ascii_case("italic"))
            .collect();
        let family = if extra.is_empty() {
            format!("{} W{}", self.style.family, self.style.weight)
        } else {
            format!("{} {}", self.style.family, extra.join(" "))
        };
        let style = if self.style.italic {
            "Italic"
        } else {
            "Regular"
        };
        (family, style.to_string())
    }

    pub(crate) fn validate(&self) -> Result<(), FoundryError> {
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
        if !metrics_are_finite(&self.metrics) || !self.style.italic_angle.is_finite() {
            return Err(FoundryError::NonFinite);
        }
        if !(1..=1000).contains(&self.style.weight) {
            return Err(FoundryError::Style(format!(
                "weight {} is outside 1-1000",
                self.style.weight
            )));
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
