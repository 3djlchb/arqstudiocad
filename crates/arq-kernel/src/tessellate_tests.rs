//! Tests of a body's wireframe (a submodule of `tessellate`): a piece of
//! a cylinder's side built by hand, its isolines clipped to it.

use arq_geometry::{Point3, Tolerance, Vector3};

use crate::brep::{Body, BodyBuilder, Coedge, Edge, Face, Loop, Lump, Shell};
use crate::curve::{Curve, EllipseCurve, LineCurve};
use crate::surface::{Cone, Surface};

use super::{Isoline, clip};

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::new(x, y, z).unwrap()
}

fn v(x: f64, y: f64, z: f64) -> Vector3 {
    Vector3::new(x, y, z).unwrap()
}

fn at(degrees: f64, z: f64) -> Point3 {
    let a = degrees.to_radians();
    p(a.cos(), a.sin(), z)
}

/// The side of the unit cylinder around z between 10° and 100°, from
/// z = 0 to 1: a loop up the 100° line, back along the top, down the 10°
/// line and along the bottom, so the face is on its left seen from
/// outside. `reversed` turns the face inside out: the rest of the side.
fn sector(reversed: bool) -> Body {
    let mut b = BodyBuilder::new();
    let (a, bb, c, d) = (
        b.vertex(at(10.0, 0.0)),
        b.vertex(at(100.0, 0.0)),
        b.vertex(at(100.0, 1.0)),
        b.vertex(at(10.0, 1.0)),
    );
    let circle = |z: f64| {
        Curve::Ellipse(
            EllipseCurve::new(p(0.0, 0.0, z), v(0.0, 0.0, 1.0), v(1.0, 0.0, 0.0), 1.0).unwrap(),
        )
    };
    let line = |from: Point3, to: Point3| Curve::Line(LineCurve::new(from, to - from).unwrap());
    let bottom = b.edge(Edge {
        curve: Some(circle(0.0)),
        start: a,
        end: bb,
        reversed: false,
    });
    let up = b.edge(Edge {
        curve: Some(line(at(100.0, 0.0), at(100.0, 1.0))),
        start: bb,
        end: c,
        reversed: false,
    });
    // The top arc runs counterclockwise from 10° to 100°; the loop takes it back.
    let top = b.edge(Edge {
        curve: Some(circle(1.0)),
        start: d,
        end: c,
        reversed: false,
    });
    let down = b.edge(Edge {
        curve: Some(line(at(10.0, 1.0), at(10.0, 0.0))),
        start: d,
        end: a,
        reversed: false,
    });
    let coedges = vec![
        b.coedge(Coedge {
            edge: bottom,
            reversed: false,
        }),
        b.coedge(Coedge {
            edge: up,
            reversed: false,
        }),
        b.coedge(Coedge {
            edge: top,
            reversed: true,
        }),
        b.coedge(Coedge {
            edge: down,
            reversed: false,
        }),
    ];
    let lp = b.add_loop(Loop { coedges });
    let side = Cone::new(p(0.0, 0.0, 0.0), v(0.0, 0.0, 1.0), 1.0, 0.0, 1.0).unwrap();
    let face = b.face(Face {
        surface: Surface::Cone(side),
        loops: vec![lp],
        reversed,
    });
    let shell = b.shell(Shell { faces: vec![face] });
    b.lump(Lump {
        shells: vec![shell],
    });
    b.build(Tolerance::new(1e-9, 1e-10).unwrap()).unwrap()
}

/// The angle (degrees, rounded) of a vertical isoline, after checking it
/// runs the whole height on the cylinder.
fn angle_of(line: &[Point3]) -> i64 {
    let (first, last) = (line[0], line[line.len() - 1]);
    let (lo, hi) = (first.z().min(last.z()), first.z().max(last.z()));
    assert!(
        lo.abs() < 1e-6 && (hi - 1.0).abs() < 1e-6,
        "{first:?} {last:?}"
    );
    for q in line {
        assert!((q.x().hypot(q.y()) - 1.0).abs() < 1e-9, "on the cylinder");
    }
    (first.y().atan2(first.x()).to_degrees().round() as i64).rem_euclid(360)
}

