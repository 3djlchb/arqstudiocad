//! OFFSET of an ellipse or a spline (ADR 0043 D4): a cubic spline through
//! points of the exact offset, its tangents at the ends those of the exact
//! offset, points added until it strays from the exact offset by no more
//! than a millionth of the curve's size, the diagonal of its box.

use std::f64::consts::FRAC_PI_2;

use crate::ellipse::Ellipse;
use crate::error::{GeometryError, GeometryResult};
use crate::point::Point2;
use crate::spline::Spline;
use crate::spline_fit::{SplineFit, SplineKnots};
use crate::tolerance::Tolerance;
use crate::vector::Vector2;

/// How far an offset may stray from the exact one, over the curve's size.
const PRECISION: f64 = 1e-6;

/// Points the offset starts from in each span: a quarter turn of an
/// ellipse, a span between a spline's knots.
const START: usize = 4;

/// Samples a span is checked at for curling and corners.
const CHECKS: usize = 64;

/// The most points an offset is fitted through before it gives up.
const MOST: usize = 4096;

/// The angle, in radians, past which a spline's tangent breaks at a knot.
const CORNER: f64 = 1e-6;

/// An ellipse or a spline, smooth between its parameters.
#[derive(Clone, Copy)]
enum Smooth<'a> {
    Ellipse(Ellipse),
    Spline(&'a Spline),
}

impl Smooth<'_> {
    /// The parameters it runs over, and whether it closes on itself.
    fn domain(self) -> (f64, f64) {
        match self {
            Self::Ellipse(e) => (e.start_param(), e.start_param() + e.sweep()),
            Self::Spline(s) => s.domain(),
        }
    }

    fn is_closed(self) -> bool {
        match self {
            Self::Ellipse(e) => e.is_full(),
            Self::Spline(s) => s.is_closed(),
        }
    }

    fn point(self, t: f64) -> Point2 {
        match self {
            Self::Ellipse(e) => e.point_at_param(t),
            Self::Spline(s) => s.point_at(t),
        }
    }

    fn derivative(self, t: f64) -> Vector2 {
        match self {
            Self::Ellipse(e) => e.derivative_at(t),
            Self::Spline(s) => s.derivative_at(t),
        }
    }

    /// Its signed curvature at `t`, positive where it turns left: exact
    /// for an ellipse, from the exact derivative on either side for a
    /// spline, within the span holding `t`.
    fn curvature(self, t: f64, span: (f64, f64)) -> f64 {
        let d1 = self.derivative(t);
        let d2 = match self {
            Self::Ellipse(e) => e.center() - e.point_at_param(t),
            Self::Spline(s) => {
                let h = 1e-6 * (span.1 - span.0);
                let (a, b) = ((t - h).max(span.0), (t + h).min(span.1));
                (s.derivative_at(b) - s.derivative_at(a)) * (1.0 / (b - a))
            }
        };
        d1.cross(d2) / d1.length().powi(3)
    }

    /// The parameters bounding its smooth spans: an ellipse's quarter
    /// turns, a spline's distinct knots.
    fn breaks(self) -> Vec<f64> {
        match self {
            Self::Ellipse(e) => {
                let (a, b) = self.domain();
                let n = (e.sweep() / FRAC_PI_2).ceil().max(1.0) as usize;
                (0..=n).map(|i| a + (b - a) * i as f64 / n as f64).collect()
            }
            Self::Spline(s) => s.span_breaks(),
        }
    }

    fn distance_to(self, p: Point2) -> f64 {
        match self {
            Self::Ellipse(e) => e.distance_to(p),
            Self::Spline(s) => s.distance_to(p),
        }
    }

    fn closest_param(self, p: Point2) -> f64 {
        match self {
            Self::Ellipse(e) => {
                // On the arc's own run of parameters.
                let (a, _) = self.domain();
                let t = e.closest_param(p);
                a + (t - a).rem_euclid(std::f64::consts::TAU)
            }
            Self::Spline(s) => s.closest_param(p),
        }
    }

    /// Its size: the diagonal of its box.
    fn size(self) -> f64 {
        let b = match self {
            Self::Ellipse(e) => e.bounds(),
            Self::Spline(s) => s.bounds(),
        };
        b.min().distance_to(b.max())
    }
}

