//! Geometry for the drawing window: the glyph outline as the session reports it, the
//! font-to-screen viewport, outline flattening, filling, and handle hit testing.
//!
//! Nothing here changes a font. The window reads glyphs with `Command::Glyph` and writes with
//! `Command::MovePoint`.

use serde_json::Value;

/// Havelin v2 chrome colors, as `0xRRGGBB`.
pub mod palette {
    pub const PAGE: u32 = 0x030303;
    pub const PANEL: u32 = 0x090907;
    pub const RAISED: u32 = 0x11110D;
    pub const HAIRLINE: u32 = 0x302A1E;
    pub const HAIRLINE_STRONG: u32 = 0x5B4A2D;
    pub const INK: u32 = 0xDED9CE;
    pub const MUTED: u32 = 0x9D988C;
    pub const BONE: u32 = 0xF3EEE4;
    pub const FOCUS: u32 = 0xD8FF00;
    pub const AMBER: u32 = 0xE8B65A;
    pub const SIGNAL: u32 = 0x8AE6A3;
    pub const ALERT: u32 = 0xF07461;
    pub const INVERSE: u32 = 0x030303;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pt {
    pub x: f64,
    pub y: f64,
}

impl Pt {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    fn lerp(self, other: Pt, t: f64) -> Pt {
        Pt::new(
            self.x + (other.x - self.x) * t,
            self.y + (other.y - self.y) * t,
        )
    }

    fn distance(self, other: Pt) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }
}

/// An axis-aligned box. In font units `y` grows up; on screen it grows down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min: Pt,
    pub max: Pt,
}

impl Bounds {
    pub fn width(&self) -> f64 {
        self.max.x - self.min.x
    }

    pub fn height(&self) -> f64 {
        self.max.y - self.min.y
    }

