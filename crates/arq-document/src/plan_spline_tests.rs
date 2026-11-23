//! A spline lying in the plan as plan geometry (ADR 0043 D4, delivery 3a):
//! read, written by its vertices, boxed by its control points, checked on
//! save by its data, stretched. A submodule of `entities`.

use acadrust::EntityType;
use arq_geometry::{Bounds2, Geometry2, Point2, Spline, SplineFit, SplineKnots, Vector2};

use crate::document::Document;
use crate::drawing::Shape;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
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

fn near(a: Point2, b: Point2) -> bool {
    a.distance_to(b) < 1e-9
}

/// The spline entity `id` is, as its plan geometry says, and its height.
fn plan_spline(doc: &Document, id: u64) -> (Spline, f64) {
    let plan = doc.entity(id).unwrap().plan().unwrap();
    match plan.geometry() {
        Geometry2::Spline(s) => ((*s).clone(), plan.elevation()),
        other => panic!("{other:?}"),
    }
}

fn codec(doc: &Document, id: u64) -> acadrust::entities::Spline {
    match doc.cad().get_entity(crate::translate::handle(id)).unwrap() {
        EntityType::Spline(sp) => sp.clone(),
        other => panic!("{other:?}"),
    }
}

/// The shape entity `id` draws, a spline.
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