/// The offset of ellipse `e` `distance` away, on the side `toward` lies.
pub(crate) fn offset_ellipse(
    e: Ellipse,
    distance: f64,
    toward: Point2,
    tol: Tolerance,
) -> GeometryResult<Spline> {
    offset(Smooth::Ellipse(e), distance, toward, tol)
}

/// The offset of spline `s` `distance` away, on the side `toward` lies.
pub(crate) fn offset_spline(
    s: &Spline,
    distance: f64,
    toward: Point2,
    tol: Tolerance,
) -> GeometryResult<Spline> {
    offset(Smooth::Spline(s), distance, toward, tol)
}

/// How far `p` is from ellipse `e` or spline `s`, as OFFSET's Through
/// measures it.
pub(crate) fn ellipse_distance(e: Ellipse, p: Point2) -> f64 {
    Smooth::Ellipse(e).distance_to(p)
}

pub(crate) fn spline_distance(s: &Spline, p: Point2) -> f64 {
    Smooth::Spline(s).distance_to(p)
}

fn offset(c: Smooth, distance: f64, toward: Point2, tol: Tolerance) -> GeometryResult<Spline> {
    let side = side_of(c, toward, tol)?;
    let breaks = breaks_checked(c)?;
    check_curl(c, &breaks, side * distance)?;
    let shift = side * distance;
    let at = |t: f64| -> Point2 {
        let d = c.derivative(t);
        c.point(t) + d.perp() * (shift / d.length())
    };
    let closed = c.is_closed();
    let mut params: Vec<f64> = Vec::new();
    for w in breaks.windows(2) {
        for i in 0..START {
            params.push(w[0] + (w[1] - w[0]) * i as f64 / START as f64);
        }
    }
    let (_, end) = c.domain();
    if !closed {
        params.push(end);
    }
    let precision = PRECISION * c.size();
    let (start_tangent, end_tangent) = if closed {
        (None, None)
    } else {
        (Some(c.derivative(params[0])), Some(c.derivative(end)))
    };
    loop {
        let points: Vec<Point2> = params.iter().map(|&t| at(t)).collect();
        let fit = SplineFit::new(
            points,
            SplineKnots::Chord,
            start_tangent,
            end_tangent,
            closed,
        )?;
        let made = fit.spline()?;
        // Each span of the fit lies between two of its points: checked at
        // its quarters against the exact offset.
        let spans = made.span_breaks();
        let intervals = if closed {
            params.len()
        } else {
            params.len() - 1
        };
        debug_assert_eq!(spans.len() - 1, intervals, "a span between each two points");
        let mut wide = Vec::new();
        for j in 0..intervals {
            let (u0, u1) = (spans[j], spans[j + 1]);
            let strays = [0.25, 0.5, 0.75].iter().any(|f| {
                let q = made.point_at(u0 + (u1 - u0) * f);
                (c.distance_to(q) - distance).abs() > precision
            });
            if strays {
                wide.push(j);
            }
        }
        if wide.is_empty() {
            return Ok(made);
        }
        if params.len() + wide.len() > MOST {
            return Err(GeometryError::OutOfRange { what: "offset" });
        }
        let next = |j: usize| params.get(j + 1).copied().unwrap_or(end);
        let added: Vec<(usize, f64)> = wide
            .iter()
            .map(|&j| (j, 0.5 * (params[j] + next(j))))
            .collect();
        for &(j, t) in added.iter().rev() {
            params.insert(j + 1, t);
        }
    }
}

/// 1 when `toward` is left of the curve where it is nearest, -1 right;
/// refused on the curve.
fn side_of(c: Smooth, toward: Point2, tol: Tolerance) -> GeometryResult<f64> {
    let t = c.closest_param(toward);
    let d = c.derivative(t);
    let across = d.cross(toward - c.point(t)) / d.length();
    if across.abs() <= tol.linear() {
        return Err(GeometryError::OutOfRange { what: "side" });
    }
    Ok(across.signum())
}

