//! `View`: which way the model is seen (ADR 0030), AutoCAD's preset views.
//!
//! A view projects in parallel onto the plane facing its direction, with
//! Z up on screen except from above and below. The projected coordinates
//! play the part the plan's X and Y play in plan, so the camera, zoom and
//! pan work alike in every view.

use arq_geometry::Point3;

/// One of AutoCAD's preset views. `Top` is the plan.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum View {
    /// The plan, seen from above.
    #[default]
    Top,
    /// Seen from below.
    Bottom,
    /// Seen from the front, looking north (+Y).
    Front,
    /// Seen from the back, looking south.
    Back,
    /// Seen from the left, looking east (+X).
    Left,
    /// Seen from the right, looking west.
    Right,
    /// From the southwest, above: AutoCAD's SW Isometric.
    SwIso,
    /// From the southeast, above.
    SeIso,
    /// From the northeast, above.
    NeIso,
    /// From the northwest, above.
    NwIso,
}

impl View {
    /// Every view, in AutoCAD's order.
    pub const ALL: [View; 10] = [
        View::Top,
        View::Bottom,
        View::Front,
        View::Back,
        View::Left,
        View::Right,
        View::SwIso,
        View::SeIso,
        View::NeIso,
        View::NwIso,
    ];

    /// Its name, as `-VIEW` takes it, in lowercase.
    pub fn name(self) -> &'static str {
        match self {
            View::Top => "top",
            View::Bottom => "bottom",
            View::Front => "front",
            View::Back => "back",
            View::Left => "left",
            View::Right => "right",
            View::SwIso => "swiso",
            View::SeIso => "seiso",
            View::NeIso => "neiso",
            View::NwIso => "nwiso",
        }
    }

    /// The view of `name`, ignoring case.
    pub fn from_name(name: &str) -> Option<View> {
        View::ALL
            .into_iter()
            .find(|v| v.name().eq_ignore_ascii_case(name))
    }

    /// From the model toward the viewer, as AutoCAD's VIEWDIR.
    pub fn direction(self) -> [f64; 3] {
        match self {
            View::Top => [0.0, 0.0, 1.0],
            View::Bottom => [0.0, 0.0, -1.0],
            View::Front => [0.0, -1.0, 0.0],
            View::Back => [0.0, 1.0, 0.0],
            View::Left => [-1.0, 0.0, 0.0],
            View::Right => [1.0, 0.0, 0.0],
            View::SwIso => [-1.0, -1.0, 1.0],
            View::SeIso => [1.0, -1.0, 1.0],
            View::NeIso => [1.0, 1.0, 1.0],
            View::NwIso => [-1.0, 1.0, 1.0],
        }
    }

    /// The world directions of the screen's right and up: up is Z (Y
    /// from above and below) turned square to the line of sight, right is
    /// up crossed with the direction toward the viewer.
    pub fn axes(self) -> ([f64; 3], [f64; 3]) {
        let d = unit(self.direction());
        let hint = if d[0] == 0.0 && d[1] == 0.0 {
            [0.0, 1.0, 0.0]
        } else {
            [0.0, 0.0, 1.0]
        };
        let along = dot(hint, d);
        let up = unit([
            hint[0] - along * d[0],
            hint[1] - along * d[1],
            hint[2] - along * d[2],
        ]);
        let right = [
            up[1] * d[2] - up[2] * d[1],
            up[2] * d[0] - up[0] * d[2],
            up[0] * d[1] - up[1] * d[0],
        ];
        (right, up)
    }

    /// The point of the XY plane (z = 0) seen at `(across, up)` on the
    /// view's plane, along the line of sight: where a point picked in the
    /// view falls (ADR 0030 D4). `None` where the plane is seen edge on
    /// (Front, Back, Left, Right).
    pub fn plane_point(self, across: f64, up: f64) -> Option<(f64, f64)> {
        let d = unit(self.direction());
        if d[2].abs() < 1e-9 {
            return None;
        }
        let (right, top) = self.axes();
        let at = |i: usize| across * right[i] + up * top[i];
        let t = -at(2) / d[2];
        Some((at(0) + t * d[0], at(1) + t * d[1]))
    }

    /// Where a point falls on the view's plane: across, and up.
    pub fn project(self, p: Point3) -> (f64, f64) {
        let (right, up) = self.axes();
        let p = [p.x(), p.y(), p.z()];
        (dot(p, right), dot(p, up))
    }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn unit(v: [f64; 3]) -> [f64; 3] {
    let length = dot(v, v).sqrt();
    [v[0] / length, v[1] / length, v[2] / length]
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
