//! Equal-length stations on an open, directed straight/bulged path.

use crate::{Arc, GeometryError, GeometryResult, Point2, Tolerance, Transform2, Vector2};
use std::f64::consts::FRAC_PI_2;

struct Segment {
    start: Point2,
    end: Point2,
    arc: Option<(Arc, bool)>,
    length: f64,
    until: f64,
}

impl Segment {
    fn sample(&self, fraction: f64) -> GeometryResult<(Point2, f64)> {
        let (point, tangent) = match self.arc {
            None => (
                Point2::new(
                    self.start.x() * (1.0 - fraction) + self.end.x() * fraction,
                    self.start.y() * (1.0 - fraction) + self.end.y() * fraction,
                )?,
                Vector2::new(self.end.x() - self.start.x(), self.end.y() - self.start.y())?.angle(),
            ),
            Some((arc, reversed)) => {
                let parameter = if reversed { 1.0 - fraction } else { fraction };
                let angle = arc.start_angle() + arc.sweep() * parameter;
                let (sin, cos) = angle.sin_cos();
                (
                    Point2::new(
                        arc.center().x() + arc.radius() * cos,
                        arc.center().y() + arc.radius() * sin,
                    )?,
                    angle + if reversed { -FRAC_PI_2 } else { FRAC_PI_2 },
                )
            }
        };
        // Stored endpoints avoid trigonometric drift at shared vertices.
        Ok((
            if fraction == 0.0 {
                self.start
            } else if fraction == 1.0 {
                self.end
            } else {
                point
            },
            tangent,
        ))
    }
}

/// N placements including both endpoints, ordered in stored path direction.
/// Positive/negative bulges retain traversal sense. Alignment preserves the
/// first item's orientation; later items turn by the tangent change. At an
/// exact interior vertex use the outgoing tangent, at the end the last one.
/// Sources are not part of this result and are never moved by this helper.
pub fn path_array_transforms(
    vertices: &[(Point2, f64)],
    base: Point2,
    count: usize,
    align: bool,
) -> GeometryResult<Vec<Transform2>> {
    if !(2..=100_000).contains(&count) {
        return Err(GeometryError::OutOfRange {
            what: "path array item count",
        });
    }
    if vertices.len() < 2 {
        return Err(GeometryError::ZeroLength);
    }
    let mut segments = Vec::with_capacity(vertices.len() - 1);
    let mut total = 0.0;
    for pair in vertices.windows(2) {
        let (start, bulge) = pair[0];
        let end = pair[1].0;
        if !bulge.is_finite() {
            return Err(GeometryError::NonFinite { what: "path bulge" });
        }
        let chord = Vector2::new(end.x() - start.x(), end.y() - start.y())?;
        let arc = if bulge == 0.0 {
            None
        } else {
            Some(Arc::from_bulge(start, end, bulge)?)
        };
        let length = arc.map_or_else(|| chord.length(), |(arc, _)| arc.length());
        if !length.is_finite() {
            return Err(GeometryError::NonFinite {
                what: "path segment length",
            });
        }
        if length <= 0.0 {
            return Err(GeometryError::ZeroLength);
        }
        total += length;
        if !total.is_finite() {
            return Err(GeometryError::NonFinite {
                what: "path length",
            });
        }
        segments.push(Segment {
            start,
            end,
            arc,
            length,
            until: total,
        });
    }
    // Allow accumulated floating-point roundoff, capped by model tolerance.
    let roundoff =
        (f64::EPSILON * total * (vertices.len() as f64 + 4.0)).min(Tolerance::DEFAULT.linear());
    let initial = segments[0].sample(0.0)?.1;
    (0..count)
        .map(|i| {
            let distance = total * (i as f64 / (count - 1) as f64);
            let index = if i == 0 {
                0
            } else {
                segments
                    .partition_point(|segment| segment.until <= distance + roundoff)
                    .min(segments.len() - 1)
            };
            let segment = &segments[index];
            let from = if index == 0 {
                0.0
            } else {
                segments[index - 1].until
            };
            let fraction = if i == count - 1 {
                1.0
            } else {
                ((distance - from) / segment.length).clamp(0.0, 1.0)
            };
            let (point, tangent) = segment.sample(fraction)?;
            Transform2::placement(base, point, if align { tangent - initial } else { 0.0 })
        })
        .collect()
}

#[cfg(test)]
#[path = "path_array_tests.rs"]
mod tests;
