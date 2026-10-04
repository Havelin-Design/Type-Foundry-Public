//! Binary font import read by [`Font::load`]: TrueType (`.ttf`), OpenType with CFF or CFF2
//! outlines (`.otf`), and the first face of a collection (`.ttc`, `.otc`).
//!
//! Outlines arrive as the parser draws them. Composite glyphs are decomposed, TrueType implied
//! on-curve points become real on-curve points, and a variable font gives its default instance.
//! WOFF and WOFF2 are recognised and refused with a message that says so.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use ttf_parser::{Face, GlyphId, OutlineBuilder, Tag, name_id};

use crate::error::FoundryError;
use crate::font::{Contour, Font, Glyph, MAX_UPM, MIN_UPM, Point, PointKind, validate_glyph};

/// Windows language ID for English (United States) in the `name` table.
const ENGLISH_US: u16 = 0x0409;
/// Extensions [`Font::load`] reads as a binary font.
const IMPORT_EXTENSIONS: [&str; 4] = ["ttf", "otf", "ttc", "otc"];
/// Extensions that are known font formats but are not imported.
const REFUSED_EXTENSIONS: [&str; 2] = ["woff", "woff2"];

fn extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
}

/// True when [`Font::load`] should read the path as a binary font, including WOFF so it can
/// refuse it by name.
pub(crate) fn is_font_binary_path(path: &Path) -> bool {
    extension(path).is_some_and(|ext| {
        IMPORT_EXTENSIONS.contains(&ext.as_str()) || REFUSED_EXTENSIONS.contains(&ext.as_str())
    })
}

/// True for a binary format that can be opened but not written. `.ttf` is written by the
/// TrueType exporter, so it is not in this list.
pub(crate) fn is_read_only_path(path: &Path) -> bool {
    extension(path).is_some_and(|ext| {
        ext != "ttf"
            && (IMPORT_EXTENSIONS.contains(&ext.as_str())
                || REFUSED_EXTENSIONS.contains(&ext.as_str()))
    })
}

pub(crate) fn load_font_binary(path: &Path) -> Result<Font, FoundryError> {
    let bytes = fs::read(path).map_err(|err| FoundryError::Io(err.to_string()))?;
    let fallback = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    read_font_binary(&bytes, &fallback)
        .map_err(|message| FoundryError::Import(format!("{}: {message}", path.display())))
}

/// Read face 0 of TrueType, OpenType, or a collection. `fallback_name` names the font when the
/// file has no family name.
pub(crate) fn read_font_binary(bytes: &[u8], fallback_name: &str) -> Result<Font, String> {
    match bytes.get(..4) {
        Some(b"wOFF") => {
            return Err("WOFF is not imported yet; convert it to .ttf or .otf first".to_string());
        }
        Some(b"wOF2") => {
            return Err("WOFF2 is not imported yet; convert it to .ttf or .otf first".to_string());
        }
        _ => {}
    }
    let face = Face::parse(bytes, 0).map_err(|err| format!("not a readable font: {err}"))?;

    let upm = face.units_per_em();
    if !(MIN_UPM..=MAX_UPM).contains(&upm) {
        return Err(format!("units per em {upm} is outside {MIN_UPM}-{MAX_UPM}"));
    }
    let name = font_name(&face).unwrap_or_else(|| fallback_name.to_string());
    let mut font = Font::new(name, upm).map_err(|err| err.to_string())?;
    font.metrics.ascender = f64::from(face.ascender());
    font.metrics.descender = f64::from(face.descender());
    if let Some(cap_height) = face.capital_height().filter(|value| *value > 0) {
        font.metrics.cap_height = f64::from(cap_height);
    }
    if let Some(x_height) = face.x_height().filter(|value| *value > 0) {
        font.metrics.x_height = f64::from(x_height);
    }
    font.metrics.baseline = 0.0;

    let unicodes = unicode_map(&face);
    let post_names = post_names(&face);
    let mut taken = BTreeMap::new();
    let mut glyphs = Vec::with_capacity(usize::from(face.number_of_glyphs()));
    for index in 0..face.number_of_glyphs() {
        let id = GlyphId(index);
        let unicode = unicodes.get(&index).copied();
        let stored = post_names
            .as_ref()
            .and_then(|names| names.get(usize::from(index)).cloned().flatten());
        let name = match stored {
            Some(name) => unique_name(name, index, &mut taken),
            None => unique_name(glyph_name(&face, id, unicode), index, &mut taken),
        };
        let mut pen = Pen::default();
        // `None` means the glyph has no outline, such as a space. That is not an error.
        let _ = face.outline_glyph(id, &mut pen);
        let glyph = Glyph {
            name,
            unicode,
            advance: f64::from(face.glyph_hor_advance(id).unwrap_or(0)),
            contours: pen.finish(),
        };
        // Names are already unique, so skip insert_glyph's name search, which is slow for
        // fonts with tens of thousands of glyphs.
        validate_glyph(&glyph).map_err(|err| err.to_string())?;
        glyphs.push(glyph);
    }
    font.glyphs = glyphs;
    Ok(font)
}

