use super::*;
use crate::Tolerance;
use std::f64::consts::{PI, TAU};
fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

#[test]
fn polar_full_turns_omit_duplicate_source_and_rotate_at_survey_coordinates() {
    let center = p(1e6, 2e6);
    for sign in [-1., 1.] {
        let transforms = polar_array_transforms(center, 4, sign * TAU, true, None).unwrap();
        assert_eq!(transforms.len(), 3);
        let expected = [
            ((0., 10. * sign), (0., 12. * sign)),
            ((-10., 0.), (-12., 0.)),
            ((0., -10. * sign), (0., -12. * sign)),
        ];
        for (t, (a, b)) in transforms.into_iter().zip(expected) {
            assert!(
                t.apply_point(p(1e6 + 10., 2e6))
                    .approx_eq(p(1e6 + a.0, 2e6 + a.1), Tolerance::DEFAULT)
            );
            assert!(
                t.apply_point(p(1e6 + 12., 2e6))
                    .approx_eq(p(1e6 + b.0, 2e6 + b.1), Tolerance::DEFAULT)
            );
        }
    }
}

#[test]
fn polar_partial_fills_include_the_last_angle_in_both_directions() {
    for sign in [-1., 1.] {
        let transforms = polar_array_transforms(Point2::ORIGIN, 3, sign * PI, true, None).unwrap();
        assert_eq!(transforms.len(), 2);
        assert!(
            transforms[0]
                .apply_point(p(10., 0.))
                .approx_eq(p(0., sign * 10.), Tolerance::DEFAULT)
        );
        assert!(
            transforms[1]
                .apply_point(p(10., 0.))
                .approx_eq(p(-10., 0.), Tolerance::DEFAULT)
        );
    }
}

#[test]
fn polar_without_rotation_moves_the_explicit_group_base_without_turning_items() {
    let transforms =
        polar_array_transforms(Point2::ORIGIN, 4, TAU, false, Some(p(10., 0.))).unwrap();
    for (t, expected) in transforms
        .into_iter()
        .zip([(0., 10.), (-10., 0.), (0., -10.)])
    {
        assert!(
            t.apply_point(p(10., 0.))
                .approx_eq(p(expected.0, expected.1), Tolerance::DEFAULT)
        );
        assert!(
            t.apply_point(p(12., 1.))
                .approx_eq(p(expected.0 + 2., expected.1 + 1.), Tolerance::DEFAULT)
        );
    }
    // A tiny fill still has a finite, local-frame orbital displacement.
    let t = polar_array_transforms(Point2::ORIGIN, 2, 1e-12, false, Some(p(1e20, 0.))).unwrap()[0];
    let q = t.apply_point(Point2::ORIGIN);
    assert!((q.x() + 0.00005).abs() < 1e-12);
    assert!((q.y() - 1e8).abs() < 1e-6);
}

#[test]
fn polar_invalid_counts_angles_missing_base_and_offset_overflow_are_rejected() {
    for count in [0, 1, 100001, usize::MAX] {
        assert!(polar_array_transforms(Point2::ORIGIN, count, TAU, true, None).is_err());
    }
    for fill in [0., TAU + 0.1, -TAU - 0.1, f64::NAN, f64::INFINITY] {
        assert!(polar_array_transforms(Point2::ORIGIN, 4, fill, true, None).is_err());
    }
    assert!(polar_array_transforms(Point2::ORIGIN, 4, TAU, false, None).is_err());
    assert!(polar_array_transforms(p(-1e308, 0.), 4, TAU, false, Some(p(1e308, 0.))).is_err());
    assert!(polar_array_transforms(Point2::ORIGIN, 2, TAU, false, Some(p(1e308, 0.))).is_err());
}

#[test]
fn polar_checked_point_application_rejects_intermediate_and_final_overflow() {
    let rotate = Transform2::rotation(p(-1e308, 0.), PI).unwrap();
    assert!(rotate.checked_apply_point(p(1e308, 0.)).is_err());
    let translate = Transform2::translation(Vector2::new(1e308, 0.).unwrap());
    assert!(translate.checked_apply_point(p(1e308, 0.)).is_err());
    let t = Transform2::rotation(p(1e6, 2e6), PI * 0.5).unwrap();
    assert!(
        t.checked_apply_point(p(1e6 + 10., 2e6))
            .unwrap()
            .approx_eq(p(1e6, 2e6 + 10.), Tolerance::DEFAULT)
    );
}
