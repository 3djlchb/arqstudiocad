use super::*;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}
fn path(points: &[(f64, f64)]) -> PolylinePath {
    points.iter().map(|&(x, y)| (p(x, y), 0.)).collect()
}

#[test]
fn closed_break_matches_all_eleven_approved_windows_cases() {
    let square = path(&[(0., 0.), (10., 0.), (10., 10.), (0., 10.)]);
    let clockwise = path(&[(0., 0.), (0., 10.), (10., 10.), (10., 0.)]);
    let mut curved = path(&[(0., 0.), (10., 0.), (10., 5.), (0., 5.)]);
    curved[2].1 = 1.;
    let b1 = path(&[(5., 10.), (0., 10.), (0., 0.), (10., 0.), (10., 5.)]);
    let b8 = path(&[
        (8., 0.),
        (10., 0.),
        (10., 10.),
        (0., 10.),
        (0., 0.),
        (2., 0.),
    ]);
    let mut b6 = path(&[(5., 10.), (0., 5.), (0., 0.), (10., 0.), (10., 2.)]);
    b6[0].1 = 2_f64.sqrt() - 1.;
    let cases = [
        ("B1", &square, p(10., 5.), p(5., 10.), b1.clone()),
        ("B2", &square, p(5., 10.), p(10., 5.), b1.clone()),
        (
            "B3",
            &square,
            p(0., 5.),
            p(5., 0.),
            path(&[(0., 5.), (0., 0.), (5., 0.)]),
        ),
        (
            "B4",
            &clockwise,
            p(10., 5.),
            p(5., 10.),
            path(&[(10., 5.), (10., 0.), (0., 0.), (0., 10.), (5., 10.)]),
        ),
        (
            "B5",
            &square,
            p(10., 5.),
            p(10., 5.),
            path(&[
                (10., 5.),
                (10., 10.),
                (0., 10.),
                (0., 0.),
                (10., 0.),
                (10., 5.),
            ]),
        ),
        ("B6", &curved, p(10., 2.), p(5., 10.), b6),
        ("B7", &square, p(10., 5.), p(5., 10.), b1),
        ("B8", &square, p(2., 0.), p(8., 0.), b8.clone()),
        ("B9", &square, p(8., 0.), p(2., 0.), b8),
        (
            "B10",
            &square,
            p(10., 5.),
            p(10., 10.),
            path(&[(10., 10.), (0., 10.), (0., 0.), (10., 0.), (10., 5.)]),
        ),
        (
            "B11",
            &square,
            p(10., 0.),
            p(0., 10.),
            path(&[(0., 10.), (0., 0.), (10., 0.)]),
        ),
    ];
    for (name, vertices, first, second, want) in cases {
        let result = break_closed_polyline(vertices, first, second, Tolerance::DEFAULT).unwrap();
        assert_eq!(result.vertices.len(), want.len(), "{name}");
        assert_eq!(result.spans.len(), want.len() - 1, "{name}: provenance");
        assert!(
            result
                .spans
                .iter()
                .all(|span| span.segment < vertices.len()),
            "{name}: unrolled indices must not escape"
        );
        for ((actual, bulge), (point, want_bulge)) in result.vertices.iter().zip(want) {
            assert!(
                actual.approx_eq(point, Tolerance::DEFAULT),
                "{name}: {actual:?} != {point:?}"
            );
            assert!(
                (bulge - want_bulge).abs() < Tolerance::DEFAULT.angular(),
                "{name}: bulge"
            );
        }
    }
}

#[test]
fn closed_break_wraps_signed_closing_arcs_with_original_width_provenance() {
    for sign in [-1., 1.] {
        let vertices = [(p(0., 0.), sign), (p(2., 0.), sign)];
        let at = p(1., sign);
        let result = break_closed_polyline(&vertices, at, at, Tolerance::DEFAULT).unwrap();
        assert_eq!(result.vertices.len(), 4);
        assert_eq!(
            result.spans.iter().map(|s| s.segment).collect::<Vec<_>>(),
            [1, 0, 1]
        );
        assert!((result.spans[0].from - 0.5).abs() < Tolerance::DEFAULT.angular());
        assert!((result.spans[2].to - 0.5).abs() < Tolerance::DEFAULT.angular());
        assert_eq!(result.vertices.last().unwrap().1, 0.);
        for pair in result.vertices.windows(2) {
            let (arc, _) = Arc::from_bulge(pair[0].0, pair[1].0, pair[0].1).unwrap();
            assert!(arc.center().approx_eq(p(1., 0.), Tolerance::DEFAULT));
            assert!((arc.radius() - 1.).abs() < Tolerance::DEFAULT.linear());
            assert_eq!(pair[0].1.signum(), sign);
        }
    }
}

#[test]
fn closed_break_projects_picks_and_opens_at_vertex_zero_far_from_origin() {
    let origin = p(1e6, 2e6);
    let square = path(&[
        (1e6, 2e6),
        (1e6 + 10., 2e6),
        (1e6 + 10., 2e6 + 10.),
        (1e6, 2e6 + 10.),
    ]);
    let opened = break_closed_polyline(&square, origin, origin, Tolerance::DEFAULT).unwrap();
    assert_eq!(opened.vertices.len(), 5);
    assert_eq!(opened.vertices.first().unwrap().0, origin);
    assert_eq!(opened.vertices.last().unwrap().0, origin);
    assert_eq!(
        opened.spans.iter().map(|s| s.segment).collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    let cut = break_closed_polyline(
        &square,
        p(1e6 + 13., 2e6 + 5.),
        p(1e6 + 5., 2e6 + 17.),
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(cut.vertices.first().unwrap().0, p(1e6 + 5., 2e6 + 10.));
    assert_eq!(cut.vertices.last().unwrap().0, p(1e6 + 10., 2e6 + 5.));
    assert_eq!(
        cut.spans.iter().map(|s| s.segment).collect::<Vec<_>>(),
        [2, 3, 0, 1]
    );
}

#[test]
fn closed_break_rejects_empty_degenerate_and_nonfinite_bulge_paths() {
    let at = p(0., 0.);
    for vertices in [
        vec![],
        vec![(at, 0.)],
        vec![(at, f64::NAN), (p(1., 0.), 0.)],
    ] {
        assert!(break_closed_polyline(&vertices, at, at, Tolerance::DEFAULT).is_err());
    }
}