/// Typographic family and style when present, else the legacy pair, as `"{family} {style}"`.
fn font_name(face: &Face<'_>) -> Option<String> {
    // Prefer the Windows English (United States) record, then any readable one.
    let lookup = |id: u16| {
        let read = |english_only: bool| {
            face.names()
                .into_iter()
                .filter(|name| name.name_id == id && name.is_unicode())
                .filter(|name| !english_only || name.language_id == ENGLISH_US)
                .find_map(|name| name.to_string())
                .map(|text| text.trim().to_string())
                .filter(|text| !text.is_empty())
        };
        read(true).or_else(|| read(false))
    };
    let family = lookup(name_id::TYPOGRAPHIC_FAMILY).or_else(|| lookup(name_id::FAMILY))?;
    let style = lookup(name_id::TYPOGRAPHIC_SUBFAMILY).or_else(|| lookup(name_id::SUBFAMILY));
    Some(match style {
        Some(style) => format!("{family} {style}"),
        None => family,
    })
}

/// The lowest Unicode value mapped to each glyph. Other values for the same glyph are dropped.
fn unicode_map(face: &Face<'_>) -> BTreeMap<u16, u32> {
    let mut map: BTreeMap<u16, u32> = BTreeMap::new();
    let Some(cmap) = face.tables().cmap else {
        return map;
    };
    for subtable in cmap.subtables {
        if !subtable.is_unicode() {
            continue;
        }
        subtable.codepoints(|codepoint| {
            if let Some(id) = subtable.glyph_index(codepoint) {
                map.entry(id.0)
                    .and_modify(|best| *best = (*best).min(codepoint))
                    .or_insert(codepoint);
            }
        });
    }
    map
}

/// Glyph names from a format 2 `post` table, read in one pass. `ttf-parser` finds each custom
/// name by walking the string list from the start, which is quadratic for large fonts.
fn post_names(face: &Face<'_>) -> Option<Vec<Option<String>>> {
    const STANDARD_NAMES: u16 = 258;
    let post = face.tables().post?;
    let raw = face.raw_face().table(Tag::from_bytes(b"post"))?;
    let read_u16 = |at: usize| {
        raw.get(at..at + 2)
            .map(|b| u16::from_be_bytes([b[0], b[1]]))
    };
    if raw.get(..4)? != [0, 2, 0, 0] {
        return None;
    }
    let count = read_u16(32)?;
    let custom: Vec<&str> = post.names().collect();
    let names = (0..count)
        .map(|glyph| {
            let index = read_u16(34 + 2 * usize::from(glyph))?;
            let name = if index < STANDARD_NAMES {
                post.glyph_name(GlyphId(glyph))?
            } else {
                custom.get(usize::from(index - STANDARD_NAMES)).copied()?
            };
            let name = name.trim();
            (!name.is_empty()).then(|| name.to_string())
        })
        .collect();
    Some(names)
}

fn glyph_name(face: &Face<'_>, id: GlyphId, unicode: Option<u32>) -> String {
    if let Some(name) = face
        .glyph_name(id)
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        return name.to_string();
    }
    match unicode {
        Some(code) if code <= 0xFFFF => format!("uni{code:04X}"),
        Some(code) => format!("u{code:05X}"),
        None if id.0 == 0 => ".notdef".to_string(),
        None => format!("glyph{:05}", id.0),
    }
}

