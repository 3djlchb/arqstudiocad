//! Where the lines and circles curves lie on meet, unbounded: shared by
//! OFFSET (joining offset segments) and EXTEND (reaching a boundary).

use crate::point::Point2;
use crate::tolerance::Tolerance;
use crate::vector::Vector2;

/// Where the line through `p` along `d` meets the one through `q` along
/// `e`; None when they run alike.
pub(crate) fn line_line(p: Point2, d: Vector2, q: Point2, e: Vector2) -> Option<Point2> {
    let cross = d.cross(e);
    if cross.abs() <= 1e-12 * d.length() * e.length() {
        return None;
    }
    let t = (q - p).cross(e) / cross;
    Point2::new(p.x() + d.x() * t, p.y() + d.y() * t).ok()
}

/// Where the line through `p` along `d` meets the circle about `c` of
/// radius `r`: none, one (touching) or two points.
pub(crate) fn line_circle(p: Point2, d: Vector2, c: Point2, r: f64) -> Vec<Point2> {
    let u = match d.normalized() {
        Ok(u) => u,
        Err(_) => return Vec::new(),
    };
    let t0 = (c - p).dot(u);
    let foot = Point2::new(p.x() + u.x() * t0, p.y() + u.y() * t0).unwrap_or(p);
    let h2 = r * r - foot.distance_to(c).powi(2);
    if h2 < 0.0 {
        return Vec::new();
    }
    let h = h2.sqrt();
    [-h, h]
        .into_iter()
        .filter_map(|s| Point2::new(foot.x() + u.x() * s, foot.y() + u.y() * s).ok())
        .collect()
}

/// Where two circles meet: none, one or two points.
pub(crate) fn circle_circle(
    c1: Point2,
    r1: f64,
    c2: Point2,
    r2: f64,
    tol: Tolerance,
) -> Vec<Point2> {
    let d = c1.distance_to(c2);
    if d <= tol.linear() || d > r1 + r2 || d < (r1 - r2).abs() {
        return Vec::new();
    }
    let a = (r1 * r1 - r2 * r2 + d * d) / (2.0 * d);
    let h = (r1 * r1 - a * a).max(0.0).sqrt();
    let u = (c2 - c1) / d;
    let base = Point2::new(c1.x() + u.x() * a, c1.y() + u.y() * a).unwrap_or(c1);
    let n = u.perp();
    [-h, h]
        .into_iter()
        .filter_map(|s| Point2::new(base.x() + n.x() * s, base.y() + n.y() * s).ok())
        .collect()
}