#[test]
fn edges_are_sampled_within_the_chord() {
    let body = sector(false);
    let wire = body.wireframe(1e-4, 0);
    assert_eq!(wire.edges.len(), 4);
    assert!(wire.isolines.is_empty(), "no isolines asked for");
    let arc = wire
        .edges
        .iter()
        .find(|e| e.len() > 2)
        .expect("an arc sampled");
    assert_eq!(arc[0], at(10.0, 0.0), "from the edge's start vertex");
    for pair in arc.windows(2) {
        // A chord's middle strays from the circle by at most the chord.
        let middle = pair[0].midpoint(pair[1]);
        assert!(1.0 - middle.x().hypot(middle.y()) <= 1e-4);
    }
    let quarter_turn = arc.len() - 1;
    assert!(quarter_turn >= 2, "90° in more than one segment");
}

#[test]
fn isolines_stay_inside_their_face() {
    // Eight isolines, every 45°: 45° and 90° fall between 10° and 100°.
    let mut inside: Vec<i64> = sector(false)
        .wireframe(1e-3, 8)
        .isolines
        .iter()
        .map(|l| angle_of(l))
        .collect();
    inside.sort_unstable();
    assert_eq!(inside, [45, 90]);
    // Turned inside out, the face is the rest of the side, a large face
    // around a small hole: of its isolines only the one at 0°, passing near
    // the boundary, is drawn; those that never come near it are not
    // guessed.
    let mut rest: Vec<i64> = sector(true)
        .wireframe(1e-3, 8)
        .isolines
        .iter()
        .map(|l| angle_of(l))
        .collect();
    rest.sort_unstable();
    assert_eq!(rest, [0]);
}

#[test]
fn far_from_the_boundary_the_state_is_carried_not_guessed() {
    let line: Isoline = Box::new(|t| p(t, 0.0, 0.0));
    // Inside by the local rule all along, but never near the boundary:
    // the rule is not trusted, so nothing is drawn.
    let never_near = |_: Point3| (1.0, true);
    assert!(clip(&line, &never_near, 0.5).is_empty());
    // Near the boundary only at its start, where it is inside; farther the
    // rule says outside but is not trusted: the inside state is carried.
    let near_start = |q: Point3| {
        if q.x() < 0.1 {
            (0.05, true)
        } else {
            (1.0, false)
        }
    };
    let pieces = clip(&line, &near_start, 0.5);
    assert_eq!(pieces.len(), 1);
    assert_eq!(pieces[0][0], p(0.0, 0.0, 0.0));
    assert_eq!(pieces[0][pieces[0].len() - 1], p(1.0, 0.0, 0.0));
}

#[test]
fn a_cone_draws_its_isolines_up_to_its_apex() {
    // The side of a cone to a point is bounded by its base circle alone:
    // its apex, an edge without a curve, still sets how far they run.
    let cone = Body::cone(p(0.0, 0.0, 0.0), 2.0, 0.0, 4.0).unwrap();
    let wire = cone.wireframe(0.01, 4);
    assert_eq!(wire.isolines.len(), 4);
    for line in &wire.isolines {
        let (first, last) = (line[0], line[line.len() - 1]);
        let (low, high) = if first.z() < last.z() {
            (first, last)
        } else {
            (last, first)
        };
        assert!(
            low.z().abs() < 1e-9 && (high.z() - 4.0).abs() < 1e-9,
            "{low:?} {high:?}"
        );
        assert!(
            high.x().abs() < 1e-9 && high.y().abs() < 1e-9,
            "to the apex"
        );
    }
    // Upside down, from its apex at the base up to its top circle.
    let inverted = Body::cone(p(0.0, 0.0, 0.0), 0.0, 2.0, 4.0).unwrap();
    assert_eq!(inverted.wireframe(0.01, 4).isolines.len(), 4);
}

