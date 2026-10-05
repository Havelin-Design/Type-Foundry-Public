//! Rectangle and oval outlines. The window commits each one with `add_contour`.

use foundry_app::{Outline, OutlineContour, OutlinePoint, Pt};
use serde_json::{Value, json};

/// A drag smaller than this on either side is not a shape.
pub const MIN_EXTENT: f64 = 4.0;

/// Cubic approximation of a quarter circle. Four of these close an oval.
pub const KAPPA: f64 = 0.5522847498307936;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapePoint {
    pub x: f64,
    pub y: f64,
    pub on: bool,
    pub smooth: bool,
}

fn corner(x: f64, y: f64) -> ShapePoint {
    ShapePoint {
        x,
        y,
        on: true,
        smooth: false,
    }
}

fn smooth_on(x: f64, y: f64) -> ShapePoint {
    ShapePoint {
        x,
        y,
        on: true,
        smooth: true,
    }
}

fn off(x: f64, y: f64) -> ShapePoint {
    ShapePoint {
        x,
        y,
        on: false,
        smooth: false,
    }
}

/// The drag box, smaller corner first. `None` when either side is under [`MIN_EXTENT`].
pub fn normalize(a: Pt, b: Pt) -> Option<(f64, f64, f64, f64)> {
    let x0 = a.x.min(b.x);
    let y0 = a.y.min(b.y);
    let x1 = a.x.max(b.x);
    let y1 = a.y.max(b.y);
    if x1 - x0 < MIN_EXTENT || y1 - y0 < MIN_EXTENT {
        return None;
    }
    Some((x0, y0, x1, y1))
}

/// Four on-curve corners, counter-clockwise from the lower left. Not smooth.
pub fn rectangle(a: Pt, b: Pt) -> Option<Vec<ShapePoint>> {
    let (x0, y0, x1, y1) = normalize(a, b)?;
    Some(vec![
        corner(x0, y0),
        corner(x1, y0),
        corner(x1, y1),
        corner(x0, y1),
    ])
}

/// Four cubic quadrants. The first on-curve point is the rightmost point.
/// The contour does not repeat that point; the last segment wraps to it.
pub fn oval(a: Pt, b: Pt) -> Option<Vec<ShapePoint>> {
    let (x0, y0, x1, y1) = normalize(a, b)?;
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let (rx, ry) = ((x1 - x0) / 2.0, (y1 - y0) / 2.0);
    let (kx, ky) = (rx * KAPPA, ry * KAPPA);
    Some(vec![
        smooth_on(cx + rx, cy),
        off(cx + rx, cy + ky),
        off(cx + kx, cy + ry),
        smooth_on(cx, cy + ry),
        off(cx - kx, cy + ry),
        off(cx - rx, cy + ky),
        smooth_on(cx - rx, cy),
        off(cx - rx, cy - ky),
        off(cx - kx, cy - ry),
        smooth_on(cx, cy - ry),
        off(cx + kx, cy - ry),
        off(cx + rx, cy - ky),
    ])
}

/// A closed contour ready for `add_contour`.
pub fn contour_value(points: &[ShapePoint]) -> Value {
    json!({
        "closed": true,
        "points": points
            .iter()
            .map(|point| json!({
                "x": point.x,
                "y": point.y,
                "kind": if point.on { "on" } else { "off" },
                "smooth": point.smooth,
            }))
            .collect::<Vec<_>>(),
    })
}

/// A one-contour outline for the drag ghost. It is not a font glyph.
pub fn outline_of(points: &[ShapePoint]) -> Outline {
    Outline {
        name: String::new(),
        unicode: None,
        advance: 0.0,
        contours: vec![OutlineContour {
            closed: true,
            points: points
                .iter()
                .map(|point| OutlinePoint {
                    at: Pt::new(point.x, point.y),
                    on: point.on,
                    smooth: point.smooth,
                })
                .collect(),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangle_corners_ignore_drag_direction() {
        let forward = rectangle(Pt::new(1.0, 2.0), Pt::new(11.0, 9.0)).unwrap();
        let backward = rectangle(Pt::new(11.0, 9.0), Pt::new(1.0, 2.0)).unwrap();
        assert_eq!(forward, backward);
        assert_eq!(forward.len(), 4);
        assert!(forward.iter().all(|point| point.on && !point.smooth));
        assert_eq!(
            forward
                .iter()
                .map(|point| (point.x, point.y))
                .collect::<Vec<_>>(),
            vec![(1.0, 2.0), (11.0, 2.0), (11.0, 9.0), (1.0, 9.0)]
        );
        let value = contour_value(&forward);
        assert_eq!(value["closed"], true);
        assert_eq!(value["points"].as_array().unwrap().len(), 4);
    }

    #[test]
    fn a_thin_drag_is_not_a_shape() {
        assert!(rectangle(Pt::new(0.0, 0.0), Pt::new(3.9, 40.0)).is_none());
        assert!(oval(Pt::new(0.0, 0.0), Pt::new(40.0, 3.9)).is_none());
        assert!(rectangle(Pt::new(0.0, 0.0), Pt::new(4.0, 4.0)).is_some());
    }

    #[test]
    fn oval_is_twelve_cubic_points_starting_at_the_right() {
        let points = oval(Pt::new(0.0, 0.0), Pt::new(200.0, 100.0)).unwrap();
        assert_eq!(points.len(), 12);
        let (cx, cy, rx, ry) = (100.0, 50.0, 100.0, 50.0);
        let (kx, ky) = (rx * KAPPA, ry * KAPPA);
        let expected = [
            (cx + rx, cy, true, true),
            (cx + rx, cy + ky, false, false),
            (cx + kx, cy + ry, false, false),
            (cx, cy + ry, true, true),
            (cx - kx, cy + ry, false, false),
            (cx - rx, cy + ky, false, false),
            (cx - rx, cy, true, true),
            (cx - rx, cy - ky, false, false),
            (cx - kx, cy - ry, false, false),
            (cx, cy - ry, true, true),
            (cx + kx, cy - ry, false, false),
            (cx + rx, cy - ky, false, false),
        ];
        for (point, (x, y, on, smooth)) in points.iter().zip(expected) {
            assert!((point.x - x).abs() < 1e-9 && (point.y - y).abs() < 1e-9);
            assert_eq!((point.on, point.smooth), (on, smooth));
        }
        assert!(
            points[0].x > points[3].x,
            "the first point is the rightmost"
        );
        assert_ne!(
            (points[0].x, points[0].y),
            (points[11].x, points[11].y),
            "the start point is not repeated"
        );
        let value = contour_value(&points);
        assert_eq!(value["closed"], true);
        assert_eq!(value["points"][1]["kind"], "off");
        assert_eq!(value["points"][0]["smooth"], true);
    }
}