    fn include(&mut self, point: Pt) {
        self.min.x = self.min.x.min(point.x);
        self.min.y = self.min.y.min(point.y);
        self.max.x = self.max.x.max(point.x);
        self.max.y = self.max.y.max(point.y);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutlinePoint {
    pub at: Pt,
    pub on: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutlineContour {
    pub closed: bool,
    pub points: Vec<OutlinePoint>,
}

/// One glyph as returned by the `glyph` command.
#[derive(Debug, Clone, PartialEq)]
pub struct Outline {
    pub name: String,
    pub advance: f64,
    pub contours: Vec<OutlineContour>,
}

impl Outline {
    /// Read the `data` of a `glyph` command response.
    pub fn from_json(data: &Value) -> Option<Self> {
        let contours = data["contours"]
            .as_array()?
            .iter()
            .map(|contour| {
                let points = contour["points"]
                    .as_array()?
                    .iter()
                    .map(|point| {
                        Some(OutlinePoint {
                            at: Pt::new(point["x"].as_f64()?, point["y"].as_f64()?),
                            on: point["kind"].as_str()? == "on",
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some(OutlineContour {
                    closed: contour["closed"].as_bool()?,
                    points,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            name: data["name"].as_str()?.to_string(),
            advance: data["advance"].as_f64()?,
            contours,
        })
    }

    /// The box to fit: every point, the advance, and the vertical metrics given.
    pub fn bounds(&self, descender: f64, ascender: f64) -> Bounds {
        let mut bounds = Bounds {
            min: Pt::new(0.0, descender.min(ascender)),
            max: Pt::new(self.advance.max(0.0), descender.max(ascender)),
        };
        for contour in &self.contours {
            for point in &contour.points {
                bounds.include(point.at);
            }
        }
        bounds
    }

    pub fn point(&self, handle: Handle) -> Option<&OutlinePoint> {
        self.contours.get(handle.contour)?.points.get(handle.point)
    }
}

/// A point address, as `move_point` takes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handle {
    pub contour: usize,
    pub point: usize,
}

/// Font units to screen points. `origin` is where font (0, 0) lands on screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub scale: f64,
    pub origin: Pt,
}

impl Viewport {
    pub const MIN_SCALE: f64 = 0.005;
    pub const MAX_SCALE: f64 = 200.0;

    /// Center `bounds` in `canvas`, leaving `margin` screen points on the tighter side.
    pub fn fit(canvas: Bounds, bounds: Bounds, margin: f64) -> Self {
        let room_x = (canvas.width() - 2.0 * margin).max(1.0);
        let room_y = (canvas.height() - 2.0 * margin).max(1.0);
        let width = bounds.width().max(1.0);
        let height = bounds.height().max(1.0);
        let scale = (room_x / width)
            .min(room_y / height)
            .clamp(Self::MIN_SCALE, Self::MAX_SCALE);
        let canvas_center = Pt::new(
            (canvas.min.x + canvas.max.x) / 2.0,
            (canvas.min.y + canvas.max.y) / 2.0,
        );
        let font_center = Pt::new(
            (bounds.min.x + bounds.max.x) / 2.0,
            (bounds.min.y + bounds.max.y) / 2.0,
        );
        Self {
            scale,
            origin: Pt::new(
                canvas_center.x - font_center.x * scale,
                canvas_center.y + font_center.y * scale,
            ),
        }
    }

    pub fn to_screen(&self, font: Pt) -> Pt {
        Pt::new(
            self.origin.x + font.x * self.scale,
            self.origin.y - font.y * self.scale,
        )
    }

    pub fn to_font(&self, screen: Pt) -> Pt {
        Pt::new(
            (screen.x - self.origin.x) / self.scale,
            (self.origin.y - screen.y) / self.scale,
        )
    }

    /// Zoom by `factor`, keeping the font point under `anchor` where it is.
    pub fn zoom_at(&mut self, anchor: Pt, factor: f64) {
        let fixed = self.to_font(anchor);
        self.scale = (self.scale * factor).clamp(Self::MIN_SCALE, Self::MAX_SCALE);
        self.origin = Pt::new(
            anchor.x - fixed.x * self.scale,
            anchor.y + fixed.y * self.scale,
        );
    }

    pub fn pan(&mut self, dx: f64, dy: f64) {
        self.origin = Pt::new(self.origin.x + dx, self.origin.y + dy);
    }
}

/// The handle under `screen`, within `radius` screen points. An on-curve point wins over any
/// off-curve point in range; otherwise the nearest point wins.
pub fn hit_test(outline: &Outline, view: &Viewport, screen: Pt, radius: f64) -> Option<Handle> {
    let mut best_on: Option<(f64, Handle)> = None;
    let mut best_off: Option<(f64, Handle)> = None;
    for (contour_index, contour) in outline.contours.iter().enumerate() {
        for (point_index, point) in contour.points.iter().enumerate() {
            let distance = view.to_screen(point.at).distance(screen);
            if distance > radius {
                continue;
            }
            let handle = Handle {
                contour: contour_index,
                point: point_index,
            };
            let slot = if point.on {
                &mut best_on
            } else {
                &mut best_off
            };
            if slot.is_none_or(|(best, _)| distance < best) {
                *slot = Some((distance, handle));
            }
        }
    }
    best_on.or(best_off).map(|(_, handle)| handle)
}

/// Flatten one contour to a polyline in font units. One off-point between on-points is a
/// quadratic, two is a cubic. Longer runs are read as a quadratic chain with implied on-points.
/// Closed contours wrap; the polyline does not repeat its first point.
pub fn flatten(contour: &OutlineContour, steps: usize) -> Vec<Pt> {
    let steps = steps.max(1);
    let points = &contour.points;
    let Some(start) = points.iter().position(|point| point.on) else {
        return flatten_all_off(points, steps);
    };
    let count = points.len();
    let span = if contour.closed { count } else { count - start };
    let mut out = vec![points[start].at];
    let mut offs: Vec<Pt> = Vec::new();
    let mut from = points[start].at;
    for step in 1..=span {
        let index = (start + step) % count;
        if !contour.closed && start + step >= count {
            break;
        }
        let point = &points[index];
        if !point.on {
            offs.push(point.at);
            continue;
        }
        emit_segment(&mut out, from, &offs, point.at, steps);
        offs.clear();
        from = point.at;
    }
    if contour.closed && out.len() > 1 {
        out.pop();
    }
    out
}

fn emit_segment(out: &mut Vec<Pt>, from: Pt, offs: &[Pt], to: Pt, steps: usize) {
    match offs {
        [] => out.push(to),
        [control] => push_quad(out, from, *control, to, steps),
        [first, second] => {
            for step in 1..=steps {
                let t = step as f64 / steps as f64;
                out.push(cubic(from, *first, *second, to, t));
            }
        }
        _ => {
            let mut start = from;
            for (index, control) in offs.iter().enumerate() {
                let end = match offs.get(index + 1) {
                    Some(next) => control.lerp(*next, 0.5),
                    None => to,
                };
                push_quad(out, start, *control, end, steps);
                start = end;
            }
        }
    }
}

fn flatten_all_off(points: &[OutlinePoint], steps: usize) -> Vec<Pt> {
    let count = points.len();
    if count == 0 {
        return Vec::new();
    }
    let mid = |index: usize| points[index].at.lerp(points[(index + 1) % count].at, 0.5);
    let mut out = vec![mid(count - 1)];
    for (index, point) in points.iter().enumerate() {
        let from = *out.last().unwrap_or(&point.at);
        push_quad(&mut out, from, point.at, mid(index), steps);
    }
    out.pop();
    out
}

fn push_quad(out: &mut Vec<Pt>, from: Pt, control: Pt, to: Pt, steps: usize) {
    for step in 1..=steps {
        let t = step as f64 / steps as f64;
        out.push(from.lerp(control, t).lerp(control.lerp(to, t), t));
    }
}

fn cubic(p0: Pt, p1: Pt, p2: Pt, p3: Pt, t: f64) -> Pt {
    let a = p0.lerp(p1, t);
    let b = p1.lerp(p2, t);
    let c = p2.lerp(p3, t);
    a.lerp(b, t).lerp(b.lerp(c, t), t)
}

/// Filled spans of a horizontal line through closed polygons, by the nonzero winding rule.
pub fn scanline_spans(polygons: &[Vec<Pt>], y: f64) -> Vec<(f64, f64)> {
    let mut crossings: Vec<(f64, i32)> = Vec::new();
    for polygon in polygons {
        let count = polygon.len();
        if count < 3 {
            continue;
        }
        for index in 0..count {
            let a = polygon[index];
            let b = polygon[(index + 1) % count];
            let (winding, low, high) = if a.y < b.y { (1, a, b) } else { (-1, b, a) };
            // Half-open so a vertex shared by two edges counts once.
            if y < low.y || y >= high.y {
                continue;
            }
            let t = (y - low.y) / (high.y - low.y);
            crossings.push((low.x + (high.x - low.x) * t, winding));
        }
    }
    crossings.sort_by(|left, right| left.0.total_cmp(&right.0));
    let mut spans = Vec::new();
    let mut winding = 0;
    let mut start = 0.0;
    for (x, delta) in crossings {
        let before = winding;
        winding += delta;
        if before == 0 && winding != 0 {
            start = x;
        } else if before != 0 && winding == 0 && x > start {
            spans.push((start, x));
        }
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn on(x: f64, y: f64) -> OutlinePoint {
        OutlinePoint {
            at: Pt::new(x, y),
            on: true,
        }
    }

    fn off(x: f64, y: f64) -> OutlinePoint {
        OutlinePoint {
            at: Pt::new(x, y),
            on: false,
        }
    }

    fn close(a: Pt, b: Pt) -> bool {
        (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
    }

    fn canvas(width: f64, height: f64) -> Bounds {
        Bounds {
            min: Pt::new(0.0, 0.0),
            max: Pt::new(width, height),
        }
    }

    #[test]
    fn fit_centers_and_flips_the_glyph() {
        let glyph = Bounds {
            min: Pt::new(0.0, -200.0),
            max: Pt::new(500.0, 800.0),
        };
        let view = Viewport::fit(canvas(800.0, 600.0), glyph, 50.0);
        assert!((view.scale - 0.5).abs() < 1e-9, "height limits: 500 / 1000");
        let top_left = view.to_screen(Pt::new(0.0, 800.0));
        let bottom_right = view.to_screen(Pt::new(500.0, -200.0));
        assert!(close(top_left, Pt::new(275.0, 50.0)), "{top_left:?}");
        assert!(
            close(bottom_right, Pt::new(525.0, 550.0)),
            "{bottom_right:?}"
        );
        let back = view.to_font(Pt::new(400.0, 300.0));
        assert!(close(back, Pt::new(250.0, 300.0)), "{back:?}");
    }

    #[test]
    fn zoom_keeps_the_anchor_still() {
        let glyph = Bounds {
            min: Pt::new(0.0, 0.0),
            max: Pt::new(100.0, 100.0),
        };
        let mut view = Viewport::fit(canvas(200.0, 200.0), glyph, 0.0);
        let anchor = Pt::new(30.0, 170.0);
        let under = view.to_font(anchor);
        view.zoom_at(anchor, 3.0);
        assert!((view.scale - 6.0).abs() < 1e-9);
        assert!(close(view.to_font(anchor), under));
    }

    #[test]
    fn on_point_wins_over_a_nearer_off_point() {
        let outline = Outline {
            name: "o".into(),
            advance: 100.0,
            contours: vec![OutlineContour {
                closed: true,
                points: vec![on(0.0, 0.0), off(3.0, 0.0), off(50.0, 50.0), on(100.0, 0.0)],
            }],
        };
        let view = Viewport {
            scale: 1.0,
            origin: Pt::new(0.0, 0.0),
        };
        let hit = hit_test(&outline, &view, Pt::new(2.5, 0.0), 6.0);
        assert_eq!(
            hit,
            Some(Handle {
                contour: 0,
                point: 0
            })
        );
        let only_off = hit_test(&outline, &view, Pt::new(50.0, -50.0), 6.0);
        assert_eq!(
            only_off,
            Some(Handle {
                contour: 0,
                point: 2
            })
        );
        assert_eq!(hit_test(&outline, &view, Pt::new(30.0, 30.0), 6.0), None);
    }

    #[test]
    fn flattens_a_cubic_with_two_offs() {
        let contour = OutlineContour {
            closed: false,
            points: vec![
                on(0.0, 0.0),
                off(0.0, 100.0),
                off(100.0, 100.0),
                on(100.0, 0.0),
            ],
        };
        let line = flatten(&contour, 4);
        assert_eq!(line.len(), 5);
        assert!(close(line[0], Pt::new(0.0, 0.0)));
        assert!(close(line[2], Pt::new(50.0, 75.0)), "{:?}", line[2]);
        assert!(close(line[4], Pt::new(100.0, 0.0)));
    }

    #[test]
    fn flattens_a_quadratic_with_one_off() {
        let contour = OutlineContour {
            closed: true,
            points: vec![on(0.0, 0.0), off(50.0, 100.0), on(100.0, 0.0)],
        };
        let line = flatten(&contour, 2);
        // Quadratic midpoint, the end point, then the straight closing edge back to the start.
        assert_eq!(line.len(), 3);
        assert!(close(line[1], Pt::new(50.0, 50.0)), "{:?}", line[1]);
        assert!(close(line[2], Pt::new(100.0, 0.0)));
    }

    #[test]
    fn closed_contours_wrap_leading_offs() {
        let contour = OutlineContour {
            closed: true,
            points: vec![off(50.0, 100.0), on(100.0, 0.0), on(0.0, 0.0)],
        };
        let line = flatten(&contour, 2);
        assert!(close(line[0], Pt::new(100.0, 0.0)));
        assert!(line.iter().any(|point| close(*point, Pt::new(50.0, 50.0))));
    }

    #[test]
    fn nonzero_fill_leaves_counters_open() {
        let outer = vec![
            Pt::new(0.0, 0.0),
            Pt::new(100.0, 0.0),
            Pt::new(100.0, 100.0),
            Pt::new(0.0, 100.0),
        ];
        let inner = vec![
            Pt::new(25.0, 25.0),
            Pt::new(25.0, 75.0),
            Pt::new(75.0, 75.0),
            Pt::new(75.0, 25.0),
        ];
        let spans = scanline_spans(&[outer.clone(), inner], 50.0);
        assert_eq!(spans, vec![(0.0, 25.0), (75.0, 100.0)]);
        assert_eq!(scanline_spans(&[outer], 50.0), vec![(0.0, 100.0)]);
    }

    #[test]
    fn reads_a_glyph_response() {
        let data = json!({
            "name": "H", "unicode": 72, "advance": 600.0,
            "contours": [{ "closed": true, "points": [
                { "x": 0.0, "y": 0.0, "kind": "on", "smooth": false },
                { "x": 10.0, "y": 20.0, "kind": "off", "smooth": false }
            ]}]
        });
        let outline = Outline::from_json(&data).unwrap();
        assert_eq!(outline.advance, 600.0);
        assert!(!outline.contours[0].points[1].on);
        let bounds = outline.bounds(-200.0, 800.0);
        assert_eq!(bounds.max, Pt::new(600.0, 800.0));
        assert!(Outline::from_json(&json!({ "name": "H" })).is_none());
    }
}
