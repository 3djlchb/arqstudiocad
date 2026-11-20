use super::*;
use crate::Tolerance;
use std::f64::consts::FRAC_PI_2;

#[test]
fn rectangular_grid_signed_axes_and_lazy_order_match_hand_placement() {
    let grid = RectangularGrid::new(2, 3, -5., 10., FRAC_PI_2).unwrap();
    assert_eq!(grid.cells(), 6);
    let offsets: Vec<_> = grid.offsets().collect::<Result<_, _>>().unwrap();
    for (offset, (x, y)) in
        offsets
            .into_iter()
            .zip([(0., 10.), (0., 20.), (5., 0.), (5., 10.), (5., 20.)])
    {
        assert!(offset.approx_eq(Vector2::new(x, y).unwrap(), Tolerance::DEFAULT));
    }
    let large = RectangularGrid::new(1, 100_000, 0., 1., 0.).unwrap();
    assert_eq!(large.offsets().take(2).count(), 2);
    assert_eq!(large.cells(), 100_000);
}

#[test]
fn rectangular_grid_rejects_invalid_inputs_and_unseen_corner_overflow() {
    for (r, c) in [(0, 2), (2, 0), (1, 1), (100001, 1), (usize::MAX, 2)] {
        assert!(RectangularGrid::new(r, c, 1., 1., 0.).is_err());
    }
    for (row, col, angle) in [
        (0., 1., 0.),
        (1., 0., 0.),
        (f64::NAN, 1., 0.),
        (1., 1., f64::INFINITY),
    ] {
        assert!(RectangularGrid::new(2, 2, row, col, angle).is_err());
    }
    assert!(RectangularGrid::new(1, 100_000, 0., 1e308, 0.).is_err());
    assert!(RectangularGrid::new(2, 2, 1.4e308, 1.4e308, std::f64::consts::FRAC_PI_4).is_err());
}
