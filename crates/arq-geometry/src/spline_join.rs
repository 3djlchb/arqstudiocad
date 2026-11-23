//! Splines joined end to end (ADR 0043 D4, JOIN): a spline turned round,
//! brought to a higher degree, and the splines touching a source's ends
//! chained onto it.

use crate::error::{GeometryError, GeometryResult};
use crate::point::Point2;
use crate::spline::{MAX_DEGREE, Spline};
use crate::spline_split::join;
use crate::tolerance::Tolerance;

impl Spline {
    /// The same curve run the other way: its points, weights and knots in
    /// reverse, its knots mirrored within its domain.
    pub fn reversed(&self) -> Self {
        let (a, b) = self.domain();
        let points = self.points().iter().rev().copied().collect();
        let weights = self.weights().iter().rev().copied().collect();
        let knots = self.knots().iter().rev().map(|k| a + b - k).collect();
        Self::new(self.degree(), points, weights, knots, self.is_closed())
            .expect("a spline's own data reversed")
    }

    /// The same curve at `degree`, at least its own, exactly (degree
    /// elevation, The NURBS Book A5.9, in homogeneous coordinates): open,
    /// its knots clamped at both ends, each inner knot as many times more
    /// as the degree rose, so as smooth as before.
    pub fn elevated(&self, degree: usize) -> GeometryResult<Self> {
        if degree < self.degree() || degree > MAX_DEGREE {
            return Err(GeometryError::OutOfRange {
                what: "spline degree",
            });
        }
        let (a, b) = self.domain();
        let clamped = self.between(a, b)?;
        if degree == clamped.degree() {
            return Ok(clamped);
        }
        elevate(&clamped, degree - clamped.degree())
    }
}

/// A homogeneous control point, `(x w, y w, w)`.
type H = [f64; 3];

fn blend(a: f64, p: H, b: f64, q: H) -> H {
    std::array::from_fn(|c| a * p[c] + b * q[c])
}

fn binomial(n: usize, k: usize) -> f64 {
    (0..k).fold(1.0, |acc, i| acc * (n - i) as f64 / (i + 1) as f64)
}

/// Clamped spline `s` raised `t` degrees: A5.9, which takes each Bézier
/// piece in turn, raises it, and removes the knots it inserted. Indexed
/// as the book writes it, so it can be checked against it.
#[allow(clippy::needless_range_loop)]
fn elevate(s: &Spline, t: usize) -> GeometryResult<Spline> {
    let p = s.degree();
    let u = s.knots();
    let pw: Vec<H> = s
        .points()
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let w = s.weight_of(i);
            [c.x() * w, c.y() * w, w]
        })
        .collect();
    let m = u.len() - 1;
    let ph = p + t;
    let ph2 = ph / 2;
    // The coefficients raising a Bézier piece of degree p by t.
    let mut bezalfs = vec![vec![0.0; p + 1]; ph + 1];
    bezalfs[0][0] = 1.0;
    bezalfs[ph][p] = 1.0;
    for i in 1..=ph2 {
        let inv = 1.0 / binomial(ph, i);
        for j in i.saturating_sub(t)..=p.min(i) {
            bezalfs[i][j] = inv * binomial(p, j) * binomial(t, i - j);
        }
    }
    for i in ph2 + 1..ph {
        for j in i.saturating_sub(t)..=p.min(i) {
            bezalfs[i][j] = bezalfs[ph - i][p - j];
        }
    }
    let zero = [0.0; 3];
    let mut bpts: Vec<H> = pw[..=p].to_vec();
    let mut next_bpts = vec![zero; p.max(1)];
    let mut ebpts = vec![zero; ph + 1];
    let mut alfs = vec![0.0; p.max(1)];
    let mut qw: Vec<H> = vec![pw[0]];
    let mut uh: Vec<f64> = vec![u[0]; ph + 1];
    let (mut a, mut b) = (p, p + 1);
    let mut r: isize = -1;
    let mut ua = u[0];
    while b < m {
        let i0 = b;
        while b < m && u[b] == u[b + 1] {
            b += 1;
        }
        let mul = b - i0 + 1;
        let ub = u[b];
        let oldr = r;
        r = p as isize - mul as isize;
        let lbz = if oldr > 0 { (oldr as usize + 2) / 2 } else { 1 };
        let rbz = if r > 0 {
            ph - (r as usize).div_ceil(2)
        } else {
            ph
        };
        if r > 0 {
            // Inserts `ub` r times, ending this Bézier piece.
            let r = r as usize;
            let numer = ub - ua;
            for k in (mul + 1..=p).rev() {
                alfs[k - mul - 1] = numer / (u[a + k] - ua);
            }
            for j in 1..=r {
                let save = r - j;
                let s = mul + j;
                for k in (s..=p).rev() {
                    let al = alfs[k - s];
                    bpts[k] = blend(al, bpts[k], 1.0 - al, bpts[k - 1]);
                }
                next_bpts[save] = bpts[p];
            }
        }
        // Raises the piece.
        for (i, slot) in ebpts.iter_mut().enumerate().skip(lbz) {
            *slot = zero;
            for j in i.saturating_sub(t)..=p.min(i) {
                *slot = blend(1.0, *slot, bezalfs[i][j], bpts[j]);
            }
        }
        if oldr > 1 {
            // Removes `ua`, inserted oldr times, as often as it can go.
            let oldr = oldr as usize;
            let kind = uh.len();
            let den = ub - ua;
            let bet = (ub - uh[kind - 1]) / den;
            for tr in 1..oldr {
                // The book's `first` and `last`, one further out each time.
                let (mut i, mut j) = (kind - 1 - tr, kind - 1 + tr);
                let mut kj = j - kind + 1;
                while j - i > tr {
                    let cind = qw.len();
                    if i < cind {
                        let alf = (ub - uh[i]) / (ua - uh[i]);
                        qw[i] = blend(alf, qw[i], 1.0 - alf, qw[i - 1]);
                    }
                    if j >= lbz {
                        if j - tr <= kind - ph + oldr {
                            let gam = (ub - uh[j - tr]) / den;
                            ebpts[kj] = blend(gam, ebpts[kj], 1.0 - gam, ebpts[kj + 1]);
                        } else {
                            ebpts[kj] = blend(bet, ebpts[kj], 1.0 - bet, ebpts[kj + 1]);
                        }
                    }
                    i += 1;
                    j -= 1;
                    kj = kj.wrapping_sub(1);
                }
            }
        }
        if a != p {
            let times = ph - oldr.max(0) as usize;
            uh.extend(std::iter::repeat_n(ua, times));
        }
        qw.extend_from_slice(&ebpts[lbz..=rbz]);
        if b < m {
            let r = r.max(0) as usize;
            bpts[..r].copy_from_slice(&next_bpts[..r]);
            bpts[r..=p].copy_from_slice(&pw[b - p + r..=b]);
            a = b;
            b += 1;
            ua = ub;
        } else {
            uh.extend(std::iter::repeat_n(ub, ph + 1));
        }
    }
    let rational = qw.iter().any(|h| (h[2] - 1.0).abs() > 1e-12);
    let points = qw
        .iter()
        .map(|h| Point2::new(h[0] / h[2], h[1] / h[2]))
        .collect::<GeometryResult<Vec<_>>>()?;
    let weights = if rational {
        qw.iter().map(|h| h[2]).collect()
    } else {
        Vec::new()
    };
    Spline::new(ph, points, weights, uh, false)
}

