//! The splines SPLINE makes (ADR 0042 D4): through fit points, by global
//! cubic interpolation, and by control vertices.
//!
//! Through fit points, as AutoCAD and BricsCAD are known to make them
//! (ezdxf's `fit_points_to_cad_cv`, MIT, after Piegl and Tiller's *The
//! NURBS Book*, §9.2; THIRD_PARTY.md):
//!
//! - always cubic, its inner knots the fit points' own parameters;
//! - each open end either along a given tangent, its length the fit
//!   points' total chord length per parameter unit, or with no curvature;
//! - closed: periodic, smooth where it closes.
//!
//! The parameters are the fit points' running chord lengths, their square
//! roots, or one a point (Knots: Chord, Square root, Uniform). The same fit
//! points give the same spline to the last bit, so a spline stored by its
//! fit points alone, as DWG keeps one, draws the same when read again.

use crate::error::{GeometryError, GeometryResult};
use crate::point::Point2;
use crate::spline::Spline;
use crate::vector::Vector2;

/// How the parameters of a spline's fit points are spaced: DWG's knot
/// parameterization.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SplineKnots {
    /// By the distance between fit points (DWG 0).
    #[default]
    Chord,
    /// By the square root of that distance (DWG 1).
    SquareRoot,
    /// One a fit point (DWG 2).
    Uniform,
}

impl SplineKnots {
    /// DWG's code for it.
    pub fn code(self) -> i32 {
        match self {
            Self::Chord => 0,
            Self::SquareRoot => 1,
            Self::Uniform => 2,
        }
    }

    /// The one DWG's `code` names; Chord for any other code, as for one
    /// not stated.
    pub fn from_code(code: i32) -> Self {
        match code {
            1 => Self::SquareRoot,
            2 => Self::Uniform,
            _ => Self::Chord,
        }
    }

    fn step(self, from: Point2, to: Point2) -> f64 {
        match self {
            Self::Chord => from.distance_to(to),
            Self::SquareRoot => from.distance_to(to).sqrt(),
            Self::Uniform => 1.0,
        }
    }
}

/// What a spline through fit points is made of: the points, how their
/// knots are spaced, the tangents at its ends, if given, and whether it
/// closes. Tangents are directions; their length is not used.
#[derive(Clone, Debug, PartialEq)]
pub struct SplineFit {
    points: Vec<Point2>,
    knots: SplineKnots,
    start_tangent: Option<Vector2>,
    end_tangent: Option<Vector2>,
    closed: bool,
}

impl SplineFit {
    /// Fit `points`, at least two, or three when `closed`, none where the
    /// one before is (nor the last on the first when closed; a last point on
    /// the first is dropped). A closed spline takes no tangent; a tangent
    /// must have a direction.
    pub fn new(
        points: Vec<Point2>,
        knots: SplineKnots,
        start_tangent: Option<Vector2>,
        end_tangent: Option<Vector2>,
        closed: bool,
    ) -> GeometryResult<Self> {
        let mut points = points;
        if closed && points.len() > 3 && points.first() == points.last() {
            points.pop();
        }
        let fewest = if closed { 3 } else { 2 };
        let repeated =
            points.windows(2).any(|w| w[0] == w[1]) || (closed && points.first() == points.last());
        if points.len() < fewest || repeated {
            return Err(GeometryError::OutOfRange {
                what: "spline fit points",
            });
        }
        let direction = |t: Option<Vector2>| match t {
            Some(_) if closed => Err(GeometryError::OutOfRange {
                what: "spline tangent",
            }),
            Some(t) => unit(t).map(Some),
            None => Ok(None),
        };
        Ok(Self {
            start_tangent: direction(start_tangent)?,
            end_tangent: direction(end_tangent)?,
            points,
            knots,
            closed,
        })
    }

    /// Its fit points.
    pub fn points(&self) -> &[Point2] {
        &self.points
    }

    /// How its knots are spaced.
    pub fn knots(&self) -> SplineKnots {
        self.knots
    }

    /// The direction at its start, if given.
    pub fn start_tangent(&self) -> Option<Vector2> {
        self.start_tangent
    }

