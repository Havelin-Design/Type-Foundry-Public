//! UFO directories share [`Font::load`] and [`Font::save`] with the JSON working file.
//!
//! Anchors, guidelines, kerning, groups, and lib data are ignored. A glyph with
//! components or an image is refused. A quadratic segment is one off-curve point.
//! A cubic segment is two. Implied-on qcurves are refused.

use std::path::Path;

use norad::fontinfo::NonNegativeIntegerOrFloat;
use norad::fontinfo::StyleMapStyle;
use norad::{ContourPoint, PointType};

use crate::error::FoundryError;
use crate::font::{Contour, Font, Glyph, MAX_UPM, MIN_UPM, Point, PointKind};

pub(crate) fn is_ufo_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("ufo"))
}

pub(crate) fn load_ufo(path: &Path) -> Result<Font, FoundryError> {
    let ufo = norad::Font::load(path).map_err(|err| FoundryError::Ufo(err.to_string()))?;
    let info = &ufo.font_info;
    let upm = read_upm(info)?;
    let mut font = Font::new(font_name(info, path), upm)?;
    apply_metrics(&mut font, info)?;
    apply_style(&mut font, info);
    let mut glyphs: Vec<&norad::Glyph> = ufo.default_layer().iter().collect();
    glyphs.sort_by(|left, right| left.name().as_str().cmp(right.name().as_str()));
    for glyph in glyphs {
        font.insert_glyph(import_glyph(glyph)?)?;
    }
    Ok(font)
}

pub(crate) fn save_ufo(font: &Font, path: &Path) -> Result<(), FoundryError> {
    let mut ufo = norad::Font::new();
    ufo.font_info.family_name = Some(font.style.family.clone());
    ufo.font_info.style_name = Some(font.style.name.clone());
    let (legacy_family, legacy_style) = font.legacy_names();
    ufo.font_info.style_map_family_name = Some(legacy_family);
    ufo.font_info.style_map_style_name = Some(match legacy_style.as_str() {
        "Bold Italic" => StyleMapStyle::BoldItalic,
        "Bold" => StyleMapStyle::Bold,
        "Italic" => StyleMapStyle::Italic,
        _ => StyleMapStyle::Regular,
    });
    ufo.font_info.open_type_os2_weight_class = Some(u32::from(font.style.weight));
    ufo.font_info.italic_angle = Some(font.style.italic_angle);
    ufo.font_info.units_per_em = Some(NonNegativeIntegerOrFloat::from(u32::from(font.upm)));
    ufo.font_info.ascender = Some(font.metrics.ascender);
    ufo.font_info.cap_height = Some(font.metrics.cap_height);
    ufo.font_info.x_height = Some(font.metrics.x_height);
    ufo.font_info.descender = Some(font.metrics.descender);
    let layer = ufo.default_layer_mut();
    for glyph in &font.glyphs {
        layer.insert_glyph(export_glyph(glyph)?);
    }
    ufo.save(path)
        .map_err(|err| FoundryError::Ufo(err.to_string()))
}

fn read_upm(info: &norad::FontInfo) -> Result<u16, FoundryError> {
    let Some(value) = info.units_per_em.as_ref() else {
        return Err(FoundryError::Ufo("the UFO has no units per em".to_string()));
    };
    let upm = value.as_f64();
    if !upm.is_finite()
        || upm.fract() != 0.0
        || !(f64::from(MIN_UPM)..=f64::from(MAX_UPM)).contains(&upm)
    {
        return Err(FoundryError::Ufo(format!(
            "units per em {upm} is outside {MIN_UPM}-{MAX_UPM}"
        )));
    }
    Ok(upm as u16)
}

fn font_name(info: &norad::FontInfo, path: &Path) -> String {
    let family = info
        .family_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty());
    let style = info
        .style_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty());
    match (family, style) {
        (Some(family), Some(style)) => format!("{family} {style}"),
        (Some(family), None) => family.to_string(),
        (None, Some(style)) => style.to_string(),
        (None, None) => path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .filter(|stem| !stem.trim().is_empty())
            .unwrap_or("Untitled")
            .to_string(),
    }
}

