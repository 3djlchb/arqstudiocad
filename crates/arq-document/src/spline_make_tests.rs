//! The splines SPLINE makes (ADR 0042 D4), written to the drawing, and a
//! spline of fit points alone, as DWG keeps one, drawn as SPLINE made it.
//! A submodule of `entities`.

use acadrust::EntityType;
use acadrust::entities::Spline as CodecSpline;
use acadrust::types::Vector3;
use arq_geometry::{Point2, Spline, SplineFit, SplineKnots, Transform2, Vector2};

use crate::document::Document;
use crate::drawing::Shape;
use crate::test_dxf::{LAYERS, dxf};

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn v(x: f64, y: f64) -> Vector2 {
    Vector2::new(x, y).unwrap()
}

fn wave() -> Vec<Point2> {
    vec![
        p(0.0, 0.0),
        p(3.0, 4.0),
        p(7.0, 1.0),
        p(9.0, 6.0),
        p(14.0, 5.0),
    ]
}

fn codec(doc: &Document, id: u64) -> CodecSpline {
    match doc.cad().get_entity(crate::translate::handle(id)).unwrap() {
        EntityType::Spline(sp) => sp.clone(),
        other => panic!("{other:?}"),
    }
}

/// The one shape entity `id` draws as, an exact spline.
fn drawn(doc: &Document, id: u64) -> Spline {
    let items = doc.drawing(id).unwrap();
    let [item] = &items[..] else {
        panic!("{items:?}")
    };
    match &item.shape {
        Shape::Spline(s) => (**s).clone(),
        other => panic!("{other:?}"),
    }
}

fn at(points: &[Point2], z: f64) -> Vec<Vector3> {
    points
        .iter()
        .map(|q| Vector3::new(q.x(), q.y(), z))
        .collect()
}

#[test]
fn a_spline_through_fit_points_keeps_them_its_control_points_and_tangents() {
    let mut doc = Document::new();
    let fit = SplineFit::new(
        wave(),
        SplineKnots::SquareRoot,
        Some(v(0.0, 2.0)),
        Some(v(3.0, 0.0)),
        false,
    )
    .unwrap();
    let id = doc.add_fit_spline(&fit, 2.0).unwrap();
    let made = fit.spline().unwrap();
    let sp = codec(&doc, id);
    assert_eq!(sp.degree, 3);
    assert_eq!(sp.knots, made.knots());
    assert_eq!(sp.control_points, at(made.points(), 2.0));
    assert!(sp.weights.is_empty());
    assert_eq!(sp.fit_points, at(&wave(), 2.0));
    assert_eq!(sp.knot_parameterization, 1);
    assert_eq!(sp.begin_tangent, Vector3::new(0.0, 1.0, 0.0));
    assert_eq!(sp.end_tangent, Vector3::new(1.0, 0.0, 0.0));
    assert_eq!(sp.normal, Vector3::new(0.0, 0.0, 1.0));
    assert!(!sp.flags.closed && !sp.flags.periodic && !sp.flags.rational);
    assert!(sp.flags.planar && !sp.flags.linear);
    assert_eq!(
        sp.dwg_flags1,
        1 | 8,
        "by fit points, with their knots method"
    );
    assert_eq!(sp.common.layer, doc.current_layer().name());
    assert_eq!(drawn(&doc, id).flatten(), made.flatten());
}

#[test]
fn a_closed_spline_is_periodic_and_marked_closed() {
    let mut doc = Document::new();
    let fit = SplineFit::new(wave(), SplineKnots::Uniform, None, None, true).unwrap();
    let id = doc.add_fit_spline(&fit, 0.0).unwrap();
    let sp = codec(&doc, id);
    assert!(sp.flags.closed && sp.flags.periodic);
    assert_eq!(sp.dwg_flags1, 1 | 4 | 8);
    assert_eq!(sp.knot_parameterization, 2);
    assert_eq!(
        (sp.begin_tangent, sp.end_tangent),
        (Vector3::ZERO, Vector3::ZERO)
    );
    assert_eq!(drawn(&doc, id).flatten(), fit.spline().unwrap().flatten());

    let cv = Spline::by_vertices(3, wave(), true).unwrap();
    let id = doc.add_spline(&cv, 0.0).unwrap();
    let sp = codec(&doc, id);
    assert!(sp.flags.closed && sp.flags.periodic);
    assert_eq!(sp.dwg_flags1, 4);
    assert!(sp.fit_points.is_empty());
    assert_eq!(sp.control_points.len(), wave().len() + 3);
}