/// The open spline `source` joined with the open splines of `others`
/// whose ends touch its own, end to end, as JOIN chains them: each time
/// the earliest that touches, at its end or before its start, turned round
/// when it runs the other way, until none does; a branch beyond the first
/// at a point is left out. Every piece is brought to the highest degree
/// among them and their knots run on one after the other; where two meet,
/// the later one's first control point gives way to the earlier one's
/// last. A chain that comes back to its start closes, its last control
/// point made its first. Also the indices of `others` taken, in the order
/// taken; None when none touches, or the source is closed.
pub fn join_splines(
    source: &Spline,
    others: &[Spline],
    tol: Tolerance,
) -> GeometryResult<Option<(Spline, Vec<usize>)>> {
    if source.is_closed() {
        return Ok(None);
    }
    let touch = |a: Point2, b: Point2| a.distance_to(b) <= tol.linear();
    let ends: Vec<Option<(Point2, Point2)>> = others
        .iter()
        .map(|s| (!s.is_closed()).then(|| s.ends()))
        .collect();
    // The chain as pieces in order, each an index (None for the source)
    // and whether it runs the other way.
    let mut chain: std::collections::VecDeque<(Option<usize>, bool)> =
        std::collections::VecDeque::from([(None, false)]);
    let (mut start, mut end) = source.ends();
    let mut taken = Vec::new();
    loop {
        let mut grew = false;
        for (i, e) in ends.iter().enumerate() {
            let Some((a, b)) = *e else { continue };
            if taken.contains(&i) || !taken.is_empty() && touch(start, end) {
                continue;
            }
            if touch(end, a) {
                chain.push_back((Some(i), false));
                end = b;
            } else if touch(end, b) {
                chain.push_back((Some(i), true));
                end = a;
            } else if touch(start, b) {
                chain.push_front((Some(i), false));
                start = a;
            } else if touch(start, a) {
                chain.push_front((Some(i), true));
                start = b;
            } else {
                continue;
            }
            taken.push(i);
            grew = true;
        }
        if !grew {
            break;
        }
    }
    if taken.is_empty() {
        return Ok(None);
    }
    let pieces: Vec<&Spline> = chain
        .iter()
        .map(|&(i, _)| i.map_or(source, |i| &others[i]))
        .collect();
    let degree = pieces.iter().map(|s| s.degree()).max().expect("a source");
    let mut joined: Option<Spline> = None;
    for (piece, &(_, back)) in pieces.iter().zip(&chain) {
        let piece = if back {
            piece.reversed()
        } else {
            (*piece).clone()
        };
        let piece = piece.elevated(degree)?;
        joined = Some(match joined {
            None => piece,
            Some(before) => {
                let shift = before.domain().1 - piece.domain().0;
                join(&before, &piece, shift)?
            }
        });
    }
    let joined = joined.expect("a source");
    let closes = touch(start, end) && pieces.len() >= 2;
    let joined = if closes { close(&joined)? } else { joined };
    Ok(Some((joined, taken)))
}

/// Clamped open spline `s`, its ends touching, closed: its last control
/// point, where it ends, made its first, so the curve closes exactly.
fn close(s: &Spline) -> GeometryResult<Spline> {
    let mut points = s.points().to_vec();
    let last = points.len() - 1;
    points[last] = points[0];
    Spline::new(
        s.degree(),
        points,
        s.weights().to_vec(),
        s.knots().to_vec(),
        true,
    )
}

#[cfg(test)]
#[path = "spline_join_tests.rs"]
mod tests;
