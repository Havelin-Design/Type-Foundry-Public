//! Point alignment. The math is pure; the window sends one `set_points` command.

use foundry_app::{Handle, Pt};
use serde_json::json;

use crate::app::{FoundryWindow, Scope, Tone};

/// Where selected points should land. Top is the greater font Y, because Y grows up.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AlignTo {
    Left,
    CenterX,
    Right,
    Top,
    Middle,
    Bottom,
    DistributeX,
    DistributeY,
}

/// New absolute positions. Fewer than two points, or fewer than three for a distribute, yields
/// nothing so the font stays put.
pub fn align_points(points: &[(Handle, Pt)], how: AlignTo) -> Vec<(Handle, f64, f64)> {
    let need = if matches!(how, AlignTo::DistributeX | AlignTo::DistributeY) {
        3
    } else {
        2
    };
    if points.len() < need {
        return Vec::new();
    }
    let (min_x, max_x, min_y, max_y) = points.iter().fold(
        (f64::MAX, f64::MIN, f64::MAX, f64::MIN),
        |(x0, x1, y0, y1), (_, at)| (x0.min(at.x), x1.max(at.x), y0.min(at.y), y1.max(at.y)),
    );
    match how {
        AlignTo::Left => place(points, |at| (min_x, at.y)),
        AlignTo::CenterX => place(points, |at| ((min_x + max_x) / 2.0, at.y)),
        AlignTo::Right => place(points, |at| (max_x, at.y)),
        AlignTo::Bottom => place(points, |at| (at.x, min_y)),
        AlignTo::Middle => place(points, |at| (at.x, (min_y + max_y) / 2.0)),
        AlignTo::Top => place(points, |at| (at.x, max_y)),
        AlignTo::DistributeX => distribute(points, true),
        AlignTo::DistributeY => distribute(points, false),
    }
}

fn place(points: &[(Handle, Pt)], at: impl Fn(Pt) -> (f64, f64)) -> Vec<(Handle, f64, f64)> {
    points
        .iter()
        .map(|(handle, point)| {
            let (x, y) = at(*point);
            (*handle, x, y)
        })
        .collect()
}

/// Keep the first and last point on the axis and space the ones between them evenly.
fn distribute(points: &[(Handle, Pt)], horizontal: bool) -> Vec<(Handle, f64, f64)> {
    let mut order: Vec<(Handle, Pt)> = points.to_vec();
    order.sort_by(|(_, a), (_, b)| {
        let (a, b) = if horizontal { (a.x, b.x) } else { (a.y, b.y) };
        a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal)
    });
    let first = if horizontal {
        order[0].1.x
    } else {
        order[0].1.y
    };
    let last = if horizontal {
        order[order.len() - 1].1.x
    } else {
        order[order.len() - 1].1.y
    };
    let step = (last - first) / (order.len() - 1) as f64;
    order
        .into_iter()
        .enumerate()
        .map(|(index, (handle, at))| {
            let along = first + step * index as f64;
            if horizontal {
                (handle, along, at.y)
            } else {
                (handle, at.x, along)
            }
        })
        .collect()
}

impl FoundryWindow {
    pub fn align_selection(&mut self, how: AlignTo) {
        let Some(name) = self.current.clone() else {
            return;
        };
        let Some(outline) = self.outline(&name) else {
            return;
        };
        let current: Vec<(Handle, Pt)> = self
            .selection
            .iter()
            .filter_map(|handle| outline.point(*handle).map(|point| (*handle, point.at)))
            .collect();
        let mut placed = align_points(&current, how);
        if self.settings.snap {
            for (_, x, y) in &mut placed {
                *x = x.round();
                *y = y.round();
            }
        }
        let moved = placed.iter().any(|(handle, x, y)| {
            current
                .iter()
                .find(|(candidate, _)| candidate == handle)
                .is_some_and(|(_, at)| (at.x - x).abs() > 1e-6 || (at.y - y).abs() > 1e-6)
        });
        if !moved {
            self.status = ("Those points already line up.".into(), Tone::Quiet);
            return;
        }
        let points: Vec<serde_json::Value> = placed
            .iter()
            .map(|(handle, x, y)| {
                json!({ "contour": handle.contour, "point": handle.point, "x": x, "y": y })
            })
            .collect();
        if self
            .edit_json(
                json!({ "op": "set_points", "name": name, "points": points }),
                Scope::Glyph(name),
            )
            .is_some()
        {
            self.status = ("Aligned the points.".into(), Tone::Done);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn points() -> Vec<(Handle, Pt)> {
        vec![
            (
                Handle {
                    contour: 0,
                    point: 0,
                },
                Pt::new(0.0, 0.0),
            ),
            (
                Handle {
                    contour: 0,
                    point: 1,
                },
                Pt::new(10.0, 30.0),
            ),
            (
                Handle {
                    contour: 0,
                    point: 2,
                },
                Pt::new(40.0, 10.0),
            ),
        ]
    }

    fn xs(placed: &[(Handle, f64, f64)]) -> Vec<f64> {
        let mut placed = placed.to_vec();
        placed.sort_by_key(|(handle, _, _)| handle.point);
        placed.iter().map(|(_, x, _)| *x).collect()
    }

    #[test]
    fn aligns_to_the_selection_box() {
        let placed = align_points(&points(), AlignTo::Left);
        assert_eq!(xs(&placed), vec![0.0, 0.0, 0.0]);
        let placed = align_points(&points(), AlignTo::Right);
        assert_eq!(xs(&placed), vec![40.0, 40.0, 40.0]);
        let placed = align_points(&points(), AlignTo::Top);
        let ys: Vec<f64> = {
            let mut placed = placed.clone();
            placed.sort_by_key(|(handle, _, _)| handle.point);
            placed.iter().map(|(_, _, y)| *y).collect()
        };
        assert_eq!(ys, vec![30.0, 30.0, 30.0]);
        assert!(align_points(&points()[..1], AlignTo::Left).is_empty());
    }

    #[test]
    fn distributes_the_points_between_the_ends() {
        let placed = align_points(&points(), AlignTo::DistributeX);
        assert_eq!(xs(&placed), vec![0.0, 20.0, 40.0]);
        assert!(align_points(&points()[..2], AlignTo::DistributeX).is_empty());
    }
}
