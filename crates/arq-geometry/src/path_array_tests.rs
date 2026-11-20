use super::*;
use crate::Tolerance;
use std::f64::consts::PI;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}
fn near(actual: Point2, expected: Point2) {
    assert!(
        actual.approx_eq(expected, Tolerance::DEFAULT),
        "{actual:?} != {expected:?}"
    );
}

#[test]
fn path_array_line_stations_include_both_ends_and_preserve_survey_offsets() {
    let base = p(1e6, 2e6);
    let path = [(p(1e6 + 10., 2e6), 0.), (p(1e6 + 30., 2e6), 0.)];
    let transforms = path_array_transforms(&path, base, 3, true).unwrap();
    assert_eq!(transforms.len(), 3);
    for (transform, x) in transforms.iter().zip([10., 20., 30.]) {
        near(
            transform.checked_apply_point(base).unwrap(),
            p(1e6 + x, 2e6),
        );
        near(
            transform
                .checked_apply_point(p(1e6 + 2., 2e6 + 1.))
                .unwrap(),
            p(1e6 + x + 2., 2e6 + 1.),
        );
    }
}

#[test]
fn path_array_corner_uses_outgoing_tangent_and_alignment_can_be_disabled() {
    let path = [(p(10., 0.), 0.), (p(20., 0.), 0.), (p(20., 10.), 0.)];
    for (align, expected) in [
        (true, [p(12., 0.), p(20., 2.), p(20., 12.)]),
        (false, [p(12., 0.), p(22., 0.), p(22., 10.)]),
    ] {
        let transforms = path_array_transforms(&path, p(0., 0.), 3, align).unwrap();
        for (transform, expected) in transforms.into_iter().zip(expected) {
            near(transform.checked_apply_point(p(2., 0.)).unwrap(), expected);
        }
    }
    // Three equal decimal lengths: total/3 rounds just below the vertex.
    let decimal = [
        (p(0., 0.), 0.),
        (p(0.01, 0.), 0.),
        (p(0.01, 0.01), 0.),
        (p(0.02, 0.01), 0.),
    ];
    let placements = path_array_transforms(&decimal, p(0., 0.), 4, true).unwrap();
    near(
        placements[1].checked_apply_point(p(0.001, 0.)).unwrap(),
        p(0.01, 0.001),
    );
}

#[test]
fn path_array_signed_bulges_follow_true_arc_length_and_traversal() {
    for sign in [1., -1.] {
        let path = [(p(0., 0.), sign), (p(10., 0.), 0.)];
        let transforms = path_array_transforms(&path, p(0., 0.), 3, true).unwrap();
        near(
            transforms[0].checked_apply_point(p(2., 0.)).unwrap(),
            p(2., 0.),
        );
        near(
            transforms[1].checked_apply_point(p(0., 0.)).unwrap(),
            p(5., -sign * 5.),
        );
        near(
            transforms[1].checked_apply_point(p(2., 0.)).unwrap(),
            p(5., -sign * 3.),
        );
        near(
            transforms[2].checked_apply_point(p(2., 0.)).unwrap(),
            p(8., 0.),
        );
    }
    let length = 5. * PI;
    let path = [
        (p(0., 0.), 0.),
        (p(length, 0.), 1.),
        (p(length + 10., 0.), 0.),
    ];
    let transforms = path_array_transforms(&path, p(0., 0.), 5, false).unwrap();
    for (transform, expected) in transforms.into_iter().zip([
        p(0., 0.),
        p(length / 2., 0.),
        p(length, 0.),
        p(length + 5., -5.),
        p(length + 10., 0.),
    ]) {
        near(transform.checked_apply_point(p(0., 0.)).unwrap(), expected);
    }
}

#[test]
fn path_array_invalid_counts_paths_and_frame_overflow_are_rejected() {
    let valid = [(p(0., 0.), 0.), (p(10., 0.), 0.)];
    for count in [0, 1, 100001, usize::MAX] {
        assert!(path_array_transforms(&valid, p(0., 0.), count, true).is_err());
    }
    for path in [
        vec![],
        vec![(p(0., 0.), 0.)],
        vec![(p(0., 0.), 0.), (p(0., 0.), 0.)],
        vec![(p(0., 0.), f64::NAN), (p(1., 0.), 0.)],
        vec![(p(0., 0.), 0.), (p(1e308, 0.), 0.), (p(0., 0.), 0.)],
    ] {
        assert!(path_array_transforms(&path, p(0., 0.), 3, true).is_err());
    }
    assert!(Transform2::placement(p(-1e308, 0.), p(1e308, 0.), 0.).is_err());
    assert!(Transform2::placement(p(0., 0.), p(1., 0.), f64::NAN).is_err());
}
