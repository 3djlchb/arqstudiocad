use super::*;

const TOL: Tolerance = Tolerance::DEFAULT;
/// tan(π/8), a quarter circle's bulge.
const Q: f64 = 0.414_213_562_373_095_03;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn path(points: &[(f64, f64, f64)]) -> Vec<(Point2, f64)> {
    points.iter().map(|&(x, y, b)| (p(x, y), b)).collect()
}

fn line(a: (f64, f64), b: (f64, f64)) -> Geometry2 {
    Geometry2::Line(Line::new(p(a.0, a.1), p(b.0, b.1)).unwrap())
}

fn same(got: &[(Point2, f64)], expected: &[(f64, f64, f64)]) {
    assert_eq!(got.len(), expected.len(), "{got:?}");
    for (&(q, b), &(x, y, e)) in got.iter().zip(expected) {
        assert!(
            q.approx_eq(p(x, y), Tolerance::new(1e-9, 1e-12).unwrap()) && (b - e).abs() < 1e-9,
            "{got:?} != {expected:?}"
        );
    }
}

const ELL: [(f64, f64, f64); 3] = [(0.0, 0.0, 0.0), (10.0, 0.0, 0.0), (10.0, 10.0, 0.0)];

#[test]
fn an_open_polyline_loses_the_part_between_the_cuts() {
    let v = path(&ELL);
    let edges = [line((3.0, -1.0), (3.0, 1.0)), line((9.0, 5.0), (11.0, 5.0))];
    let left = trim_polyline(&v, false, &edges, p(5.0, 0.2), TOL)
        .unwrap()
        .unwrap();
    assert_eq!(left.len(), 2, "two pieces: {left:?}");
    same(&left[0], &[(0.0, 0.0, 0.0), (3.0, 0.0, 0.0)]);
    same(&left[1], &[(10.0, 5.0, 0.0), (10.0, 10.0, 0.0)]);
    // Picked before the first cut: the rest stays, its corner too.
    let left = trim_polyline(&v, false, &edges, p(1.0, 0.0), TOL)
        .unwrap()
        .unwrap();
    same(
        &left[0],
        &[(3.0, 0.0, 0.0), (10.0, 0.0, 0.0), (10.0, 10.0, 0.0)],
    );
    assert_eq!(trim_polyline(&v, false, &[], p(1.0, 0.0), TOL), Ok(None));
}

#[test]
fn a_closed_polyline_opens_into_what_is_left() {
    let v = path(&[
        (0.0, 0.0, 0.0),
        (4.0, 0.0, 0.0),
        (4.0, 2.0, 0.0),
        (0.0, 2.0, 0.0),
    ]);
    let edges = [line((2.0, -1.0), (2.0, 3.0))];
    let left = trim_polyline(&v, true, &edges, p(4.0, 1.0), TOL)
        .unwrap()
        .unwrap();
    let [only] = left.as_slice() else {
        panic!("{left:?}")
    };
    same(
        only,
        &[
            (2.0, 2.0, 0.0),
            (0.0, 2.0, 0.0),
            (0.0, 0.0, 0.0),
            (2.0, 0.0, 0.0),
        ],
    );
    // Picked on the left: the right part is left, round the start.
    let left = trim_polyline(&v, true, &edges, p(0.0, 1.0), TOL)
        .unwrap()
        .unwrap();
    same(
        &left[0],
        &[
            (2.0, 0.0, 0.0),
            (4.0, 0.0, 0.0),
            (4.0, 2.0, 0.0),
            (2.0, 2.0, 0.0),
        ],
    );
    // One cut cannot open it.
    let one = [line((2.0, -1.0), (2.0, 1.0))];
    assert_eq!(trim_polyline(&v, true, &one, p(4.0, 1.0), TOL), Ok(None));
}

