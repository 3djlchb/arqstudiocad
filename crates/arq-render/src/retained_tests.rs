//! Tests of the retained geometry (a submodule of `retained`), on the CPU.

use arq_core::{CadDocument, Color, DrawItem, Shape};
use arq_geometry::{Bounds2, Point2};

use super::*;

const DARK: Style = Style {
    background: [33, 40, 48],
};

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

/// A 100 × 100 view of the origin at one pixel per unit.
fn camera() -> Camera {
    Camera::new(100, 100)
}

fn world(r: &Retained, v: &Vertex) -> (f64, f64) {
    (
        r.origin.0 + f64::from(v.at[0]),
        r.origin.1 + f64::from(v.at[1]),
    )
}

#[test]
fn a_line_is_two_vertices_in_its_color() {
    let mut cad = CadDocument::new();
    let (id, _) = cad.add_line(p(-30.0, 10.0), p(30.0, 10.0)).unwrap();
    cad.set_entity_color(&[id], Color::Index(1)).unwrap();
    let scene = Scene::build(cad.document());
    let r = Retained::build(&scene, &camera(), DARK).unwrap();
    assert_eq!(r.vertices.len(), 2);
    assert_eq!(world(&r, &r.vertices[0]), (-30.0, 10.0));
    assert_eq!(world(&r, &r.vertices[1]), (30.0, 10.0));
    assert!(r.vertices.iter().all(|v| v.rgba == [255, 0, 0, 255]));
    assert!(
        r.vertices
            .iter()
            .all(|v| v.along == SOLID && v.offset == [0.0, 0.0])
    );
}

#[test]
fn survey_coordinates_stay_small_and_exact() {
    let (x0, y0) = (500_000.123, 9_977_000.456);
    let mut cad = CadDocument::new();
    cad.add_line(p(x0 - 0.001, y0), p(x0 + 0.001, y0)).unwrap();
    let scene = Scene::build(cad.document());
    let mut cam = camera();
    cam.fit(Bounds2::new(
        p(x0 - 0.002, y0 - 0.002),
        p(x0 + 0.002, y0 + 0.002),
    ));
    let r = Retained::build(&scene, &cam, DARK).unwrap();
    assert_eq!(r.vertices.len(), 2);
    for (v, x) in r.vertices.iter().zip([x0 - 0.001, x0 + 0.001]) {
        assert!(v.at[0].abs() < 0.1 && v.at[1].abs() < 0.1, "{v:?} is small");
        let (wx, wy) = world(&r, v);
        // A ten-millionth of a millimetre on a coordinate of ten million.
        assert!((wx - x).abs() < 1e-9 && (wy - y0).abs() < 1e-9, "{wx} {wy}");
    }
}

#[test]
fn far_geometry_is_clipped_to_the_region() {
    let mut cad = CadDocument::new();
    cad.add_line(p(-1000.0, -1000.0), p(1000.0, 1000.0))
        .unwrap();
    cad.add_circle(p(10_000.0, 10_000.0), 1.0).unwrap();
    let scene = Scene::build(cad.document());
    let mut cam = camera();
    cam.zoom_at(50.0, 50.0, 1e6);
    let r = Retained::build(&scene, &cam, DARK).unwrap();
    assert_eq!(r.vertices.len(), 2, "the diagonal only, clipped");
    // The view is 1e-4 wide; the region nine views, so 9e-4.
    for v in &r.vertices {
        assert!(v.at[0].abs() <= 4.6e-4 && v.at[1].abs() <= 4.6e-4, "{v:?}");
    }
}

#[test]
fn points_dots_and_dashes() {
    let items = vec![
        DrawItem {
            shape: Shape::Point(p(10.0, 10.0)),
            color: Color::Index(2),
        },
        DrawItem {
            shape: Shape::Placeholder(Bounds2::new(p(-20.0, -20.0), p(0.0, -10.0))),
            color: Color::Index(4),
        },
    ];
    let scene = Scene::from_items(items);
    let r = Retained::build(&scene, &camera(), DARK).unwrap();
    let (point, dashed) = r.vertices.split_at(4);
    let offsets: Vec<_> = point.iter().map(|v| v.offset).collect();
    assert_eq!(
        offsets,
        [[-2.0, 0.0], [2.0, 0.0], [0.0, -2.0], [0.0, 2.0]],
        "a cross two pixels each way"
    );
    assert_eq!(dashed.len(), 8, "four dashed sides");
    for side in dashed.chunks(2) {
        assert_eq!(side[0].along, 0.0, "dashes start at each side's start");
        assert!(
            side[1].along == 10.0 || side[1].along == 20.0,
            "{:?}",
            side[1]
        );
    }

    assert!(r.dots.is_empty(), "nothing that small");

    // A circle far smaller than a pixel is a dot at its center, and so is
    // a segment that can be shorter than a pixel within the band (half the
    // scale: under two units here), at its middle.
    let mut cad = CadDocument::new();
    cad.add_circle(p(0.0, 0.0), 0.01).unwrap();
    cad.add_line(p(10.0, 10.0), p(11.5, 10.0)).unwrap();
    cad.add_line(p(20.0, 20.0), p(22.5, 20.0)).unwrap();
    let r = Retained::build(&Scene::build(cad.document()), &camera(), DARK).unwrap();
    assert_eq!(r.vertices.len(), 4, "both lines, not the circle");
    let dots: Vec<_> = r.dots.iter().map(|v| world(&r, v)).collect();
    assert_eq!(dots, [(0.0, 0.0), (10.75, 10.0)], "the long line has none");
}