fn unique_name(name: String, index: u16, taken: &mut BTreeMap<String, u16>) -> String {
    let name = if taken.contains_key(&name) {
        format!("{name}.{index}")
    } else {
        name
    };
    taken.insert(name.clone(), index);
    name
}

/// Collects drawn outlines as closed contours of on and off points.
#[derive(Default)]
struct Pen {
    contours: Vec<Contour>,
    points: Vec<Point>,
}

impl Pen {
    fn push(&mut self, x: f32, y: f32, kind: PointKind) {
        self.points.push(Point {
            x: f64::from(x),
            y: f64::from(y),
            kind,
            smooth: false,
        });
    }

    fn end_contour(&mut self) {
        let mut points = std::mem::take(&mut self.points);
        // The parser draws back to the start point. A closed contour does not repeat it, and any
        // off-points before it now wrap to the first point.
        if points.len() > 1
            && let (Some(first), Some(last)) = (points.first(), points.last())
            && first.x == last.x
            && first.y == last.y
            && last.kind == PointKind::On
        {
            points.pop();
        }
        if !points.is_empty() {
            self.contours.push(Contour {
                closed: true,
                points,
            });
        }
    }

    fn finish(mut self) -> Vec<Contour> {
        self.end_contour();
        self.contours
    }
}

