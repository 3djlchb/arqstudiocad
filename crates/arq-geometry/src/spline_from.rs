//! Lines, arcs, elliptical arcs and polyline paths as exact splines, so
//! JOIN can chain them onto a spline (ADR 0043 D4, maintainer, 2026-11-24):
//! a line of degree 1, an arc or an elliptical arc a rational quadratic in
//! pieces of a quarter turn at most (The NURBS Book A7.1; an ellipse is a
//! circle under an affine map, which a NURBS follows exactly).

use std::f64::consts::FRAC_PI_2;

use crate::arc::Arc;
use crate::ellipse::Ellipse;
use crate::error::{GeometryError, GeometryResult};
use crate::geometry::Geometry2;
use crate::line::Line;
use crate::point::Point2;
use crate::spline::Spline;
use crate::spline_split::join;
use crate::vector::Vector2;

impl Spline {
    /// A line as a spline of degree 1 from its start to its end.
    pub fn from_line(line: Line) -> GeometryResult<Self> {
        Self::new(
            1,
            vec![line.start(), line.end()],
            Vec::new(),
            vec![0.0, 0.0, 1.0, 1.0],
            false,
        )
    }

    /// An arc as a rational quadratic spline, counterclockwise from its
    /// start to its end, exactly.
    pub fn from_arc(arc: Arc) -> GeometryResult<Self> {
        let r = arc.radius();
        conic(
            arc.center(),
            Vector2::new(r, 0.0)?,
            Vector2::new(0.0, r)?,
            arc.start_angle(),
            arc.sweep(),
        )
    }

    /// An elliptical arc as a rational quadratic spline, from its start to
    /// its end parameter, exactly. Refused for a whole ellipse, which JOIN
    /// does not chain.
    pub fn from_elliptical_arc(e: Ellipse) -> GeometryResult<Self> {
        if e.is_full() {
            return Err(GeometryError::OutOfRange { what: "curve" });
        }
        conic(
            e.center(),
            e.major_axis(),
            e.minor_axis(),
            e.start_param(),
            e.sweep(),
        )
    }

    /// An open polyline path, its vertices and the bulge of the segment
    /// leaving each, as one spline: each straight segment of degree 1, each
    /// arc a rational quadratic, all raised to the highest degree among
    /// them and run on one after the other.
    pub fn from_path(vertices: &[(Point2, f64)]) -> GeometryResult<Self> {
        let pieces = vertices
            .windows(2)
            .map(|w| {
                let ((a, bulge), (b, _)) = (w[0], w[1]);
                if bulge == 0.0 {
                    Self::from_line(Line::new(a, b)?)
                } else {
                    let (arc, backward) = Arc::from_bulge(a, b, bulge)?;
                    let s = Self::from_arc(arc)?;
                    Ok(if backward { s.reversed() } else { s })
                }
            })
            .collect::<GeometryResult<Vec<_>>>()?;
        chain(&pieces)
    }

    /// `curve` as a spline, as JOIN chains it onto one: a line, an arc, an
    /// elliptical arc, or a spline itself. None for a circle or a whole
    /// ellipse, which JOIN never chains.
    pub fn from_curve(curve: &Geometry2) -> Option<GeometryResult<Self>> {
        match curve {
            Geometry2::Line(l) => Some(Self::from_line(*l)),
            Geometry2::Arc(a) => Some(Self::from_arc(*a)),
            Geometry2::Ellipse(e) if !e.is_full() => Some(Self::from_elliptical_arc(*e)),
            Geometry2::Spline(s) => Some(Ok(s.as_ref().clone())),
            Geometry2::Circle(_) | Geometry2::Ellipse(_) => None,
        }
    }
}

/// `pieces`, each starting where the one before ends, as one spline at the
/// highest degree among them.
fn chain(pieces: &[Spline]) -> GeometryResult<Spline> {
    let degree = pieces
        .iter()
        .map(Spline::degree)
        .max()
        .ok_or(GeometryError::OutOfRange { what: "path" })?;
    let mut joined: Option<Spline> = None;
    for piece in pieces {
        let piece = piece.elevated(degree)?;
        joined = Some(match joined {
            None => piece,
            Some(before) => {
                let shift = before.domain().1 - piece.domain().0;
                join(&before, &piece, shift)?
            }
        });
    }
    Ok(joined.expect("one piece at least"))
}

/// The conic `center + u cos t + v sin t` from parameter `start` through
/// `sweep` (positive counterclockwise), as a rational quadratic spline in
/// equal pieces of a quarter turn at most, its knots the pieces' ends
/// twice over, from 0 to 1.
fn conic(center: Point2, u: Vector2, v: Vector2, start: f64, sweep: f64) -> GeometryResult<Spline> {
    if !(sweep.is_finite() && sweep != 0.0) {
        return Err(GeometryError::OutOfRange { what: "arc sweep" });
    }
    let n = (sweep.abs() / FRAC_PI_2).ceil().max(1.0) as usize;
    let step = sweep / n as f64;
    let weight = (step / 2.0).cos();
    let at = |t: f64, scale: f64| center + (u * t.cos() + v * t.sin()) * scale;
    let mut points = vec![at(start, 1.0)];
    let mut weights = vec![1.0];
    let mut knots = vec![0.0; 3];
    for i in 0..n {
        let t0 = start + step * i as f64;
        points.push(at(t0 + step / 2.0, 1.0 / weight));
        points.push(at(t0 + step, 1.0));
        weights.extend([weight, 1.0]);
        if i + 1 < n {
            let k = (i + 1) as f64 / n as f64;
            knots.extend([k, k]);
        }
    }
    knots.extend([1.0; 3]);
    Spline::new(2, points, weights, knots, false)
}

#[cfg(test)]
#[path = "spline_from_tests.rs"]
mod tests;