    /// The direction at its end, if given.
    pub fn end_tangent(&self) -> Option<Vector2> {
        self.end_tangent
    }

    /// Whether it closes.
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// The cubic spline through its points. Refused only when the points
    /// leave the equations without a single solution.
    pub fn spline(&self) -> GeometryResult<Spline> {
        let (points, knots) = self.control()?;
        Spline::new(3, points, Vec::new(), knots, self.closed)
    }

    /// The control points and knots of [`SplineFit::spline`], of degree
    /// three and not rational, without building it.
    pub fn control(&self) -> GeometryResult<(Vec<Point2>, Vec<f64>)> {
        if self.closed {
            self.closed_control()
        } else {
            self.open_control()
        }
    }

    /// The running parameters of the points, from zero, and back to the
    /// first when closed.
    fn parameters(&self) -> Vec<f64> {
        let mut t = Vec::with_capacity(self.points.len() + 1);
        let mut s = 0.0;
        t.push(s);
        let back = self.closed.then(|| self.points[0]);
        for (a, b) in self
            .points
            .windows(2)
            .map(|w| (w[0], w[1]))
            .chain(back.map(|first| (self.points[self.points.len() - 1], first)))
        {
            s += self.knots.step(a, b);
            t.push(s);
        }
        t
    }

    /// The fit points' total chord length.
    fn chord_length(&self) -> f64 {
        self.points.windows(2).map(|w| w[0].distance_to(w[1])).sum()
    }

    /// Open: `n + 3` control points for `n + 1` fit points, the knots the
    /// parameters with the ends repeated four times. The system is
    /// tridiagonal: each fit point, then each end's condition next to it.
    fn open_control(&self) -> GeometryResult<(Vec<Point2>, Vec<f64>)> {
        const P: usize = 3;
        let t = self.parameters();
        let n = self.points.len() - 1;
        let (t0, tn) = (t[0], t[n]);
        let mut knots = vec![t0; P + 1];
        knots.extend_from_slice(&t[1..n]);
        knots.extend(std::iter::repeat_n(tn, P + 1));
        let count = n + 3;
        let mut sys = Tridiagonal::new(count);
        let at = |p: Point2| [p.x(), p.y(), 0.0];
        sys.row(0, [0.0, 1.0, 0.0], at(self.points[0]));
        sys.row(count - 1, [0.0, 1.0, 0.0], at(self.points[n]));
        for k in 1..n {
            sys.row(k + 1, basis_at_knot(&knots, P + k), at(self.points[k]));
        }
        // A tangent's length per parameter unit: the chord length over the
        // parameters' run, so that it is ezdxf's with parameters in [0, 1].
        let speed = self.chord_length() / (tn - t0);
        let pf = P as f64;
        match self.start_tangent {
            Some(d) => {
                let v = d * (speed * (knots[P + 1] - t0) / pf);
                sys.row(1, [-1.0, 1.0, 0.0], [v.x(), v.y(), 0.0]);
            }
            None => {
                // C''(t0) = 0, The NURBS Book (3.9).
                let (u1, u2) = (knots[P + 1] - t0, knots[P + 2] - t0);
                let f = pf * (pf - 1.0) / u1;
                sys.row(1, [f / u1, -f * (u1 + u2) / (u1 * u2), f / u2], [0.0; 3]);
            }
        }
        let m = knots.len() - 1;
        match self.end_tangent {
            Some(d) => {
                let v = d * (speed * (tn - knots[m - P - 1]) / pf);
                sys.row(count - 2, [0.0, -1.0, 1.0], [v.x(), v.y(), 0.0]);
            }
            None => {
                // C''(tn) = 0, The NURBS Book (3.10).
                let (u1, u2) = (tn - knots[m - P - 1], tn - knots[m - P - 2]);
                let f = pf * (pf - 1.0) / u1;
                sys.row(
                    count - 2,
                    [f / u2, -f * (u1 + u2) / (u1 * u2), f / u1],
                    [0.0; 3],
                );
            }
        }
        let points = sys
            .solve()?
            .into_iter()
            .map(point)
            .collect::<GeometryResult<_>>()?;
        Ok((points, knots))
    }