impl OutlineBuilder for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.end_contour();
        self.push(x, y, PointKind::On);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.push(x, y, PointKind::On);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.push(x1, y1, PointKind::Off);
        self.push(x, y, PointKind::On);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.push(x1, y1, PointKind::Off);
        self.push(x2, y2, PointKind::Off);
        self.push(x, y, PointKind::On);
    }

    fn close(&mut self) {
        self.end_contour();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blend_fonts;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_IDS: AtomicU64 = AtomicU64::new(0);

    fn temp_dir() -> PathBuf {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let id = TEMP_IDS.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("typefoundry-sfnt-{tick}-{id}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn point(x: f64, y: f64, kind: PointKind) -> Point {
        Point {
            x,
            y,
            kind,
            smooth: false,
        }
    }

    fn face(name: &str, shift: f64, advance: f64) -> Font {
        let mut font = Font::new(name, 1000).unwrap();
        font.insert_glyph(Glyph {
            name: "H".into(),
            unicode: Some(u32::from('H')),
            advance,
            contours: vec![Contour {
                closed: true,
                points: vec![
                    point(40.0 + shift, 0.0, PointKind::On),
                    point(40.0 + shift, 700.0, PointKind::On),
                    point(160.0 + shift, 700.0, PointKind::On),
                    point(160.0 + shift, 0.0, PointKind::On),
                ],
            }],
        })
        .unwrap();
        font.insert_glyph(Glyph {
            name: "o".into(),
            unicode: Some(u32::from('o')),
            advance: 500.0,
            contours: vec![Contour {
                closed: true,
                points: vec![
                    point(50.0, 0.0, PointKind::On),
                    point(250.0 + shift, 0.0, PointKind::On),
                    point(250.0 + shift, 250.0, PointKind::Off),
                    point(50.0, 250.0, PointKind::On),
                ],
            }],
        })
        .unwrap();
        font.insert_glyph(Glyph {
            name: "space".into(),
            unicode: Some(32),
            advance: 250.0,
            contours: Vec::new(),
        })
        .unwrap();
        font
    }

    #[test]
    fn a_saved_ttf_opens_with_names_unicodes_and_points() {
        let dir = temp_dir();
        let path = dir.join("Round.ttf");
        let source = face("Round", 0.0, 400.0);
        source.save(&path).unwrap();

        let loaded = Font::load(&path).unwrap();
        assert_eq!(loaded.name, "Round Regular");
        assert_eq!(loaded.upm, 1000);
        assert_eq!(loaded.glyph_names(), vec![".notdef", "H", "o", "space"]);

        let h = loaded.glyph("H").unwrap();
        assert_eq!(h.unicode, Some(72));
        assert_eq!(h.advance, 400.0);
        assert_eq!(h.contours, source.glyph("H").unwrap().contours);

        let o = loaded.glyph("o").unwrap();
        let kinds: Vec<PointKind> = o.contours[0].points.iter().map(|p| p.kind).collect();
        assert_eq!(
            kinds,
            vec![PointKind::On, PointKind::On, PointKind::Off, PointKind::On]
        );
        assert_eq!(o.contours, source.glyph("o").unwrap().contours);

        let space = loaded.glyph("space").unwrap();
        assert!(space.contours.is_empty());
        assert_eq!(space.advance, 250.0);

        // Saving the import again keeps one .notdef and the same glyphs.
        let again = dir.join("Again.ttf");
        loaded.save(&again).unwrap();
        assert_eq!(Font::load(&again).unwrap().glyphs, loaded.glyphs);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn two_ttfs_blend_at_the_midpoint() {
        let dir = temp_dir();
        let narrow = dir.join("Narrow.ttf");
        let wide = dir.join("Wide.ttf");
        face("Narrow", 0.0, 400.0).save(&narrow).unwrap();
        face("Wide", 100.0, 800.0).save(&wide).unwrap();

        let mid = blend_fonts(
            &Font::load(&narrow).unwrap(),
            &Font::load(&wide).unwrap(),
            0.5,
        )
        .unwrap();
        let h = mid.glyph("H").unwrap();
        assert_eq!(h.advance, 600.0);
        assert_eq!(h.contours[0].points[0].x, 90.0);

        let _ = fs::remove_dir_all(dir);
    }

    // A minimal OpenType font with CFF outlines, built byte by byte so no third-party font is
    // needed. Glyph 1 is named `o` through a custom charset and draws one cubic.
    fn cff_font() -> Vec<u8> {
        fn index(items: &[&[u8]]) -> Vec<u8> {
            let mut out = (items.len() as u16).to_be_bytes().to_vec();
            if items.is_empty() {
                return out;
            }
            out.push(1);
            let mut offset = 1u8;
            out.push(offset);
            for item in items {
                offset += item.len() as u8;
                out.push(offset);
            }
            for item in items {
                out.extend_from_slice(item);
            }
            out
        }
        fn int(value: i32) -> Vec<u8> {
            let mut out = vec![29];
            out.extend_from_slice(&value.to_be_bytes());
            out
        }
        let small = |value: i32| (value + 139) as u8;

        let notdef: Vec<u8> = vec![14];
        let glyph: Vec<u8> = vec![
            small(10),
            small(0),
            21, // rmoveto to (10, 0)
            small(80),
            small(0),
            5, // rlineto to (90, 0)
            small(0),
            small(50),
            small(-30),
            small(50),
            small(-50),
            small(0),
            8,  // rrcurveto
            14, // endchar
        ];
        let header = [1u8, 0, 4, 1];
        let names = index(&[b"Cubic"]);
        let strings = index(&[b"o"]);
        let globals = index(&[]);
        let charset = [0u8, 0x01, 0x87]; // format 0, glyph 1 is SID 391, the first custom string
        let top_len = 2 * 6; // two operators, each a 5-byte integer and a 1-byte operator
        let top_index_len = 2 + 1 + 2 + top_len;
        let charset_at = header.len() + names.len() + top_index_len + strings.len() + globals.len();
        let charstrings_at = charset_at + charset.len();
        let mut top = int(charset_at as i32);
        top.push(15);
        top.extend(int(charstrings_at as i32));
        top.push(17);
        assert_eq!(top.len(), top_len);

        let mut cff = header.to_vec();
        cff.extend(names);
        cff.extend(index(&[&top]));
        cff.extend(strings);
        cff.extend(globals);
        cff.extend_from_slice(&charset);
        cff.extend(index(&[&notdef, &glyph]));

        let mut head = vec![0u8; 54];
        head[0..4].copy_from_slice(&0x0001_0000u32.to_be_bytes());
        head[12..16].copy_from_slice(&0x5F0F_3CF5u32.to_be_bytes());
        head[18..20].copy_from_slice(&1000u16.to_be_bytes());
        let mut hhea = vec![0u8; 36];
        hhea[0..4].copy_from_slice(&0x0001_0000u32.to_be_bytes());
        hhea[4..6].copy_from_slice(&760i16.to_be_bytes());
        hhea[6..8].copy_from_slice(&(-240i16).to_be_bytes());
        hhea[34..36].copy_from_slice(&2u16.to_be_bytes());
        let mut hmtx = Vec::new();
        for advance in [500u16, 520] {
            hmtx.extend_from_slice(&advance.to_be_bytes());
            hmtx.extend_from_slice(&0i16.to_be_bytes());
        }
        let mut maxp = 0x0000_5000u32.to_be_bytes().to_vec();
        maxp.extend_from_slice(&2u16.to_be_bytes());

        let tables: [(&[u8; 4], Vec<u8>); 5] = [
            (b"CFF ", cff),
            (b"head", head),
            (b"hhea", hhea),
            (b"hmtx", hmtx),
            (b"maxp", maxp),
        ];
        let mut out = b"OTTO".to_vec();
        out.extend_from_slice(&(tables.len() as u16).to_be_bytes());
        out.extend_from_slice(&[0, 64, 0, 2, 0, 16]);
        let mut offset = 12 + 16 * tables.len();
        let mut body = Vec::new();
        for (tag, data) in &tables {
            out.extend_from_slice(*tag);
            out.extend_from_slice(&0u32.to_be_bytes());
            out.extend_from_slice(&(offset as u32).to_be_bytes());
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            let mut padded = data.clone();
            padded.resize(data.len().div_ceil(4) * 4, 0);
            offset += padded.len();
            body.extend(padded);
        }
        out.extend(body);
        out
    }

    #[test]
    fn an_otf_with_cff_outlines_keeps_its_cubic() {
        let dir = temp_dir();
        let path = dir.join("Cubic.otf");
        fs::write(&path, cff_font()).unwrap();

        let font = Font::load(&path).unwrap();
        assert_eq!(font.name, "Cubic", "no name table, so the file stem");
        assert_eq!(font.metrics.ascender, 760.0);
        assert_eq!(font.glyph_names(), vec![".notdef", "o"]);
        let o = font.glyph("o").unwrap();
        assert_eq!(o.advance, 520.0);
        assert_eq!(o.unicode, None);
        let points: Vec<(f64, f64, PointKind)> = o.contours[0]
            .points
            .iter()
            .map(|p| (p.x, p.y, p.kind))
            .collect();
        assert_eq!(
            points,
            vec![
                (10.0, 0.0, PointKind::On),
                (90.0, 0.0, PointKind::On),
                (90.0, 50.0, PointKind::Off),
                (60.0, 100.0, PointKind::Off),
                (10.0, 100.0, PointKind::On),
            ]
        );
        assert!(o.contours[0].closed);

        let refused = font.save(&dir.join("Copy.otf")).unwrap_err();
        assert!(refused.to_string().contains("not written"), "{refused}");
        assert!(!dir.join("Copy.otf").exists());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn woff_and_garbage_are_refused_by_name() {
        let dir = temp_dir();
        let woff = dir.join("Web.woff2");
        fs::write(&woff, b"wOF2\0\x01\0\0rest").unwrap();
        let err = Font::load(&woff).unwrap_err().to_string();
        assert!(err.contains("WOFF2 is not imported"), "{err}");
        assert!(err.contains("Web.woff2"), "{err}");

        let junk = dir.join("Junk.ttf");
        fs::write(&junk, b"not a font").unwrap();
        let err = Font::load(&junk).unwrap_err();
        assert!(matches!(err, FoundryError::Import(_)), "{err}");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn duplicate_names_get_the_glyph_index() {
        let mut taken = BTreeMap::new();
        assert_eq!(unique_name("a".into(), 3, &mut taken), "a");
        assert_eq!(unique_name("a".into(), 7, &mut taken), "a.7");
    }

    #[test]
    fn path_kinds() {
        assert!(is_font_binary_path(Path::new("A.TTF")));
        assert!(is_font_binary_path(Path::new("a.otc")));
        assert!(is_font_binary_path(Path::new("a.woff")));
        assert!(!is_font_binary_path(Path::new("a.json")));
        assert!(!is_read_only_path(Path::new("a.ttf")));
        assert!(is_read_only_path(Path::new("a.otf")));
    }
}