/// Family, style, weight, and italic from fontinfo. Missing values keep the Regular defaults.
fn apply_style(font: &mut Font, info: &norad::FontInfo) {
    let clean = |text: &Option<String>| {
        text.as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
    };
    if let Some(family) = clean(&info.family_name) {
        font.style.family = family;
    }
    if let Some(style) = clean(&info.style_name) {
        font.style.name = style;
    }
    if let Some(weight) = info.open_type_os2_weight_class {
        font.style.weight = u16::try_from(weight.clamp(1, 1000)).unwrap_or(400);
    }
    if let Some(angle) = info.italic_angle.filter(|angle| angle.is_finite()) {
        font.style.italic_angle = angle;
    }
    let mapped_italic = matches!(
        info.style_map_style_name,
        Some(StyleMapStyle::Italic | StyleMapStyle::BoldItalic)
    );
    font.style.italic = mapped_italic
        || font.style.italic_angle != 0.0
        || font.style.name.to_lowercase().contains("italic")
        || font.style.name.to_lowercase().contains("oblique");
}

fn apply_metrics(font: &mut Font, info: &norad::FontInfo) -> Result<(), FoundryError> {
    if let Some(value) = info.ascender {
        font.metrics.ascender = value;
    }
    if let Some(value) = info.cap_height {
        font.metrics.cap_height = value;
    }
    if let Some(value) = info.x_height {
        font.metrics.x_height = value;
    }
    if let Some(value) = info.descender {
        font.metrics.descender = value;
    }
    font.metrics.baseline = 0.0;
    if !font.metrics.ascender.is_finite()
        || !font.metrics.cap_height.is_finite()
        || !font.metrics.x_height.is_finite()
        || !font.metrics.descender.is_finite()
    {
        return Err(FoundryError::NonFinite);
    }
    Ok(())
}

fn import_glyph(glyph: &norad::Glyph) -> Result<Glyph, FoundryError> {
    let name = glyph.name().as_str();
    if !glyph.components.is_empty() {
        return Err(FoundryError::Ufo(format!(
            "glyph {name} has components, which are not imported"
        )));
    }
    if glyph.image.is_some() {
        return Err(FoundryError::Ufo(format!(
            "glyph {name} has an image, which is not imported"
        )));
    }
    let unicode = glyph.codepoints.iter().next().map(u32::from);
    let contours = glyph
        .contours
        .iter()
        .map(|contour| import_contour(contour, name))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Glyph {
        name: name.to_string(),
        unicode,
        advance: glyph.width,
        contours,
    })
}

fn import_contour(contour: &norad::Contour, glyph_name: &str) -> Result<Contour, FoundryError> {
    if contour.points.is_empty() {
        return Err(FoundryError::EmptyContour(glyph_name.to_string()));
    }
    if contour
        .points
        .iter()
        .skip(1)
        .any(|point| point.typ == PointType::Move)
    {
        return Err(FoundryError::Ufo(format!(
            "glyph {glyph_name} has a move point that is not first"
        )));
    }
    let on_indexes = on_indexes(&contour.points, |point| point.typ != PointType::OffCurve);
    let closed = contour.is_closed();
    check_segments(&contour.points, &on_indexes, closed, glyph_name)?;
    let points = contour
        .points
        .iter()
        .map(|point| {
            let kind = if point.typ == PointType::OffCurve {
                PointKind::Off
            } else {
                PointKind::On
            };
            Point {
                x: point.x,
                y: point.y,
                kind,
                smooth: kind == PointKind::On && point.smooth,
            }
        })
        .collect();
    Ok(Contour { closed, points })
}

fn check_segments(
    points: &[norad::ContourPoint],
    on_indexes: &[usize],
    closed: bool,
    glyph_name: &str,
) -> Result<(), FoundryError> {
    if on_indexes.is_empty() {
        return Err(FoundryError::Ufo(format!(
            "glyph {glyph_name} has a contour with no on-curve points"
        )));
    }
    if !closed && (on_indexes[0] != 0 || *on_indexes.last().unwrap() != points.len() - 1) {
        return Err(FoundryError::Ufo(format!(
            "glyph {glyph_name} has an open contour that starts or ends off-curve"
        )));
    }
    for (nth, &on_index) in on_indexes.iter().enumerate() {
        let offs = off_count(points.len(), on_indexes, nth, closed);
        let typ = points[on_index].typ;
        let valid = match typ {
            PointType::Move => !closed && nth == 0 && offs == 0,
            PointType::Line => offs == 0,
            PointType::QCurve => offs == 1,
            PointType::Curve => offs <= 2,
            PointType::OffCurve => false,
        };
        if !valid {
            return Err(FoundryError::Ufo(format!(
                "glyph {glyph_name} has a {typ} point with {offs} off-curve points before it"
            )));
        }
    }
    Ok(())
}

