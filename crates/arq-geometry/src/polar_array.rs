//! Placement transforms for independent 2D polar arrays.

use crate::{GeometryError, GeometryResult, Point2, Transform2, Vector2};

/// Transforms for copies 1..count; item zero is the unchanged source.
/// A full turn divides the circle into count intervals without duplicating
/// the source. Partial fills include the final angle. Negative fills run CW.
/// Without item rotation the supplied base point orbits the center, and the
/// complete selection receives that translation without changing orientation.
pub fn polar_array_transforms(
    center: Point2,
    count: usize,
    fill: f64,
    rotate_items: bool,
    base: Option<Point2>,
) -> GeometryResult<Vec<Transform2>> {
    if !(2..=100_000).contains(&count) {
        return Err(GeometryError::OutOfRange {
            what: "polar array item count",
        });
    }
    if !fill.is_finite() || fill == 0.0 || fill.abs() > std::f64::consts::TAU {
        return Err(GeometryError::OutOfRange {
            what: "polar array fill angle",
        });
    }
    let delta = if rotate_items {
        None
    } else {
        let base = base.ok_or(GeometryError::OutOfRange {
            what: "polar array base point",
        })?;
        Some(Vector2::new(base.x() - center.x(), base.y() - center.y())?)
    };
    let intervals = if fill.abs() == std::f64::consts::TAU {
        count
    } else {
        count - 1
    };
    (1..count)
        .map(|i| {
            let angle = fill * (i as f64 / intervals as f64);
            match delta {
                None => Transform2::rotation(center, angle),
                Some(d) => {
                    let sin = angle.sin();
                    // cos(angle)-1 without cancellation for small angles.
                    let half = (angle * 0.5).sin();
                    let cos_minus_one = -2.0 * half * half;
                    let offset = Vector2::new(
                        d.x() * cos_minus_one - d.y() * sin,
                        d.x() * sin + d.y() * cos_minus_one,
                    )?;
                    Ok(Transform2::translation(offset))
                }
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "polar_array_tests.rs"]
mod tests;