#[test]
fn an_arc_segment_is_cut_with_its_bulge() {
    // A half circle about (1, 0), through (1, -1), cut at its middle.
    let v = path(&[(0.0, 0.0, 1.0), (2.0, 0.0, 0.0)]);
    let edges = [line((1.0, -2.0), (1.0, 0.5))];
    let left = trim_polyline(&v, false, &edges, p(0.3, -0.7), TOL)
        .unwrap()
        .unwrap();
    same(&left[0], &[(1.0, -1.0, Q), (2.0, 0.0, 0.0)]);
}

#[test]
fn an_open_polyline_grows_at_the_end_picked() {
    let v = path(&ELL);
    let edges = [
        line((5.0, 15.0), (15.0, 15.0)),
        line((-4.0, -1.0), (-4.0, 1.0)),
    ];
    let grown = extend_polyline(&v, false, &edges, p(10.0, 9.0), TOL)
        .unwrap()
        .unwrap();
    same(
        &grown,
        &[(0.0, 0.0, 0.0), (10.0, 0.0, 0.0), (10.0, 15.0, 0.0)],
    );
    let grown = extend_polyline(&v, false, &edges, p(1.0, 0.0), TOL)
        .unwrap()
        .unwrap();
    same(
        &grown,
        &[(-4.0, 0.0, 0.0), (10.0, 0.0, 0.0), (10.0, 10.0, 0.0)],
    );
    let missed = [line((30.0, 30.0), (31.0, 30.0))];
    assert_eq!(
        extend_polyline(&v, false, &missed, p(10.0, 9.0), TOL),
        Ok(None)
    );
    assert!(extend_polyline(&v, true, &edges, p(10.0, 9.0), TOL).is_err());
}

#[test]
fn an_arc_at_the_end_grows_round_its_circle_either_way() {
    // Counterclockwise: a quarter circle of radius 5 from (5, 0) to (0, 5),
    // grown to x = -3, where y = 4.
    let edges = [line((-3.0, -10.0), (-3.0, 10.0))];
    let v = path(&[(5.0, 0.0, Q), (0.0, 5.0, 0.0)]);
    let grown = extend_polyline(&v, false, &edges, p(0.5, 4.9), TOL)
        .unwrap()
        .unwrap();
    let sweep = 4.0f64.atan2(-3.0);
    same(&grown, &[(5.0, 0.0, (sweep / 4.0).tan()), (-3.0, 4.0, 0.0)]);
    // Clockwise: from (0, 5) to (5, 0), grown to y = -3, where x = 4.
    let edges = [line((-10.0, -3.0), (10.0, -3.0))];
    let v = path(&[(0.0, 5.0, -Q), (5.0, 0.0, 0.0)]);
    let grown = extend_polyline(&v, false, &edges, p(4.9, 0.5), TOL)
        .unwrap()
        .unwrap();
    let sweep = std::f64::consts::FRAC_PI_2 + 3.0f64.atan2(4.0);
    same(
        &grown,
        &[(0.0, 5.0, -(sweep / 4.0).tan()), (4.0, -3.0, 0.0)],
    );
}

#[test]
fn quick_search_reuses_polyline_terminal_choice_and_conservative_bounds() {
    let vertices = [(p(0., 0.), 0.), (p(10., 0.), -1.), (p(10., 10.), 0.)];
    let (first, pick) = polyline_extension_target(&vertices, false, p(0., 0.)).unwrap();
    assert!(matches!(first, Geometry2::Line(_)));
    assert_eq!(pick, p(0., 0.));
    let (last, pick) = polyline_extension_target(&vertices, false, p(10., 10.)).unwrap();
    assert!(matches!(last, Geometry2::Arc(_)));
    assert_eq!(pick, p(10., 10.));
    assert!(polyline_extension_target(&vertices, true, p(0., 0.)).is_err());
    let bounds = polyline_trim_bounds(&vertices, false, TOL).unwrap();
    assert!(bounds.contains_point(p(5., 5.), TOL));
    assert!(bounds.contains_point(p(10., 10.), TOL));
}
