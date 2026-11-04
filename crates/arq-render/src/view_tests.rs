//! Tests of the preset views (a submodule of `view`): each one's screen
//! axes, worked out by hand from AutoCAD's view directions.

use super::*;

fn close(a: [f64; 3], b: [f64; 3]) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < 1e-12)
}

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::new(x, y, z).unwrap()
}

#[test]
fn orthographic_views_keep_their_axes_square() {
    let (x, y, z) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    let minus = |v: [f64; 3]| [-v[0], -v[1], -v[2]];
    for (view, right, up) in [
        (View::Top, x, y),
        // Turned over about Y: X runs to the left.
        (View::Bottom, minus(x), y),
        (View::Front, x, z),
        (View::Back, minus(x), z),
        (View::Left, minus(y), z),
        (View::Right, y, z),
    ] {
        let (r, u) = view.axes();
        assert!(close(r, right) && close(u, up), "{view:?}: {r:?} {u:?}");
    }
}

#[test]
fn the_plan_projects_onto_x_and_y() {
    assert_eq!(View::Top.project(p(3.0, 4.0, 50.0)), (3.0, 4.0));
    assert_eq!(View::Front.project(p(3.0, 4.0, 50.0)), (3.0, 50.0));
}

#[test]
fn isometric_views_show_the_three_axes_alike() {
    // From the southwest: X runs right and up, Y left and up, Z straight
    // up; each axis keeps √(2/3) of its length.
    let (right, up) = View::SwIso.axes();
    let s = 2.0f64.sqrt();
    let t = 6.0f64.sqrt();
    assert!(close(right, [1.0 / s, -1.0 / s, 0.0]), "{right:?}");
    assert!(close(up, [1.0 / t, 1.0 / t, 2.0 / t]), "{up:?}");
    for view in [View::SwIso, View::SeIso, View::NeIso, View::NwIso] {
        for axis in [p(1.0, 0.0, 0.0), p(0.0, 1.0, 0.0), p(0.0, 0.0, 1.0)] {
            let (a, b) = view.project(axis);
            assert!(((a * a + b * b) - 2.0 / 3.0).abs() < 1e-12, "{view:?}");
        }
        let (a, b) = view.project(p(0.0, 0.0, 1.0));
        assert!(a.abs() < 1e-12 && b > 0.0, "{view:?}: Z points up");
    }
    // From the northeast X runs left and down.
    let (a, b) = View::NeIso.project(p(1.0, 0.0, 0.0));
    assert!(a < 0.0 && b < 0.0, "{a} {b}");
}

#[test]
fn views_by_name() {
    assert_eq!(View::from_name("SWiso"), Some(View::SwIso));
    assert_eq!(View::from_name("plan"), None);
    for view in View::ALL {
        assert_eq!(View::from_name(view.name()), Some(view));
    }
    assert_eq!(View::default(), View::Top);
}
