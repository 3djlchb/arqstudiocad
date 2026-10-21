//! Tests of the boundary representation (a submodule of `brep`): a
//! square sheet built by hand, then broken in each way the check must
//! refuse.

use arq_geometry::{Point3, Tolerance, Transform3, Vector3};

use super::*;
use crate::curve::{Curve, LineCurve};
use crate::surface::{Plane, Surface};

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::new(x, y, z).unwrap()
}

fn v(x: f64, y: f64, z: f64) -> Vector3 {
    Vector3::new(x, y, z).unwrap()
}

/// The unit square (0,0)-(1,1) at height `z` as one planar face; the
/// builder before `build`, so a test may break it first.
fn square(z: f64) -> BodyBuilder {
    let mut b = BodyBuilder::new();
    let corners = [
        p(0.0, 0.0, z),
        p(1.0, 0.0, z),
        p(1.0, 1.0, z),
        p(0.0, 1.0, z),
    ];
    let vertices: Vec<VertexId> = corners.iter().map(|&c| b.vertex(c)).collect();
    let coedges = (0..4)
        .map(|i| {
            let (a, c) = (corners[i], corners[(i + 1) % 4]);
            let edge = b.edge(Edge {
                curve: Some(Curve::Line(LineCurve::new(a, c - a).unwrap())),
                start: vertices[i],
                end: vertices[(i + 1) % 4],
                reversed: false,
            });
            b.coedge(Coedge {
                edge,
                reversed: false,
            })
        })
        .collect();
    let lp = b.add_loop(Loop { coedges });
    let face = b.face(Face {
        surface: Surface::Plane(Plane::new(p(0.0, 0.0, z), v(0.0, 0.0, 1.0)).unwrap()),
        loops: vec![lp],
        reversed: false,
    });
    let shell = b.shell(Shell { faces: vec![face] });
    b.lump(Lump {
        shells: vec![shell],
    });
    b
}

#[test]
fn a_square_sheet_is_a_body() {
    let body = square(0.0).build(Tolerance::DEFAULT).unwrap();
    assert_eq!(
        (body.lumps().len(), body.shells().len(), body.faces().len()),
        (1, 1, 1)
    );
    assert_eq!(
        (
            body.loops().len(),
            body.coedges().len(),
            body.edges().len(),
            body.vertices().len()
        ),
        (1, 4, 4, 4)
    );
    let b = body.vertex_bounds().unwrap();
    assert_eq!((b.min(), b.max()), (p(0.0, 0.0, 0.0), p(1.0, 1.0, 0.0)));
    let edge = body.edge(EdgeId(1));
    assert_eq!(body.vertex(edge.start).point, p(1.0, 0.0, 0.0));
}

#[test]
fn broken_parts_are_refused() {
    let tol = Tolerance::DEFAULT;
    assert_eq!(BodyBuilder::new().build(tol), Err(BrepError::Empty));
    // A loop running one edge backwards no longer closes.
    let mut b = square(0.0);
    b.body.coedges[1].reversed = true;
    assert_eq!(b.build(tol), Err(BrepError::OpenLoop(0)));
    // A vertex lifted off its edges' lines.
    let mut b = square(0.0);
    b.body.vertices[2].point = p(1.0, 1.0, 0.5);
    assert!(matches!(
        b.build(tol),
        Err(BrepError::VertexOffCurve { .. })
    ));
    // A face on a plane its vertices are not on.
    let mut b = square(0.0);
    b.body.faces[0].surface =
        Surface::Plane(Plane::new(p(0.0, 0.0, 1.0), v(0.0, 0.0, 1.0)).unwrap());
    assert!(matches!(
        b.build(tol),
        Err(BrepError::VertexOffSurface { face: 0, .. })
    ));
    // Indices naming nothing, and an empty loop.
    let mut b = square(0.0);
    b.body.edges[0].start = VertexId(9);
    assert_eq!(b.build(tol), Err(BrepError::Dangling("vertex")));
    let mut b = square(0.0);
    b.body.loops[0].coedges.clear();
    assert_eq!(b.build(tol), Err(BrepError::EmptyLoop(0)));
    let mut b = square(0.0);
    b.body.shells[0].faces.push(FaceId(3));
    assert_eq!(b.build(tol), Err(BrepError::Dangling("face")));
}

#[test]
fn a_body_moves_whole() {
    let body = square(0.0).build(Tolerance::DEFAULT).unwrap();
    let moved = body
        .transformed(Transform3::translation(v(0.0, 0.0, 2.0)), 1.0)
        .unwrap();
    assert_eq!(moved, square(2.0).build(Tolerance::DEFAULT).unwrap());
    moved.check(Tolerance::DEFAULT).unwrap();
}