#[test]
fn arcs_are_fine_enough_for_the_whole_band() {
    let mut cad = CadDocument::new();
    cad.add_circle(p(0.0, 0.0), 40.0).unwrap();
    let scene = Scene::build(cad.document());
    let r = Retained::build(&scene, &camera(), DARK).unwrap();
    // At twice the scale, each chord strays at most a quarter pixel.
    let scale = 2.0;
    for seg in r.vertices.chunks(2) {
        let (a, b) = (world(&r, &seg[0]), world(&r, &seg[1]));
        let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        let sagitta = 40.0 - mid.0.hypot(mid.1);
        assert!(sagitta * scale <= ARC_TOLERANCE_PX + 1e-9, "{sagitta}");
    }
}

#[test]
fn it_serves_until_the_scene_the_zoom_or_the_view_moves_on() {
    let mut cad = CadDocument::new();
    cad.add_line(p(-30.0, 0.0), p(30.0, 0.0)).unwrap();
    let mut scene = Scene::build(cad.document());
    let cam = camera();
    let r = Retained::build(&scene, &cam, DARK).unwrap();
    assert!(r.serves(&scene, &cam, DARK));

    let mut panned = cam;
    panned.pan(300.0, 0.0); // three views: still in the region
    assert!(r.serves(&scene, &panned, DARK));
    panned.pan(200.0, 0.0); // five views: out of it
    assert!(!r.serves(&scene, &panned, DARK));

    let mut zoomed = cam;
    zoomed.zoom_at(50.0, 50.0, 1.9);
    assert!(r.serves(&scene, &zoomed, DARK));
    zoomed.zoom_at(50.0, 50.0, 1.2);
    assert!(!r.serves(&scene, &zoomed, DARK), "past the band");
    let mut out = cam;
    out.zoom_at(50.0, 50.0, 1.0 / 2.1);
    assert!(!r.serves(&scene, &out, DARK));

    let light = Style {
        background: [250, 250, 250],
    };
    assert!(
        !r.serves(&scene, &cam, light),
        "colors depend on the background"
    );

    let (_, changes) = cad.add_circle(p(0.0, 0.0), 5.0).unwrap();
    assert!(scene.update(cad.document(), &changes));
    assert!(!r.serves(&scene, &cam, DARK), "the scene changed");
    let rebuilt = Scene::build(cad.document());
    assert!(!r.serves(&rebuilt, &cam, DARK), "another scene");
}

#[test]
fn only_the_real_ends_are_kept() {
    // A line's two ends and an arc's first and last points; a circle has
    // none, and the joints between chords are not ends.
    let mut cad = CadDocument::new();
    cad.add_line(p(-40.0, 0.0), p(-20.0, 0.0)).unwrap();
    cad.add_circle(p(0.0, 0.0), 10.0).unwrap();
    cad.add_arc(p(20.0, 0.0), 10.0, 0.0, std::f64::consts::FRAC_PI_2)
        .unwrap();
    let r = Retained::build(&Scene::build(cad.document()), &camera(), DARK).unwrap();
    let ends: Vec<_> = r
        .ends
        .iter()
        .map(|v| world(&r, v))
        .map(|(x, y)| ((x * 1e6).round() / 1e6, (y * 1e6).round() / 1e6))
        .collect();
    assert_eq!(
        ends,
        [(-40.0, 0.0), (-20.0, 0.0), (30.0, 0.0), (20.0, 10.0)]
    );
    assert!(r.vertices.len() > 20, "the chords themselves");
}

#[test]
fn a_band_is_kept_as_triangles_clipped_to_the_region() {
    let mut cad = CadDocument::new();
    let style = arq_core::PolylineStyle {
        width: 4.0,
        ..Default::default()
    };
    let (id, _) = cad
        .add_polyline_with(
            &[(p(-1000.0, 0.0), 0.0), (p(1000.0, 0.0), 0.0)],
            false,
            style,
        )
        .unwrap();
    cad.set_entity_color(&[id], Color::Index(1)).unwrap();
    let scene = Scene::build(cad.document());
    let r = Retained::build(&scene, &camera(), DARK).unwrap();
    assert!(!r.triangles.is_empty() && r.triangles.len().is_multiple_of(3));
    assert!(
        r.triangles
            .iter()
            .all(|v| v.rgba == [255, 0, 0, 255] && v.along == SOLID)
    );
    // The region is nine views of 100: nothing beyond ±450.
    for v in &r.triangles {
        let (x, y) = world(&r, v);
        assert!(
            x.abs() <= 450.0 + 1e-3 && y.abs() <= 2.0 + 1e-3,
            "({x}, {y})"
        );
    }
    assert_eq!(r.vertices.len(), 2, "its centerline is a line too");
}