/// Its smooth spans' bounds; refused where a spline's tangent breaks at a
/// knot, or vanishes, or where a closed spline closes with a corner.
fn breaks_checked(c: Smooth) -> GeometryResult<Vec<f64>> {
    let breaks = c.breaks();
    let Smooth::Spline(s) = c else {
        return Ok(breaks);
    };
    let corner = |a: Vector2, b: Vector2| a.cross(b).atan2(a.dot(b)).abs() > CORNER;
    for w in breaks.windows(3) {
        let (left, right) = (s.between(w[0], w[1])?, s.between(w[1], w[2])?);
        if corner(end_tangent(&left)?, start_tangent(&right)?) {
            return Err(GeometryError::OutOfRange { what: "corner" });
        }
    }
    if s.is_closed() && !s.is_periodic() {
        let (a, b) = s.domain();
        let whole = s.between(a, b)?;
        if corner(end_tangent(&whole)?, start_tangent(&whole)?) {
            return Err(GeometryError::OutOfRange { what: "corner" });
        }
    }
    Ok(breaks)
}

/// The direction a clamped spline leaves its start by: toward its first
/// control point apart from the start.
fn start_tangent(s: &Spline) -> GeometryResult<Vector2> {
    let p = s.points();
    p.iter()
        .find(|q| **q != p[0])
        .map(|q| *q - p[0])
        .ok_or(GeometryError::OutOfRange { what: "corner" })
}

/// The direction a clamped spline arrives at its end by.
fn end_tangent(s: &Spline) -> GeometryResult<Vector2> {
    let p = s.points();
    let last = p[p.len() - 1];
    p.iter()
        .rev()
        .find(|q| **q != last)
        .map(|q| last - *q)
        .ok_or(GeometryError::OutOfRange { what: "corner" })
}

/// Refused where the offset by `shift` (left when positive) curls back on
/// itself: where the curve turns toward that side tighter than `shift`
/// away, or where it stops. Each span is sampled, and the tightest sample
/// sought on between its neighbors.
fn check_curl(c: Smooth, breaks: &[f64], shift: f64) -> GeometryResult<()> {
    let bend = |t: f64, span: (f64, f64)| shift * c.curvature(t, span);
    for w in breaks.windows(2) {
        let span = (w[0], w[1]);
        let at = |i: usize| w[0] + (w[1] - w[0]) * i as f64 / CHECKS as f64;
        let mut tightest = (0, f64::NEG_INFINITY);
        for i in 0..=CHECKS {
            let t = at(i);
            if c.derivative(t).length() <= 1e-12 * c.size() {
                return Err(GeometryError::OutOfRange { what: "offset" });
            }
            let b = bend(t, span);
            if b > tightest.1 {
                tightest = (i, b);
            }
        }
        let (lo, hi) = (
            at(tightest.0.saturating_sub(1)),
            at((tightest.0 + 1).min(CHECKS)),
        );
        let most = golden_max(|t| bend(t, span), lo, hi).max(tightest.1);
        if most >= 1.0 {
            return Err(GeometryError::OutOfRange { what: "offset" });
        }
    }
    Ok(())
}

/// The greatest value of `f` between `lo` and `hi`, by golden-section
/// search.
fn golden_max(f: impl Fn(f64) -> f64, mut lo: f64, mut hi: f64) -> f64 {
    let g = 0.5 * (5f64.sqrt() - 1.0);
    let (mut x1, mut x2) = (hi - g * (hi - lo), lo + g * (hi - lo));
    let (mut f1, mut f2) = (f(x1), f(x2));
    for _ in 0..60 {
        if f1 < f2 {
            lo = x1;
            x1 = x2;
            f1 = f2;
            x2 = lo + g * (hi - lo);
            f2 = f(x2);
        } else {
            hi = x2;
            x2 = x1;
            f2 = f1;
            x1 = hi - g * (hi - lo);
            f1 = f(x1);
        }
    }
    f1.max(f2)
}

#[cfg(test)]
#[path = "curve_offset_tests.rs"]
mod tests;