fn export_glyph(glyph: &Glyph) -> Result<norad::Glyph, FoundryError> {
    if norad::Name::new(&glyph.name).is_err() {
        return Err(FoundryError::Ufo(format!(
            "glyph name {} is not a valid UFO name",
            glyph.name
        )));
    }
    let mut exported = norad::Glyph::new(&glyph.name);
    exported.width = glyph.advance;
    if let Some(code) = glyph.unicode {
        let Some(ch) = char::from_u32(code) else {
            return Err(FoundryError::Ufo(format!(
                "glyph {} has a unicode value that is not a character",
                glyph.name
            )));
        };
        exported.codepoints.insert(ch);
    }
    for contour in &glyph.contours {
        exported
            .contours
            .push(export_contour(contour, &glyph.name)?);
    }
    Ok(exported)
}

fn export_contour(contour: &Contour, glyph_name: &str) -> Result<norad::Contour, FoundryError> {
    let on_indexes = on_indexes(&contour.points, |point| point.kind == PointKind::On);
    if on_indexes.is_empty() {
        return Err(FoundryError::Ufo(format!(
            "glyph {glyph_name} has a contour with no on-curve points"
        )));
    }
    if !contour.closed
        && (on_indexes[0] != 0 || *on_indexes.last().unwrap() != contour.points.len() - 1)
    {
        return Err(FoundryError::Ufo(format!(
            "glyph {glyph_name} has an open contour that starts or ends off-curve"
        )));
    }
    let mut exported = Vec::with_capacity(contour.points.len());
    for (nth, &on_index) in on_indexes.iter().enumerate() {
        let offs = off_count(contour.points.len(), &on_indexes, nth, contour.closed);
        let typ = if !contour.closed && nth == 0 {
            PointType::Move
        } else {
            match offs {
                0 => PointType::Line,
                1 => PointType::QCurve,
                2 => PointType::Curve,
                _ => {
                    return Err(FoundryError::Ufo(format!(
                        "glyph {glyph_name} has a segment with {offs} off-curve points"
                    )));
                }
            }
        };
        for index in between_indexes(contour.points.len(), &on_indexes, nth, contour.closed) {
            exported.push(ufo_point(&contour.points[index], PointType::OffCurve));
        }
        exported.push(ufo_point(&contour.points[on_index], typ));
    }
    if exported.len() != contour.points.len() {
        return Err(FoundryError::Ufo(format!(
            "glyph {glyph_name} has a contour that could not be written"
        )));
    }
    Ok(norad::Contour::new(exported, None))
}

fn ufo_point(point: &Point, typ: PointType) -> ContourPoint {
    ContourPoint::new(
        point.x,
        point.y,
        typ,
        typ != PointType::OffCurve && point.smooth,
        None,
        None,
    )
}

fn on_indexes<T>(points: &[T], is_on: impl Fn(&T) -> bool) -> Vec<usize> {
    points
        .iter()
        .enumerate()
        .filter(|(_, point)| is_on(point))
        .map(|(index, _)| index)
        .collect()
}

fn off_count(len: usize, on_indexes: &[usize], nth: usize, closed: bool) -> usize {
    between_indexes(len, on_indexes, nth, closed).len()
}

