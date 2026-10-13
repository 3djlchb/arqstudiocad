//! `Camera`: which part of the plan the view shows, in `f64`.
//!
//! Screen pixels have their origin at the top-left corner and grow down;
//! world Y grows up. Everything is kept in `f64` and converted to `f32` only
//! for screen coordinates near the view, so survey coordinates (~1e6) keep
//! their precision at any zoom.

use arq_geometry::{Bounds2, Point2};

/// Zoom limits, in pixels per model unit.
const MIN_SCALE: f64 = 1e-9;
const MAX_SCALE: f64 = 1e9;

/// The view: a center in world coordinates, a scale and a size in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    center: (f64, f64),
    scale: f64,
    width: u32,
    height: u32,
}

impl Camera {
    /// A `width` × `height` view of the origin, one pixel per unit.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            center: (0.0, 0.0),
            scale: 1.0,
            width: width.max(1),
            height: height.max(1),
        }
    }

    /// The size in pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// The world point at the middle of the view.
    pub fn center(&self) -> (f64, f64) {
        self.center
    }

    /// Pixels per model unit.
    pub fn scale(&self) -> f64 {
        self.scale
    }

    /// A new size, keeping the center and scale.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width.max(1);
        self.height = height.max(1);
    }

    /// World to screen pixels.
    pub fn to_screen(&self, x: f64, y: f64) -> (f64, f64) {
        (
            (x - self.center.0) * self.scale + f64::from(self.width) / 2.0,
            f64::from(self.height) / 2.0 - (y - self.center.1) * self.scale,
        )
    }

    /// Screen pixels to world.
    pub fn to_world(&self, sx: f64, sy: f64) -> (f64, f64) {
        (
            self.center.0 + (sx - f64::from(self.width) / 2.0) / self.scale,
            self.center.1 - (sy - f64::from(self.height) / 2.0) / self.scale,
        )
    }

    /// The part of the world the view shows.
    pub fn visible(&self) -> Option<Bounds2> {
        let (x0, y0) = self.to_world(0.0, f64::from(self.height));
        let (x1, y1) = self.to_world(f64::from(self.width), 0.0);
        Some(Bounds2::new(
            Point2::new(x0, y0).ok()?,
            Point2::new(x1, y1).ok()?,
        ))
    }

    /// Zooms by `factor` (above 1 closer) keeping the world point under the
    /// screen point `(sx, sy)` where it is: zooming with the wheel.
    pub fn zoom_at(&mut self, sx: f64, sy: f64, factor: f64) {
        if !(factor.is_finite() && factor > 0.0) {
            return;
        }
        let (wx, wy) = self.to_world(sx, sy);
        self.scale = (self.scale * factor).clamp(MIN_SCALE, MAX_SCALE);
        self.center = (
            wx - (sx - f64::from(self.width) / 2.0) / self.scale,
            wy + (sy - f64::from(self.height) / 2.0) / self.scale,
        );
    }

    /// Moves the view by a drag of `(dx, dy)` pixels: the drawing follows
    /// the pointer.
    pub fn pan(&mut self, dx: f64, dy: f64) {
        if dx.is_finite() && dy.is_finite() {
            self.center = (
                self.center.0 - dx / self.scale,
                self.center.1 + dy / self.scale,
            );
        }
    }

    /// Shows all of `bounds` with a small margin (zoom extents). A point or
    /// a flat box keeps the scale where the other side gives none.
    pub fn fit(&mut self, bounds: Bounds2) {
        const MARGIN: f64 = 0.05;
        let c = bounds.center();
        self.center = (c.x(), c.y());
        let usable = 1.0 - 2.0 * MARGIN;
        let sx = f64::from(self.width) * usable / bounds.width();
        let sy = f64::from(self.height) * usable / bounds.height();
        let scale = match (sx.is_finite(), sy.is_finite()) {
            (true, true) => sx.min(sy),
            (true, false) => sx,
            (false, true) => sy,
            (false, false) => self.scale,
        };
        self.scale = scale.clamp(MIN_SCALE, MAX_SCALE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    #[test]
    fn screen_and_world_are_inverse_and_y_points_up() {
        let mut cam = Camera::new(200, 100);
        cam.fit(Bounds2::new(
            p(500_000.0, 9_977_000.0),
            p(500_010.0, 9_977_010.0),
        ));
        // 10 units into 90 usable pixels of height: 9 px per unit.
        assert_eq!(cam.scale(), 9.0);
        assert_eq!(cam.to_screen(500_005.0, 9_977_005.0), (100.0, 50.0));
        assert_eq!(
            cam.to_screen(500_005.0, 9_977_006.0),
            (100.0, 41.0),
            "up is up"
        );
        let (x, y) = cam.to_world(37.0, 81.0);
        // At survey magnitudes (~1e7) f64 round trips within ~1e-8 pixel.
        let (sx, sy) = cam.to_screen(x, y);
        assert!(
            (sx - 37.0).abs() < 1e-6 && (sy - 81.0).abs() < 1e-6,
            "{sx} {sy}"
        );
    }

    #[test]
    fn zooming_keeps_the_point_under_the_cursor() {
        let mut cam = Camera::new(400, 300);
        let before = cam.to_world(100.0, 50.0);
        cam.zoom_at(100.0, 50.0, 2.0);
        assert_eq!(cam.scale(), 2.0);
        assert_eq!(cam.to_world(100.0, 50.0), before);
        cam.zoom_at(0.0, 0.0, f64::NAN);
        assert_eq!(cam.scale(), 2.0, "invalid factors are ignored");
    }

    #[test]
    fn panning_follows_the_pointer() {
        let mut cam = Camera::new(100, 100);
        cam.zoom_at(50.0, 50.0, 4.0);
        let grabbed = cam.to_world(10.0, 10.0);
        cam.pan(30.0, -20.0);
        assert_eq!(cam.to_world(40.0, -10.0), grabbed);
    }

    #[test]
    fn fitting_flat_and_single_point_extents() {
        let mut cam = Camera::new(100, 50);
        cam.fit(Bounds2::new(p(0.0, 0.0), p(90.0, 0.0)));
        assert_eq!((cam.scale(), cam.center()), (1.0, (45.0, 0.0)));
        cam.fit(Bounds2::from_point(p(7.0, 8.0)));
        assert_eq!((cam.scale(), cam.center()), (1.0, (7.0, 8.0)), "scale kept");
        let v = cam.visible().unwrap();
        assert_eq!((v.min(), v.max()), (p(-43.0, -17.0), p(57.0, 33.0)));
    }
}