#[test]
fn a_ball_turned_about_x_draws_its_meridians_to_its_poles_there() {
    use std::f64::consts::{FRAC_PI_2, TAU};
    // A half disc of radius 2 turned about world X: its pole along X, as
    // AutoCAD draws it, its meridians from (-2, 0, 0) to (2, 0, 0).
    let arc = arq_geometry::Arc::new(
        arq_geometry::Point2::new(0.0, 0.0).unwrap(),
        2.0,
        -FRAC_PI_2,
        FRAC_PI_2,
    )
    .unwrap();
    let chord = arq_geometry::Line::new(
        arq_geometry::Point2::new(0.0, 2.0).unwrap(),
        arq_geometry::Point2::new(0.0, -2.0).unwrap(),
    )
    .unwrap();
    let half = [
        crate::ProfilePiece {
            curve: arq_geometry::Geometry2::Arc(arc),
            backward: false,
        },
        crate::ProfilePiece {
            curve: arq_geometry::Geometry2::Line(chord),
            backward: false,
        },
    ];
    let frame = crate::RevolutionFrame {
        origin: p(0.0, 0.0, 0.0),
        axis: v(1.0, 0.0, 0.0),
        radial: v(0.0, 1.0, 0.0),
    };
    let ball = Body::revolution(&half, frame, TAU).unwrap();
    // Meridians from pole to pole, and parallels square to X.
    let wire = ball.wireframe(0.01, 4);
    let (mut meridians, mut parallels) = (0, 0);
    for line in &wire.isolines {
        let (first, last) = (line[0], line[line.len() - 1]);
        if line.iter().all(|q| (q.x() - first.x()).abs() < 1e-9) {
            parallels += 1;
        } else {
            assert!((first.x().abs() - 2.0).abs() < 1e-9, "{first:?}");
            assert!((last.x() + first.x()).abs() < 1e-9, "{last:?}");
            meridians += 1;
        }
    }
    assert!(meridians > 0 && parallels > 0, "{meridians} {parallels}");
}

#[test]
fn the_nearest_guide_is_the_one_a_full_search_finds() {
    // A sector's curved face, its boundary drawn finely: hundreds of
    // guides in runs. Points on and around it, near corners too, where
    // guides tie: the runs give what visiting every guide in order gives,
    // the first of equals included.
    let body = sector(false);
    let size = 1e-3;
    let edges: Vec<Vec<Point3>> = body
        .edges()
        .iter()
        .map(|e| super::edge_polyline(&body, e, size))
        .collect();
    for face in body.faces() {
        let boundary = super::boundary_of(&body, face, &edges);
        let Some((guides, _)) = super::guides_of(face, &boundary) else {
            continue;
        };
        assert!(guides.guides.len() > 2 * super::Guides::RUN);
        let corners: Vec<Point3> = boundary.iter().map(|s| s.0).collect();
        let mut probes = corners.clone();
        for k in 0..400 {
            let t = k as f64 * 0.618_033_988_749_895;
            let c = corners[k * 7 % corners.len()];
            probes.push(c + v(t.sin(), (2.0 * t).cos(), (3.0 * t).sin()) * (0.5 * (t % 1.0)));
        }
        for q in probes {
            let mut best: Option<(f64, usize, Point3)> = None;
            for (i, g) in guides.guides.iter().enumerate() {
                let len2 = g.along.dot(g.along);
                if len2 == 0.0 {
                    continue;
                }
                let t = ((q - g.a).dot(g.along) / len2).clamp(0.0, 1.0);
                let r = g.a + g.along * t;
                let d = r.distance_squared_to(q);
                if best.is_none_or(|(b, _, _)| d < b) {
                    best = Some((d, i, r));
                }
            }
            let (d2, i, r) = best.unwrap();
            let expected = (d2.sqrt(), guides.guides[i].left.dot(q - r) > 0.0);
            assert_eq!(super::side_of(&guides, q), expected);
        }
    }
}