#[test]
fn a_spline_by_vertices_keeps_its_degree_knots_and_points() {
    let mut doc = Document::new();
    for degree in [1, 2, 3, 4] {
        let cv = Spline::by_vertices(degree, wave(), false).unwrap();
        let id = doc.add_spline(&cv, -1.5).unwrap();
        let sp = codec(&doc, id);
        assert_eq!(sp.degree, degree as i32);
        assert_eq!(sp.knots, cv.knots());
        assert_eq!(sp.control_points, at(&wave(), -1.5));
        assert!(sp.fit_points.is_empty());
        assert!(!sp.flags.closed && !sp.flags.periodic && sp.flags.planar);
        assert_eq!(sp.flags.linear, degree == 1);
        assert_eq!(sp.dwg_flags1, 0);
        assert_eq!(drawn(&doc, id).flatten(), cv.flatten());
    }
    let refused = doc.add_spline(&Spline::by_vertices(3, wave(), false).unwrap(), f64::NAN);
    assert!(refused.is_err());
    assert_eq!(doc.entities().len(), 4, "nothing written when refused");
}

#[test]
fn a_made_spline_saved_and_read_again_is_drawn_and_boxed_the_same() {
    // DWG keeps a spline made through fit points by them alone: read
    // again, it is made again from them, to the last bit, and the check
    // after saving finds nothing lost.
    let fits = [
        SplineFit::new(
            wave(),
            SplineKnots::SquareRoot,
            Some(v(0.0, 2.0)),
            Some(v(3.0, -1.0)),
            false,
        )
        .unwrap(),
        SplineFit::new(wave(), SplineKnots::Chord, None, None, false).unwrap(),
        SplineFit::new(wave(), SplineKnots::Uniform, None, None, true).unwrap(),
    ];
    let mut doc = Document::new();
    let mut ids: Vec<u64> = fits
        .iter()
        .map(|f| doc.add_fit_spline(f, 1.25).unwrap())
        .collect();
    for closed in [false, true] {
        let cv = Spline::by_vertices(3, wave(), closed).unwrap();
        ids.push(doc.add_spline(&cv, 0.0).unwrap());
    }
    let dir = tempfile::tempdir().unwrap();
    for name in ["made.dwg", "made.dxf"] {
        let path = dir.path().join(name);
        doc.save(&path).unwrap();
        let back = Document::open(&path).unwrap();
        for (i, &id) in ids.iter().enumerate() {
            let sp = codec(&back, id);
            if name.ends_with(".dwg") && i < fits.len() {
                assert!(
                    sp.control_points.is_empty(),
                    "DWG keeps the fit points alone"
                );
            }
            assert_eq!(
                drawn(&back, id).flatten(),
                drawn(&doc, id).flatten(),
                "{name} {i}"
            );
            assert_eq!(
                back.entity(id).unwrap().bounds(),
                doc.entity(id).unwrap().bounds(),
                "{name} {i}"
            );
        }
    }
}

/// A drawing of one SPLINE of fit points alone, flags `flags`, knots
/// method in them, through `points` at height `z`, with tangents `start`
/// and `end` (DXF 12 and 13) when given.
fn fit_only(
    flags: i32,
    points: &[(f64, f64, f64)],
    start: Option<(f64, f64, f64)>,
    end: Option<(f64, f64, f64)>,
) -> Document {
    let mut c: Vec<String> = ["0", "SPLINE", "8", "0", "210", "0", "220", "0", "230", "1"]
        .into_iter()
        .map(String::from)
        .collect();
    let mut push = |code: &str, value: String| {
        c.push(code.to_owned());
        c.push(value);
    };
    push("70", flags.to_string());
    push("71", "3".to_owned());
    push("72", "0".to_owned());
    push("73", "0".to_owned());
    push("74", points.len().to_string());
    for (code, t) in [("2", start), ("3", end)] {
        if let Some((x, y, z)) = t {
            push(&format!("1{code}"), format!("{x:?}"));
            push(&format!("2{code}"), format!("{y:?}"));
            push(&format!("3{code}"), format!("{z:?}"));
        }
    }
    for (x, y, z) in points {
        push("11", format!("{x:?}"));
        push("21", format!("{y:?}"));
        push("31", format!("{z:?}"));
    }
    let refs: Vec<&str> = c.iter().map(String::as_str).collect();
    dxf(LAYERS, &[], &[&refs])
}