fn window(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<arq_geometry::CrossingArea> {
    vec![arq_geometry::CrossingArea::Rectangle(Bounds2::new(
        p(x0, y0),
        p(x1, y1),
    ))]
}

#[test]
fn a_spline_in_the_plan_is_plan_geometry_boxed_by_its_control_points() {
    let mut doc = Document::new();
    let cv = Spline::by_vertices(3, wave(), false).unwrap();
    let id = doc.add_spline(&cv, 2.5).unwrap();
    let (s, z) = plan_spline(&doc, id);
    assert_eq!((s, z), (cv.clone(), 2.5));
    let e = doc.entity(id).unwrap();
    assert!(e.is_editable() && e.is_transformable());
    // The entity's box its control points' (ADR 0042 D4), not the curve's.
    let control = Bounds2::from_points(wave()).unwrap();
    assert_eq!(e.bounds(), Some(control));
    assert!(cv.bounds().max().y() < control.max().y());
    // One through fit points, as DWG keeps one: plan geometry too.
    let fit = SplineFit::new(wave(), SplineKnots::Chord, None, None, false).unwrap();
    let id = doc.add_fit_spline(&fit, -1.0).unwrap();
    assert_eq!(plan_spline(&doc, id), (fit.spline().unwrap(), -1.0));
    assert!(doc.check_model().is_empty(), "{:?}", doc.check_model());
}

#[test]
fn the_check_after_saving_sees_a_spline_changed_within_the_same_box() {
    // The same control points, one weight changed: the same entity box,
    // another curve.
    let spline = |w: f64| {
        Spline::new(
            3,
            wave(),
            vec![1.0, 1.0, w, 1.0, 1.0],
            Spline::clamped_knots(3, 5),
            false,
        )
        .unwrap()
    };
    let doc = |w: f64| {
        let mut d = Document::new();
        d.add_spline(&spline(w), 0.0).unwrap();
        d
    };
    assert!(doc(2.0).validate_roundtrip(&doc(2.0)).is_empty());
    assert!(!doc(2.0).validate_roundtrip(&doc(2.5)).is_empty());
    // Knots changed, likewise.
    let mut other = Document::new();
    let knots = vec![0.0, 0.0, 0.0, 0.0, 0.3, 1.0, 1.0, 1.0, 1.0];
    other
        .add_spline(
            &Spline::new(3, wave(), vec![1.0, 1.0, 2.0, 1.0, 1.0], knots, false).unwrap(),
            0.0,
        )
        .unwrap();
    assert!(!doc(2.0).validate_roundtrip(&other).is_empty());
}

#[test]
fn a_spline_moved_whole_keeps_its_fit_and_the_model_in_step() {
    let mut doc = Document::new();
    let fit = SplineFit::new(
        wave(),
        SplineKnots::SquareRoot,
        Some(Vector2::new(0.0, 1.0).unwrap()),
        None,
        false,
    )
    .unwrap();
    let id = doc.add_fit_spline(&fit, 0.0).unwrap();
    let t = arq_geometry::Transform2::rotation(p(1.0, 1.0), 0.6).unwrap();
    doc.transform_entity(id, t).unwrap();
    let sp = codec(&doc, id);
    assert_eq!(sp.fit_points.len(), 5, "fit points carried");
    assert_eq!(sp.knot_parameterization, 1);
    assert!(doc.check_model().is_empty(), "{:?}", doc.check_model());
}

#[test]
fn stretching_a_fitted_spline_moves_its_fit_points_inside_and_fits_again() {
    let mut doc = Document::new();
    let tangent = Vector2::new(1.0, 0.0).unwrap();
    let fit = SplineFit::new(wave(), SplineKnots::SquareRoot, Some(tangent), None, false).unwrap();
    let id = doc.add_fit_spline(&fit, 1.5).unwrap();
    let offset = Vector2::new(0.0, -2.0).unwrap();
    // Round (7,1) only.
    let areas = window(6.0, 0.0, 8.0, 2.0);
    let preview = doc.stretch_shapes(id, &areas, offset).unwrap();
    doc.stretch_entity_in_areas(id, &areas, offset).unwrap();
    let sp = codec(&doc, id);
    let moved: Vec<(f64, f64)> = sp.fit_points.iter().map(|q| (q.x, q.y)).collect();
    assert_eq!(
        moved,
        [(0.0, 0.0), (3.0, 4.0), (7.0, -1.0), (9.0, 6.0), (14.0, 5.0)]
    );
    assert!(sp.fit_points.iter().all(|q| q.z == 1.5));
    assert_eq!(
        (sp.knot_parameterization, sp.begin_tangent.x),
        (1, 1.0),
        "kept"
    );
    let curve = drawn(&doc, id);
    assert!(curve.contains_point(p(7.0, -1.0), arq_geometry::Tolerance::DEFAULT));
    let again = SplineFit::new(
        vec![
            p(0.0, 0.0),
            p(3.0, 4.0),
            p(7.0, -1.0),
            p(9.0, 6.0),
            p(14.0, 5.0),
        ],
        SplineKnots::SquareRoot,
        Some(tangent),
        None,
        false,
    )
    .unwrap()
    .spline()
    .unwrap();
    assert_eq!(curve.flatten(), again.flatten());
    // The preview is what was written.
    let [Shape::Spline(shown)] = &preview[..] else {
        panic!("{preview:?}")
    };
    assert_eq!(shown.flatten(), again.flatten());
    assert!(doc.check_model().is_empty(), "{:?}", doc.check_model());
}

#[test]
fn stretching_a_spline_by_vertices_moves_those_inside() {
    let mut doc = Document::new();
    let id = doc
        .add_spline(&Spline::by_vertices(3, wave(), false).unwrap(), 0.0)
        .unwrap();
    let offset = Vector2::new(1.0, 1.0).unwrap();
    doc.stretch_entity_in_areas(id, &window(8.0, 5.0, 10.0, 7.0), offset)
        .unwrap();
    let (s, _) = plan_spline(&doc, id);
    let expected = vec![
        p(0.0, 0.0),
        p(3.0, 4.0),
        p(7.0, 1.0),
        p(10.0, 7.0),
        p(14.0, 5.0),
    ];
    assert_eq!(s.points(), expected.as_slice());
    assert!(codec(&doc, id).fit_points.is_empty());
    // A closed one: a vertex it repeats at its end moves with its own, so
    // it stays closed and smooth.
    let closed = doc
        .add_spline(&Spline::by_vertices(3, wave(), true).unwrap(), 0.0)
        .unwrap();
    doc.stretch_entity_in_areas(closed, &window(-1.0, -1.0, 1.0, 1.0), offset)
        .unwrap();
    let (s, _) = plan_spline(&doc, closed);
    assert!(s.is_closed());
    let sp = codec(&doc, closed);
    assert!(
        sp.flags.closed && sp.flags.periodic,
        "written closed and periodic"
    );
    let n = s.points().len();
    assert_eq!(s.points()[0], p(1.0, 1.0));
    assert_eq!(s.points()[n - 3], p(1.0, 1.0), "its copy too");
    let (a, b) = s.domain();
    assert!(near(s.point_at(a), s.point_at(b)));
}

#[test]
fn a_spline_with_none_inside_stays_and_with_all_moves_whole() {
    let mut doc = Document::new();
    let fit = SplineFit::new(wave(), SplineKnots::Chord, None, None, false).unwrap();
    let id = doc.add_fit_spline(&fit, 0.0).unwrap();
    let before = codec(&doc, id);
    let offset = Vector2::new(5.0, 0.0).unwrap();
    doc.stretch_entity_in_areas(id, &window(20.0, 20.0, 30.0, 30.0), offset)
        .unwrap();
    assert_eq!(codec(&doc, id), before);
    doc.stretch_entity_in_areas(id, &window(-1.0, -1.0, 15.0, 7.0), offset)
        .unwrap();
    let sp = codec(&doc, id);
    assert_eq!(sp.fit_points[0].x, 5.0);
    assert_eq!(sp.fit_points.len(), 5, "moved whole, its fit kept");
    let (s, _) = plan_spline(&doc, id);
    assert!(near(s.ends().0, p(5.0, 0.0)));
}

#[test]
fn a_spline_s_grips_are_its_fit_points_or_its_own_vertices() {
    use crate::grips::GripKind;
    let mut doc = Document::new();
    let fit = SplineFit::new(wave(), SplineKnots::Chord, None, None, false).unwrap();
    let fitted = doc.add_fit_spline(&fit, 0.0).unwrap();
    let grips = doc.grips(&[fitted]);
    assert!(grips.iter().all(|g| g.kind == GripKind::FitPoint));
    assert_eq!(grips.iter().map(|g| g.point).collect::<Vec<_>>(), wave());
    assert_eq!(
        grips.iter().map(|g| g.vertex_index).collect::<Vec<_>>(),
        (0..5).map(Some).collect::<Vec<_>>()
    );
    let closed = doc
        .add_spline(&Spline::by_vertices(2, wave(), true).unwrap(), 0.0)
        .unwrap();
    let grips = doc.grips(&[closed]);
    assert_eq!(grips.len(), 5, "its two repeated left out");
    assert!(grips.iter().all(|g| g.kind == GripKind::ControlVertex));
    assert_eq!(
        (GripKind::FitPoint.name(), GripKind::ControlVertex.name()),
        ("FIT", "CV")
    );
}

#[test]
fn a_spline_s_point_moved_by_its_number() {
    let mut doc = Document::new();
    let tangent = Vector2::new(0.0, 1.0).unwrap();
    let fit = SplineFit::new(wave(), SplineKnots::Uniform, Some(tangent), None, false).unwrap();
    let id = doc.add_fit_spline(&fit, 2.0).unwrap();
    // Asked first, nothing changes.
    let before = codec(&doc, id);
    let (shown, kept, z) = doc.spline_with_point(id, 2, p(7.0, -1.0)).unwrap();
    assert_eq!(codec(&doc, id), before);
    assert_eq!(z, 2.0);
    let kept = kept.expect("its fit");
    assert_eq!(kept.points()[2], p(7.0, -1.0));
    assert_eq!(
        (kept.knots(), kept.start_tangent()),
        (SplineKnots::Uniform, Some(tangent))
    );
    doc.set_spline_point(id, 2, p(7.0, -1.0)).unwrap();
    assert_eq!(drawn(&doc, id).flatten(), shown.flatten());
    assert_eq!(codec(&doc, id).fit_points[2].y, -1.0);
    assert!(doc.check_model().is_empty());
    // Onto its neighbor, or past its points: refused, nothing changed.
    let now = codec(&doc, id);
    assert!(doc.set_spline_point(id, 3, p(7.0, -1.0)).is_err());
    assert!(doc.set_spline_point(id, 9, p(0.0, 0.0)).is_err());
    assert_eq!(codec(&doc, id), now);
    // By vertices: that vertex alone.
    let cv = doc
        .add_spline(&Spline::by_vertices(3, wave(), false).unwrap(), 0.0)
        .unwrap();
    doc.set_spline_point(cv, 4, p(15.0, 5.0)).unwrap();
    let (s, _) = plan_spline(&doc, cv);
    assert_eq!(s.points()[4], p(15.0, 5.0));
    assert_eq!(s.points()[..4], wave()[..4]);
    // Anything else is no spline.
    let layer = doc.current_layer().id();
    let line = arq_geometry::Line::new(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
    let other = doc
        .add_entity(crate::PlanGeometry::new(line.into()), layer)
        .unwrap();
    assert!(doc.spline_with_point(other, 0, p(0.0, 1.0)).is_err());
    assert!(doc.spline_fit(other).is_none());
}
