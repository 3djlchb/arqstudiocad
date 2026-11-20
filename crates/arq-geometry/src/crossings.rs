//! Where the lines and circles curves lie on meet, unbounded: shared by
//! OFFSET (joining offset segments) and EXTEND (reaching a boundary).

use crate::ellipse::Ellipse;
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

/// Where the line through `p` along `d` meets the whole of ellipse `e`:
/// in the ellipse's own frame, where it is the unit circle, a quadratic;
/// none, one (touching) or two points.
pub(crate) fn line_ellipse(p: Point2, d: Vector2, e: Ellipse) -> Vec<Point2> {
    let (x0, y0) = e.local(p);
    let (dx, dy) = e.local_vector(d);
    let a = dx * dx + dy * dy;
    if a == 0.0 {
        return Vec::new();
    }
    let b = x0 * dx + y0 * dy;
    let c = x0 * x0 + y0 * y0 - 1.0;
    let disc = b * b - a * c;
    if disc < 0.0 {
        return Vec::new();
    }
    let h = disc.sqrt();
    [(-b - h) / a, (-b + h) / a]
        .into_iter()
        .filter_map(|s| Point2::new(p.x() + d.x() * s, p.y() + d.y() * s).ok())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_meets_a_whole_ellipse_twice_once_or_never() {
        // About (0,0), semi-axes 5 and 3, an arc of it: its whole counts.
        let e = Ellipse::new(
            Point2::ORIGIN,
            Vector2::new(5.0, 0.0).unwrap(),
            0.6,
            0.0,
            1.0,
        )
        .unwrap();
        let p = |x: f64, y: f64| Point2::new(x, y).unwrap();
        let x = Vector2::new(1.0, 0.0).unwrap();
        let found = line_ellipse(p(-20.0, -1.8), x, e);
        assert_eq!(found.len(), 2);
        assert!(found.iter().any(|q| q.distance_to(p(-4.0, -1.8)) < 1e-9));
        assert!(found.iter().any(|q| q.distance_to(p(4.0, -1.8)) < 1e-9));
        assert!(line_ellipse(p(-20.0, 3.5), x, e).is_empty(), "above it");
        assert!(
            line_ellipse(p(-20.0, 0.0), Vector2::ZERO, e).is_empty(),
            "no direction"
        );
    }
}