const FIT: i32 = 8 | 1024;

fn wave3(z: f64) -> Vec<(f64, f64, f64)> {
    wave().iter().map(|q| (q.x(), q.y(), z)).collect()
}

#[test]
fn a_spline_of_fit_points_alone_is_the_one_spline_makes_through_them() {
    // Its knots method from the flags, its tangents' directions; closed,
    // periodic and its tangents not used.
    let cases = [
        (
            FIT | 64,
            Some((0.0, 4.0, 0.0)),
            None,
            SplineKnots::SquareRoot,
            false,
        ),
        (
            FIT | 128,
            None,
            Some((2.0, 2.0, 0.0)),
            SplineKnots::Uniform,
            false,
        ),
        (FIT | 32, None, None, SplineKnots::Chord, false),
        (
            FIT | 32 | 1,
            Some((1.0, 0.0, 0.0)),
            None,
            SplineKnots::Chord,
            true,
        ),
    ];
    for (flags, start, end, knots, closed) in cases {
        let doc = fit_only(flags, &wave3(3.0), start, end);
        let id = doc.entities()[0].id();
        let direction = |t: Option<(f64, f64, f64)>| t.filter(|_| !closed).map(|t| v(t.0, t.1));
        let fit = SplineFit::new(wave(), knots, direction(start), direction(end), closed).unwrap();
        let made = fit.spline().unwrap();
        assert_eq!(drawn(&doc, id).flatten(), made.flatten(), "{flags}");
        let b = doc.entity(id).unwrap().bounds().unwrap();
        assert_eq!(
            b,
            arq_geometry::Bounds2::from_points(made.points().iter().copied()).unwrap()
        );
    }
}

#[test]
fn a_spline_of_fit_points_out_of_the_plan_is_drawn_as_the_codec_draws_it() {
    let mut tilted = wave3(0.0);
    tilted[2].2 = 1.0;
    let steep = Some((1.0, 0.0, 1.0));
    for doc in [
        fit_only(FIT, &tilted, None, None),
        fit_only(FIT, &wave3(0.0), steep, None),
        fit_only(FIT, &wave3(0.0), None, steep),
    ] {
        let id = doc.entities()[0].id();
        let items = doc.drawing(id).unwrap();
        assert_eq!(items.len(), wave().len() - 1, "{items:?}");
        assert!(items.iter().all(|i| matches!(i.shape, Shape::Line(_))));
    }
}

#[test]
fn a_spline_of_fit_points_alone_moves_whole() {
    let mut doc = fit_only(FIT | 32, &wave3(2.0), Some((0.0, 1.0, 0.0)), None);
    let id = doc.entities()[0].id();
    let t = Transform2::rotation(p(1.0, 1.0), 0.9)
        .unwrap()
        .then(Transform2::scaling(Point2::ORIGIN, 1.5).unwrap());
    let before = drawn(&doc, id);
    doc.transform_entity(id, t).unwrap();
    let after = drawn(&doc, id);
    let sp = codec(&doc, id);
    assert!(sp.control_points.is_empty());
    assert!(sp.fit_points.iter().all(|q| q.z == 2.0), "at its height");
    let moved = before.transformed(t).unwrap();
    for (a, b) in after.flatten().iter().zip(moved.flatten()) {
        assert!(a.distance_to(*b) < 1e-9, "{a:?} {b:?}");
    }
}

#[test]
fn a_spline_written_as_plan_geometry_is_a_spline_drawn_as_it() {
    // As the model writes curves it owns (ADR 0043 D4), at its height.
    let mut doc = Document::new();
    let layer = doc.current_layer().id();
    let s = Spline::by_vertices(3, wave(), true).unwrap();
    let plan = crate::PlanGeometry::at_elevation(s.clone().into(), 2.5).unwrap();
    let id = doc.add_entity(plan, layer).unwrap();
    let sp = codec(&doc, id);
    assert_eq!(sp.control_points, at(s.points(), 2.5));
    assert!(sp.flags.closed && sp.flags.periodic);
    assert_eq!(drawn(&doc, id).flatten(), s.flatten());
}