    /// Closed: periodic, its `m` control points wrapped by three, its knots
    /// the parameters running on past both ends. Each fit point holds three
    /// control points in turn, a cyclic tridiagonal system.
    fn closed_control(&self) -> GeometryResult<(Vec<Point2>, Vec<f64>)> {
        const P: usize = 3;
        let t = self.parameters();
        let m = self.points.len();
        let period = t[m];
        // Knot j is parameter j - 3, wrapped by the period.
        let knots: Vec<f64> = (0..m + 2 * P + 1)
            .map(|j| {
                let k = j as isize - P as isize;
                let wraps = k.div_euclid(m as isize);
                t[k.rem_euclid(m as isize) as usize] + wraps as f64 * period
            })
            .collect();
        // Fit point i is held by control points i, i + 1, i + 2 (mod m):
        // unknown r_i is control point i + 1, so row i is r_(i-1), r_i, r_(i+1).
        let rows: Vec<[f64; 3]> = (0..m).map(|i| basis_at_knot(&knots, P + i)).collect();
        let r = solve_cyclic(&rows, &self.points)?;
        let mut points: Vec<Point2> = (0..m).map(|j| r[(j + m - 1) % m]).collect();
        points.extend_from_within(..P);
        Ok((points, knots))
    }
}

impl Spline {
    /// The spline of `degree` by control `points` (SPLINE's CV method),
    /// its knots uniform. Open: clamped to its first and last point, its
    /// degree lowered to one fewer than the points. Closed: periodic, the
    /// first `degree` points wrapped, from three points.
    pub fn by_vertices(degree: usize, points: Vec<Point2>, closed: bool) -> GeometryResult<Self> {
        let n = points.len();
        let fewest = if closed { 3 } else { 2 };
        if n < fewest || degree == 0 {
            return Err(GeometryError::OutOfRange {
                what: "spline degree",
            });
        }
        let p = degree.min(n - 1);
        if !closed {
            return Spline::new(p, points, Vec::new(), Spline::clamped_knots(p, n), false);
        }
        let mut points = points;
        points.extend_from_within(..p);
        let knots = (0..n + 2 * p + 1).map(|j| j as f64 - p as f64).collect();
        Spline::new(p, points, Vec::new(), knots, true)
    }
}

/// `t`'s direction: `t` itself when it is already of unit length to a few
/// bits, so a tangent read back as written, normalized, gives the same
/// spline on every platform (`hypot` is not correctly rounded on all).
fn unit(t: Vector2) -> GeometryResult<Vector2> {
    if (t.length() - 1.0).abs() <= 4.0 * f64::EPSILON {
        Ok(t)
    } else {
        t.normalized()
    }
}

fn point(c: [f64; 3]) -> GeometryResult<Point2> {
    Point2::new(c[0], c[1])
}

/// The three cubic basis functions not zero at knot `span`, the start of
/// that span: those of control points `span - 3` to `span - 1` (The NURBS
/// Book, A2.2; the fourth is zero there).
fn basis_at_knot(knots: &[f64], span: usize) -> [f64; 3] {
    const P: usize = 3;
    let u = knots[span];
    let mut n = [0.0f64; P + 1];
    let (mut left, mut right) = ([0.0f64; P + 1], [0.0f64; P + 1]);
    n[0] = 1.0;
    for j in 1..=P {
        left[j] = u - knots[span + 1 - j];
        right[j] = knots[span + j] - u;
        let mut saved = 0.0;
        for r in 0..j {
            let denom = right[r + 1] + left[j - r];
            let temp = if denom == 0.0 { 0.0 } else { n[r] / denom };
            n[r] = saved + right[r + 1] * temp;
            saved = left[j - r] * temp;
        }
        n[j] = saved;
    }
    [n[0], n[1], n[2]]
}

/// A tridiagonal system with three right-hand columns, solved by Gaussian
/// elimination with partial pivoting (LAPACK's `dgtsv`).
struct Tridiagonal {
    lower: Vec<f64>,
    diag: Vec<f64>,
    upper: Vec<f64>,
    rhs: Vec<[f64; 3]>,
}