fn between_indexes(len: usize, on_indexes: &[usize], nth: usize, closed: bool) -> Vec<usize> {
    if !closed && nth == 0 {
        return Vec::new();
    }
    let from = if nth == 0 {
        *on_indexes.last().unwrap()
    } else {
        on_indexes[nth - 1]
    };
    let to = on_indexes[nth];
    let mut indexes = Vec::new();
    if len == 0 {
        return indexes;
    }
    let mut index = (from + 1) % len;
    while index != to {
        indexes.push(index);
        index = (index + 1) % len;
        if indexes.len() > len {
            break;
        }
    }
    indexes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blend_fonts;
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
        let path = std::env::temp_dir().join(format!("typefoundry-ufo-{tick}-{id}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn line(x: f64, y: f64) -> ContourPoint {
        ContourPoint::new(x, y, PointType::Line, false, None, None)
    }

    fn write_square(path: &Path, family: &str, x: f64, advance: f64) {
        let mut font = norad::Font::new();
        font.font_info.family_name = Some(family.to_string());
        font.font_info.units_per_em = Some(NonNegativeIntegerOrFloat::from(1000_u32));
        let mut glyph = norad::Glyph::new("H");
        glyph.width = advance;
        glyph.codepoints.insert('H');
        glyph.contours.push(norad::Contour::new(
            vec![
                line(x, 0.0),
                line(x + 80.0, 0.0),
                line(x + 80.0, 120.0),
                line(x, 120.0),
            ],
            None,
        ));
        let mut later = norad::Glyph::new("Z");
        later.width = advance;
        font.default_layer_mut().insert_glyph(later);
        font.default_layer_mut().insert_glyph(glyph);
        font.save(path).unwrap();
    }

    #[test]
    fn blends_two_synthetic_ufos_at_the_midpoint() {
        let dir = temp_dir();
        let narrow = dir.join("Narrow.ufo");
        let wide = dir.join("Wide.ufo");
        let mid = dir.join("Mid.ufo");
        write_square(&narrow, "Narrow", 40.0, 400.0);
        write_square(&wide, "Wide", 140.0, 800.0);

        let left = Font::load(&narrow).unwrap();
        let right = Font::load(&wide).unwrap();
        assert_eq!(left.glyph_names(), vec!["H", "Z"]);
        let blended = blend_fonts(&left, &right, 0.5).unwrap();
        blended.save(&mid).unwrap();
        let opened = Font::load(&mid).unwrap();
        // UFO stores family and style apart; the blend of two families is its own family.
        assert_eq!(opened.style.family, "Narrow / Wide @ 0.5");
        assert_eq!(opened.style.name, "Regular");
        assert_eq!(opened.name, "Narrow / Wide @ 0.5 Regular");
        assert_eq!(opened.glyph("H").unwrap().advance, 600.0);
        assert_eq!(opened.glyph("H").unwrap().contours[0].points[0].x, 90.0);
        assert_eq!(opened.glyph("H").unwrap().unicode, Some('H' as u32));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn round_trips_a_cubic_and_an_open_quadratic() {
        let dir = temp_dir();
        let path = dir.join("curves.ufo");
        let mut font = Font::new("Curves", 1000).unwrap();
        font.insert_glyph(Glyph {
            name: "C".into(),
            unicode: Some('C' as u32),
            advance: 500.0,
            contours: vec![
                Contour {
                    closed: true,
                    points: vec![
                        Point {
                            x: 0.0,
                            y: 0.0,
                            kind: PointKind::On,
                            smooth: false,
                        },
                        Point {
                            x: 40.0,
                            y: 80.0,
                            kind: PointKind::Off,
                            smooth: false,
                        },
                        Point {
                            x: 80.0,
                            y: 80.0,
                            kind: PointKind::Off,
                            smooth: false,
                        },
                        Point {
                            x: 120.0,
                            y: 0.0,
                            kind: PointKind::On,
                            smooth: true,
                        },
                    ],
                },
                Contour {
                    closed: false,
                    points: vec![
                        Point {
                            x: 10.0,
                            y: 10.0,
                            kind: PointKind::On,
                            smooth: false,
                        },
                        Point {
                            x: 30.0,
                            y: 40.0,
                            kind: PointKind::Off,
                            smooth: false,
                        },
                        Point {
                            x: 50.0,
                            y: 10.0,
                            kind: PointKind::On,
                            smooth: false,
                        },
                    ],
                },
            ],
        })
        .unwrap();
        font.save(&path).unwrap();
        let opened = Font::load(&path).unwrap();
        assert_eq!(opened.glyph("C"), font.glyph("C"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn refuses_a_component_and_names_the_glyph() {
        let dir = temp_dir();
        let path = dir.join("parts.ufo");
        let mut font = norad::Font::new();
        font.font_info.family_name = Some("Parts".into());
        font.font_info.units_per_em = Some(NonNegativeIntegerOrFloat::from(1000_u32));
        let mut base = norad::Glyph::new("B");
        base.contours.push(norad::Contour::new(
            vec![line(0.0, 0.0), line(10.0, 0.0), line(10.0, 10.0)],
            None,
        ));
        let mut accented = norad::Glyph::new("A");
        accented.components.push(norad::Component::new(
            norad::Name::new("B").unwrap(),
            norad::AffineTransform::default(),
            None,
        ));
        font.default_layer_mut().insert_glyph(base);
        font.default_layer_mut().insert_glyph(accented);
        font.save(&path).unwrap();
        let error = Font::load(&path).unwrap_err();
        assert!(error.to_string().contains("glyph A"), "{error}");
        assert!(error.to_string().contains("components"), "{error}");
        let _ = fs::remove_dir_all(dir);
    }
}