#[test]
fn a_spline_piece_written_is_by_its_vertices_its_fit_dropped() {
    // A fitted spline cut keeps the exact piece, its control points; its
    // fit points no longer describe it (ADR 0043 D4).
    let mut doc = Document::new();
    let fit = SplineFit::new(
        wave(),
        SplineKnots::SquareRoot,
        Some(v(0.0, 1.0)),
        None,
        false,
    )
    .unwrap();
    let id = doc.add_fit_spline(&fit, 1.5).unwrap();
    let plan = doc.entity(id).unwrap().plan().unwrap();
    assert!((plan.elevation() - 1.5).abs() < 1e-12);
    let arq_geometry::Geometry2::Spline(whole) = plan.geometry() else {
        panic!()
    };
    let (a, b) = whole.domain();
    let piece = whole.between(a + 0.2 * (b - a), b).unwrap();
    doc.set_plan_geometry(id, plan.with_geometry(piece.clone().into()))
        .unwrap();
    let sp = codec(&doc, id);
    assert!(sp.fit_points.is_empty());
    assert_eq!(sp.control_points, at(piece.points(), 1.5));
    assert_eq!(sp.knots, piece.knots());
    assert_eq!(
        (sp.begin_tangent, sp.knot_parameterization),
        (Vector3::ZERO, 0)
    );
    assert_eq!(sp.dwg_flags1, 0, "by its vertices");
    assert_eq!(drawn(&doc, id).flatten(), piece.flatten());
    // A line is not written as a spline, nor a spline as a line.
    let layer = doc.current_layer().id();
    let line = arq_geometry::Line::new(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    let other = doc
        .add_entity(crate::PlanGeometry::new(line.into()), layer)
        .unwrap();
    assert!(doc.set_plan_geometry(other, plan).is_err());
    let not_a_spline = crate::PlanGeometry::new(line.into());
    assert!(doc.set_plan_geometry(id, not_a_spline).is_err());
}

#[test]
fn a_closed_fitted_spline_is_written_with_its_first_fit_point_again_at_its_end() {
    // As ProgeCAD writes one, which reads ours back closed only so (SP3).
    let fit = SplineFit::new(wave(), SplineKnots::Chord, None, None, true).unwrap();
    let mut doc = Document::new();
    let id = doc.add_fit_spline(&fit, 0.0).unwrap();
    let written = |doc: &Document, id: u64| -> Vec<Point2> {
        codec(doc, id)
            .fit_points
            .iter()
            .map(|q| p(q.x, q.y))
            .collect()
    };
    let mut expected = wave();
    expected.push(wave()[0]);
    assert_eq!(written(&doc, id), expected);
    // Read, the repeat is dropped: the fit as made.
    assert_eq!(doc.spline_fit(id), Some(fit.clone()));
    // An open one is written as it is.
    let open = SplineFit::new(wave(), SplineKnots::Chord, None, None, false).unwrap();
    let open_id = doc.add_fit_spline(&open, 0.0).unwrap();
    assert_eq!(written(&doc, open_id), wave());
    // Its point moved by its grip: still once again at the end, not twice.
    doc.set_spline_point(id, 1, p(3.0, 5.0)).unwrap();
    let moved = written(&doc, id);
    assert_eq!(moved.len(), wave().len() + 1);
    assert_eq!(moved[0], moved[moved.len() - 1]);
    let refit = doc.spline_fit(id).unwrap();
    assert!(refit.is_closed() && refit.points().len() == wave().len());
    // Through DWG and DXF, the same fit and the same curve.
    let dxf = Document::from_dxf_bytes(doc.to_dxf_bytes().unwrap()).unwrap();
    let dwg = Document::read(
        doc.to_dwg_bytes().unwrap(),
        crate::FileFormat::Dwg,
        std::path::Path::new("closed.dwg"),
    )
    .unwrap();
    for back in [dxf, dwg] {
        let closed = back
            .entities()
            .iter()
            .map(|e| e.id())
            .find(|&e| back.spline_fit(e).is_some_and(|f| f.is_closed()))
            .unwrap();
        assert_eq!(back.spline_fit(closed), Some(refit.clone()));
        assert_eq!(written(&back, closed), moved);
        assert_eq!(drawn(&back, closed).flatten(), drawn(&doc, id).flatten());
    }
}