impl Tridiagonal {
    fn new(n: usize) -> Self {
        Self {
            lower: vec![0.0; n],
            diag: vec![0.0; n],
            upper: vec![0.0; n],
            rhs: vec![[0.0; 3]; n],
        }
    }

    /// Row `i`: the coefficients of unknowns `i - 1`, `i` and `i + 1`.
    fn row(&mut self, i: usize, c: [f64; 3], rhs: [f64; 3]) {
        if i > 0 {
            self.lower[i - 1] = c[0];
        }
        self.diag[i] = c[1];
        self.upper[i] = c[2];
        self.rhs[i] = rhs;
    }

    fn solve(self) -> GeometryResult<Vec<[f64; 3]>> {
        let Self {
            mut lower,
            mut diag,
            mut upper,
            mut rhs,
        } = self;
        let n = diag.len();
        let singular = || GeometryError::OutOfRange {
            what: "spline fit points",
        };
        // `lower[i]` holds the second superdiagonal once row i is reduced.
        for i in 0..n.saturating_sub(1) {
            if diag[i].abs() >= lower[i].abs() {
                if diag[i] == 0.0 {
                    return Err(singular());
                }
                let f = lower[i] / diag[i];
                diag[i + 1] -= f * upper[i];
                let top = rhs[i];
                for (b, a) in rhs[i + 1].iter_mut().zip(top) {
                    *b -= f * a;
                }
                lower[i] = 0.0;
            } else {
                let f = diag[i] / lower[i];
                diag[i] = lower[i];
                let below = diag[i + 1];
                diag[i + 1] = upper[i] - f * below;
                if i + 2 < n {
                    lower[i] = upper[i + 1];
                    upper[i + 1] = -f * lower[i];
                } else {
                    lower[i] = 0.0;
                }
                upper[i] = below;
                let (top, next) = (rhs[i], rhs[i + 1]);
                rhs[i] = next;
                rhs[i + 1] = std::array::from_fn(|c| top[c] - f * next[c]);
            }
        }
        if diag[n - 1] == 0.0 {
            return Err(singular());
        }
        for i in (0..n).rev() {
            let mut b = rhs[i];
            for c in 0..3 {
                if i + 1 < n {
                    b[c] -= upper[i] * rhs[i + 1][c];
                }
                if i + 2 < n {
                    b[c] -= lower[i] * rhs[i + 2][c];
                }
                b[c] /= diag[i];
            }
            if !b.iter().all(|v| v.is_finite()) {
                return Err(singular());
            }
            rhs[i] = b;
        }
        Ok(rhs)
    }
}

/// `rows[i]` · (x[i-1], x[i], x[i+1]) = `points[i]`, indices cyclic: by
/// Sherman and Morrison over a tridiagonal system, its correction carried
/// as the third right-hand column.
fn solve_cyclic(rows: &[[f64; 3]], points: &[Point2]) -> GeometryResult<Vec<Point2>> {
    let m = rows.len();
    let (beta, alpha) = (rows[0][0], rows[m - 1][2]);
    let gamma = -rows[0][1];
    let mut sys = Tridiagonal::new(m);
    for (i, (r, q)) in rows.iter().zip(points).enumerate() {
        let mut c = *r;
        let mut u = 0.0;
        if i == 0 {
            c[1] -= gamma;
            u = gamma;
        }
        if i == m - 1 {
            c[1] -= alpha * beta / gamma;
            u = alpha;
        }
        sys.row(i, c, [q.x(), q.y(), u]);
    }
    let x = sys.solve()?;
    let (z0, zl) = (x[0][2], x[m - 1][2]);
    let denom = 1.0 + z0 + beta * zl / gamma;
    if denom == 0.0 {
        return Err(GeometryError::OutOfRange {
            what: "spline fit points",
        });
    }
    x.iter()
        .map(|v| {
            let f = |c: usize| (x[0][c] + beta * x[m - 1][c] / gamma) / denom;
            Point2::new(v[0] - f(0) * v[2], v[1] - f(1) * v[2])
        })
        .collect()
}

#[cfg(test)]
#[path = "spline_fit_tests.rs"]
mod tests;
