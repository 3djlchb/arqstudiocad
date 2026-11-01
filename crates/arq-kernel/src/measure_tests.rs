//! Tests of measuring a body (a submodule of `measure`): solids built by
//! hand whose volume, area, centroid and bounds are known in closed form,
//! moved about to show the results follow.

use std::collections::HashMap;
use std::f64::consts::PI;

use arq_geometry::{Point3, Tolerance, Transform3, Vector3};

use crate::brep::{
    Body, BodyBuilder, Coedge, Edge, EdgeId, Face, Loop, LoopId, Lump, Shell, VertexId,
};
use crate::curve::{Curve, EllipseCurve, LineCurve};
use crate::nurbs::NurbsSurface;
use crate::surface::{Cone, Plane, Sphere, Surface, Torus};

use super::{MassProperties, MeasureError};

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::new(x, y, z).unwrap()
}

fn v(x: f64, y: f64, z: f64) -> Vector3 {
    Vector3::new(x, y, z).unwrap()
}

fn tol() -> Tolerance {
    Tolerance::new(1e-7, 1e-9).unwrap()
}

/// `a` and `b` agree to `rel` of `scale`.
#[track_caller]
fn close(a: f64, b: f64, scale: f64, rel: f64) {
    assert!((a - b).abs() <= rel * scale, "{a} != {b} (scale {scale})");
}

#[track_caller]
fn same_point(a: Point3, b: Point3, scale: f64) {
    for (x, y) in [(a.x(), b.x()), (a.y(), b.y()), (a.z(), b.z())] {
        close(x, y, scale, 1e-9);
    }
}

#[track_caller]
fn measured(body: &Body) -> MassProperties {
    body.mass_properties().expect("measurable")
}

/// A circle around `normal` through `center`, counterclockwise about it,
/// starting where the chart's frame starts.
fn circle(center: Point3, normal: Vector3, radius: f64) -> Curve {
    let (e1, _) = crate::chart::frame(normal.normalized().unwrap());
    Curve::Ellipse(EllipseCurve::new(center, normal, e1 * radius, 1.0).unwrap())
}

fn plane(origin: Point3, normal: Vector3) -> Surface {
    Surface::Plane(Plane::new(origin, normal).unwrap())
}

fn line(from: Point3, to: Point3) -> Curve {
    Curve::Line(LineCurve::new(from, to - from).unwrap())
}

/// A loop of edges, each used forward or reversed.
fn loop_of(b: &mut BodyBuilder, uses: &[(EdgeId, bool)]) -> LoopId {
    let coedges = uses
        .iter()
        .map(|&(edge, reversed)| b.coedge(Coedge { edge, reversed }))
        .collect();
    b.add_loop(Loop { coedges })
}

fn face(b: &mut BodyBuilder, surface: Surface, loops: Vec<LoopId>) -> crate::brep::FaceId {
    b.face(Face {
        surface,
        loops,
        reversed: false,
    })
}

fn closed_edge(b: &mut BodyBuilder, curve: Curve, at: VertexId) -> EdgeId {
    b.edge(Edge {
        curve: Some(curve),
        start: at,
        end: at,
        reversed: false,
    })
}

fn finish(mut b: BodyBuilder, shells: Vec<Vec<crate::brep::FaceId>>) -> Body {
    let shells = shells
        .into_iter()
        .map(|faces| b.shell(Shell { faces }))
        .collect();
    b.lump(Lump { shells });
    b.build(tol()).unwrap()
}

/// The flat faces of a body, added to `b`: `faces` lists vertex indices
/// counterclockwise seen from outside.
fn flat_faces(
    b: &mut BodyBuilder,
    points: &[Point3],
    faces: &[&[usize]],
) -> Vec<crate::brep::FaceId> {
    let ids: Vec<VertexId> = points.iter().map(|&q| b.vertex(q)).collect();
    let mut edges = HashMap::new();
    let mut out = Vec::new();
    for corners in faces {
        let mut uses = Vec::new();
        for k in 0..corners.len() {
            let (i, j) = (corners[k], corners[(k + 1) % corners.len()]);
            let key = (i.min(j), i.max(j));
            let edge = *edges.entry(key).or_insert_with(|| {
                b.edge(Edge {
                    curve: Some(line(points[key.0], points[key.1])),
                    start: ids[key.0],
                    end: ids[key.1],
                    reversed: false,
                })
            });
            uses.push((edge, i > j));
        }
        let lp = loop_of(b, &uses);
        let (a, c, d) = (points[corners[0]], points[corners[1]], points[corners[2]]);
        out.push(face(b, plane(a, (c - a).cross(d - a)), vec![lp]));
    }
    out
}

/// A box's corners from `lo`, of size `a × b × c`, and its faces.
fn box_parts(lo: Point3, a: f64, b: f64, c: f64) -> ([Point3; 8], [&'static [usize]; 6]) {
    let (x, y, z) = (lo.x(), lo.y(), lo.z());
    (
        [
            p(x, y, z),
            p(x + a, y, z),
            p(x + a, y + b, z),
            p(x, y + b, z),
            p(x, y, z + c),
            p(x + a, y, z + c),
            p(x + a, y + b, z + c),
            p(x, y + b, z + c),
        ],
        [
            &[0, 3, 2, 1],
            &[4, 5, 6, 7],
            &[0, 1, 5, 4],
            &[1, 2, 6, 5],
            &[2, 3, 7, 6],
            &[3, 0, 4, 7],
        ],
    )
}

fn cuboid(lo: Point3, a: f64, b: f64, c: f64) -> Body {
    let mut builder = BodyBuilder::new();
    let (points, faces) = box_parts(lo, a, b, c);
    let faces = flat_faces(&mut builder, &points, &faces);
    finish(builder, vec![faces])
}

#[test]
fn a_box() {
    let m = measured(&cuboid(p(1.0, -2.0, 3.0), 2.0, 3.0, 4.0));
    close(m.volume, 24.0, 24.0, 1e-12);
    close(m.area, 52.0, 52.0, 1e-12);
    same_point(m.centroid, p(2.0, -0.5, 5.0), 5.0);
    same_point(m.bounds.min(), p(1.0, -2.0, 3.0), 5.0);
    same_point(m.bounds.max(), p(3.0, 1.0, 7.0), 5.0);
}

/// A cylinder of `radius` and `height` standing on the origin along z.
fn cylinder(radius: f64, height: f64) -> Body {
    let z = v(0.0, 0.0, 1.0);
    let (start, _) = crate::chart::frame(z);
    let mut b = BodyBuilder::new();
    let lo = b.vertex(p(0.0, 0.0, 0.0) + start * radius);
    let hi = b.vertex(p(0.0, 0.0, height) + start * radius);
    let bottom = closed_edge(&mut b, circle(p(0.0, 0.0, 0.0), z, radius), lo);
    let top = closed_edge(&mut b, circle(p(0.0, 0.0, height), z, radius), hi);
    // Seen from outside: the bottom disk clockwise about z, the top one
    // counterclockwise; the side has the bottom forward, the top back.
    let under = loop_of(&mut b, &[(bottom, true)]);
    let over = loop_of(&mut b, &[(top, false)]);
    let low = loop_of(&mut b, &[(bottom, false)]);
    let high = loop_of(&mut b, &[(top, true)]);
    let side = Surface::Cone(Cone::new(p(0.0, 0.0, 0.0), z, radius, 0.0, 1.0).unwrap());
    let faces = vec![
        face(&mut b, plane(p(0.0, 0.0, 0.0), -z), vec![under]),
        face(&mut b, plane(p(0.0, 0.0, height), z), vec![over]),
        face(&mut b, side, vec![low, high]),
    ];
    finish(b, vec![faces])
}

#[test]
fn a_cylinder() {
    let (r, h) = (1.5, 2.0);
    let m = measured(&cylinder(r, h));
    close(m.volume, PI * r * r * h, 15.0, 1e-10);
    close(m.area, 2.0 * PI * r * (h + r), 35.0, 1e-10);
    same_point(m.centroid, p(0.0, 0.0, 1.0), 2.0);
    same_point(m.bounds.min(), p(-r, -r, 0.0), 2.0);
    same_point(m.bounds.max(), p(r, r, h), 2.0);
}

#[test]
fn moved_and_turned_it_measures_the_same() {
    let body = cylinder(1.5, 2.0);
    let axis = v(1.0, 2.0, 3.0).normalized().unwrap();
    let turn = Transform3::translation(v(100.0, -50.0, 1e4))
        .then(Transform3::rotation(axis, 37f64.to_radians()).unwrap());
    let moved = body.transformed(turn, 1.0).unwrap();
    let (a, b) = (measured(&body), measured(&moved));
    close(b.volume, a.volume, a.volume, 1e-10);
    close(b.area, a.area, a.area, 1e-10);
    same_point(b.centroid, turn.apply_point(a.centroid), 1e4);
}

/// A cone of base `radius` on the origin and apex at `height` on z.
fn cone(radius: f64, height: f64) -> Body {
    let z = v(0.0, 0.0, 1.0);
    let (start, _) = crate::chart::frame(z);
    let slant = radius.hypot(height);
    let mut b = BodyBuilder::new();
    let rim = b.vertex(p(0.0, 0.0, 0.0) + start * radius);
    let edge = closed_edge(&mut b, circle(p(0.0, 0.0, 0.0), z, radius), rim);
    let base = loop_of(&mut b, &[(edge, true)]);
    let side = loop_of(&mut b, &[(edge, false)]);
    let surface = Cone::new(p(0.0, 0.0, 0.0), z, radius, -radius / slant, height / slant).unwrap();
    let faces = vec![
        face(&mut b, plane(p(0.0, 0.0, 0.0), -z), vec![base]),
        face(&mut b, Surface::Cone(surface), vec![side]),
    ];
    finish(b, vec![faces])
}

#[test]
fn a_cone_to_its_apex() {
    let (r, h) = (2.0, 3.0);
    let m = measured(&cone(r, h));
    close(m.volume, PI * r * r * h / 3.0, 13.0, 1e-10);
    close(m.area, PI * r * (r.hypot(h) + r), 35.0, 1e-10);
    same_point(m.centroid, p(0.0, 0.0, h / 4.0), 3.0);
    // The apex is no vertex, and still bounds it.
    same_point(m.bounds.min(), p(-r, -r, 0.0), 3.0);
    same_point(m.bounds.max(), p(r, r, h), 3.0);
}

fn single_face_body(surface: Surface) -> Body {
    let mut b = BodyBuilder::new();
    let whole = face(&mut b, surface, vec![]);
    finish(b, vec![vec![whole]])
}

#[test]
fn a_whole_sphere() {
    let r = 2.0;
    let c = p(1.0, 2.0, 3.0);
    let m = measured(&single_face_body(Surface::Sphere(
        Sphere::new(c, r).unwrap(),
    )));
    close(m.volume, 4.0 / 3.0 * PI * r.powi(3), 34.0, 1e-10);
    close(m.area, 4.0 * PI * r * r, 50.0, 1e-10);
    same_point(m.centroid, c, 3.0);
    same_point(m.bounds.min(), p(-1.0, 0.0, 1.0), 3.0);
    same_point(m.bounds.max(), p(3.0, 4.0, 5.0), 3.0);
}

#[test]
fn a_whole_torus() {
    let (big, small) = (3.0, 1.0);
    let c = p(1.0, 2.0, 3.0);
    let axis = v(0.0, 1.0, 1.0).normalized().unwrap();
    let m = measured(&single_face_body(Surface::Torus(
        Torus::new(c, axis, big, small).unwrap(),
    )));
    close(m.volume, 2.0 * PI * PI * big * small * small, 60.0, 1e-10);
    close(m.area, 4.0 * PI * PI * big * small, 120.0, 1e-10);
    same_point(m.centroid, c, 4.0);
    // Along x, perpendicular to the axis: the outer equator.
    close(m.bounds.max().x(), c.x() + big + small, 4.0, 1e-9);
    // Along y, at 45° to the axis: the tube's circle tilted.
    close(
        m.bounds.max().y(),
        c.y() + big * 0.5f64.sqrt() + small,
        4.0,
        1e-9,
    );
}

/// The half ball above z = 0 and its flat face.
fn hemisphere(r: f64) -> Body {
    let z = v(0.0, 0.0, 1.0);
    let (start, _) = crate::chart::frame(z);
    let mut b = BodyBuilder::new();
    let rim = b.vertex(p(0.0, 0.0, 0.0) + start * r);
    let edge = closed_edge(&mut b, circle(p(0.0, 0.0, 0.0), z, r), rim);
    let flat = loop_of(&mut b, &[(edge, true)]);
    let dome = loop_of(&mut b, &[(edge, false)]);
    let faces = vec![
        face(&mut b, plane(p(0.0, 0.0, 0.0), -z), vec![flat]),
        face(
            &mut b,
            Surface::Sphere(Sphere::new(p(0.0, 0.0, 0.0), r).unwrap()),
            vec![dome],
        ),
    ];
    finish(b, vec![faces])
}

#[test]
fn a_hemisphere_holds_one_pole() {
    let r = 2.0;
    let m = measured(&hemisphere(r));
    close(m.volume, 2.0 / 3.0 * PI * r.powi(3), 17.0, 1e-10);
    close(m.area, 3.0 * PI * r * r, 38.0, 1e-10);
    same_point(m.centroid, p(0.0, 0.0, 3.0 * r / 8.0), 2.0);
    same_point(m.bounds.min(), p(-r, -r, 0.0), 2.0);
    same_point(m.bounds.max(), p(r, r, r), 2.0);
    // Upside down, it holds the other pole.
    let flip = Transform3::rotation(v(1.0, 0.0, 0.0), PI).unwrap();
    let m = measured(&hemisphere(r).transformed(flip, 1.0).unwrap());
    close(m.volume, 2.0 / 3.0 * PI * r.powi(3), 17.0, 1e-10);
    same_point(m.centroid, p(0.0, 0.0, -3.0 * r / 8.0), 2.0);
    close(m.bounds.min().z(), -r, 2.0, 1e-9);
}

/// A ball of radius `r` cut by the plane x = `x0`: its sphere face holds
/// both poles around a loop that does not wind round the axis.
fn cut_ball(r: f64, x0: f64) -> Body {
    let x = v(1.0, 0.0, 0.0);
    let (start, _) = crate::chart::frame(x);
    let rho = (r * r - x0 * x0).sqrt();
    let mut b = BodyBuilder::new();
    let rim = b.vertex(p(x0, 0.0, 0.0) + start * rho);
    let edge = closed_edge(&mut b, circle(p(x0, 0.0, 0.0), x, rho), rim);
    let flat = loop_of(&mut b, &[(edge, false)]);
    let round = loop_of(&mut b, &[(edge, true)]);
    let faces = vec![
        face(&mut b, plane(p(x0, 0.0, 0.0), x), vec![flat]),
        face(
            &mut b,
            Surface::Sphere(Sphere::new(p(0.0, 0.0, 0.0), r).unwrap()),
            vec![round],
        ),
    ];
    finish(b, vec![faces])
}

#[test]
fn a_ball_cut_off_its_axis_holds_both_poles() {
    let (r, x0) = (2.0, 1.2);
    let h = r - x0;
    let cap = PI * h * h * (3.0 * r - h) / 3.0;
    let ball = 4.0 / 3.0 * PI * r.powi(3);
    let m = measured(&cut_ball(r, x0));
    close(m.volume, ball - cap, 34.0, 1e-10);
    close(
        m.area,
        4.0 * PI * r * r - 2.0 * PI * r * h + PI * (r * r - x0 * x0),
        50.0,
        1e-10,
    );
    // The cap's centroid, from the center.
    let cap_x = 3.0 * (2.0 * r - h).powi(2) / (4.0 * (3.0 * r - h));
    same_point(m.centroid, p(-cap * cap_x / (ball - cap), 0.0, 0.0), 2.0);
    close(m.bounds.max().x(), x0, 2.0, 1e-9);
    same_point(m.bounds.min(), p(-r, -r, -r), 2.0);
    close(m.bounds.max().z(), r, 2.0, 1e-9);
}

#[test]
fn a_void_is_taken_out() {
    // A box with a ball-shaped void, whose face points into it.
    let mut b = BodyBuilder::new();
    let (points, faces) = box_parts(p(0.0, 0.0, 0.0), 4.0, 4.0, 4.0);
    let outer = flat_faces(&mut b, &points, &faces);
    let void = b.face(Face {
        surface: Surface::Sphere(Sphere::new(p(2.0, 2.0, 2.0), 1.0).unwrap()),
        loops: vec![],
        reversed: true,
    });
    let m = measured(&finish(b, vec![outer, vec![void]]));
    close(m.volume, 64.0 - 4.0 / 3.0 * PI, 64.0, 1e-10);
    close(m.area, 96.0 + 4.0 * PI, 110.0, 1e-10);
    same_point(m.centroid, p(2.0, 2.0, 2.0), 4.0);
}

/// Half a cylinder (y ≥ 0): its curved face is a piece of a closed
/// surface whose loop does not wind around it.
fn half_cylinder(r: f64, h: f64) -> Body {
    let z = v(0.0, 0.0, 1.0);
    let mut b = BodyBuilder::new();
    let (a0, b0, a1, b1) = (p(r, 0.0, 0.0), p(-r, 0.0, 0.0), p(r, 0.0, h), p(-r, 0.0, h));
    let (va0, vb0, va1, vb1) = (b.vertex(a0), b.vertex(b0), b.vertex(a1), b.vertex(b1));
    let arc = |zc: f64| {
        Curve::Ellipse(EllipseCurve::new(p(0.0, 0.0, zc), z, v(r, 0.0, 0.0), 1.0).unwrap())
    };
    let edge = |b: &mut BodyBuilder, curve, start, end| {
        b.edge(Edge {
            curve: Some(curve),
            start,
            end,
            reversed: false,
        })
    };
    // Arcs counterclockwise from +x to −x, through +y.
    let low = edge(&mut b, arc(0.0), va0, vb0);
    let high = edge(&mut b, arc(h), va1, vb1);
    let right = edge(&mut b, line(a0, a1), va0, va1);
    let left = edge(&mut b, line(b0, b1), vb0, vb1);
    let floor = edge(&mut b, line(a0, b0), va0, vb0);
    let roof = edge(&mut b, line(b1, a1), vb1, va1);
    let side = loop_of(
        &mut b,
        &[(low, false), (left, false), (high, true), (right, true)],
    );
    let under = loop_of(&mut b, &[(low, true), (floor, false)]);
    let over = loop_of(&mut b, &[(high, false), (roof, false)]);
    let back = loop_of(
        &mut b,
        &[(floor, true), (right, false), (roof, true), (left, true)],
    );
    let surface = Surface::Cone(Cone::new(p(0.0, 0.0, 0.0), z, r, 0.0, 1.0).unwrap());
    let faces = vec![
        face(&mut b, surface, vec![side]),
        face(&mut b, plane(a0, -z), vec![under]),
        face(&mut b, plane(a1, z), vec![over]),
        face(&mut b, plane(a0, v(0.0, -1.0, 0.0)), vec![back]),
    ];
    finish(b, vec![faces])
}

#[test]
fn half_a_cylinder() {
    let (r, h) = (2.0, 3.0);
    let m = measured(&half_cylinder(r, h));
    close(m.volume, PI * r * r * h / 2.0, 19.0, 1e-10);
    close(m.area, PI * r * h + PI * r * r + 2.0 * r * h, 45.0, 1e-10);
    same_point(m.centroid, p(0.0, 4.0 * r / (3.0 * PI), h / 2.0), 3.0);
    same_point(m.bounds.min(), p(-r, 0.0, 0.0), 3.0);
    same_point(m.bounds.max(), p(r, r, h), 3.0);
}

#[test]
fn a_nurbs_face_trimmed_by_its_loop() {
    // A box whose top lies on a bilinear NURBS patch wider than the face.
    let mut b = BodyBuilder::new();
    let (points, faces) = box_parts(p(0.0, 0.0, 0.0), 2.0, 3.0, 4.0);
    let mut ids = flat_faces(&mut b, &points, &faces);
    let top = ids.remove(1);
    let mut body_faces: Vec<crate::brep::FaceId> = ids;
    let patch = NurbsSurface::new(
        1,
        1,
        vec![0.0, 0.0, 1.0, 1.0],
        vec![0.0, 0.0, 1.0, 1.0],
        2,
        2,
        vec![
            p(-1.0, -1.0, 4.0),
            p(-1.0, 5.0, 4.0),
            p(4.0, -1.0, 4.0),
            p(4.0, 5.0, 4.0),
        ],
        vec![1.0; 4],
    )
    .unwrap();
    // The flat top's loop, on the patch instead.
    let replaced = b.face_mut(top);
    replaced.surface = Surface::Nurbs(patch);
    body_faces.insert(1, top);
    let m = measured(&finish(b, vec![body_faces]));
    close(m.volume, 24.0, 24.0, 1e-10);
    close(m.area, 52.0, 52.0, 1e-10);
    same_point(m.centroid, p(1.0, 1.5, 2.0), 4.0);
}

#[test]
fn an_open_body_is_not_measured() {
    // A box without its top.
    let mut b = BodyBuilder::new();
    let (points, faces) = box_parts(p(0.0, 0.0, 0.0), 1.0, 1.0, 1.0);
    let mut ids = flat_faces(&mut b, &points, &faces);
    ids.remove(1);
    let body = finish(b, vec![ids]);
    assert!(matches!(
        body.mass_properties(),
        Err(MeasureError::NotClosed { .. })
    ));
}

/// An elliptic cylinder of semi-axes `a` (along x) and `b`, `height` up z.
fn elliptic_cylinder(a: f64, b: f64, height: f64) -> Body {
    let z = v(0.0, 0.0, 1.0);
    let section = |zc: f64| EllipseCurve::new(p(0.0, 0.0, zc), z, v(a, 0.0, 0.0), b / a).unwrap();
    let mut builder = BodyBuilder::new();
    let lo = builder.vertex(p(a, 0.0, 0.0));
    let hi = builder.vertex(p(a, 0.0, height));
    let bottom = closed_edge(&mut builder, Curve::Ellipse(section(0.0)), lo);
    let top = closed_edge(&mut builder, Curve::Ellipse(section(height)), hi);
    let under = loop_of(&mut builder, &[(bottom, true)]);
    let over = loop_of(&mut builder, &[(top, false)]);
    let low = loop_of(&mut builder, &[(bottom, false)]);
    let high = loop_of(&mut builder, &[(top, true)]);
    let side = Surface::EllipticCylinder(crate::surface::EllipticCylinder::new(section(0.0)));
    let faces = vec![
        face(&mut builder, plane(p(0.0, 0.0, 0.0), -z), vec![under]),
        face(&mut builder, plane(p(0.0, 0.0, height), z), vec![over]),
        face(&mut builder, side, vec![low, high]),
    ];
    finish(builder, vec![faces])
}

/// An ellipse's perimeter, by the Gauss–Kummer series.
fn perimeter(a: f64, b: f64) -> f64 {
    let h = ((a - b) / (a + b)).powi(2);
    let (mut sum, mut coefficient, mut power) = (1.0, 1.0, 1.0);
    for n in 1..60 {
        // binom(1/2, n), squared.
        coefficient *= (0.5 - (n - 1) as f64) / n as f64;
        power *= h;
        sum += coefficient * coefficient * power;
    }
    PI * (a + b) * sum
}

#[test]
fn an_elliptic_cylinder() {
    let (a, b, h) = (3.0, 1.5, 2.0);
    let m = measured(&elliptic_cylinder(a, b, h));
    close(m.volume, PI * a * b * h, 30.0, 1e-10);
    close(m.area, 2.0 * PI * a * b + h * perimeter(a, b), 60.0, 1e-10);
    same_point(m.centroid, p(0.0, 0.0, 1.0), 3.0);
    same_point(m.bounds.min(), p(-a, -b, 0.0), 3.0);
    same_point(m.bounds.max(), p(a, b, h), 3.0);
}

#[test]
fn an_arc_running_against_its_curve() {
    // The half cylinder with its top arc drawn clockwise: the edge runs
    // against its curve.
    let (r, h) = (2.0, 3.0);
    let z = v(0.0, 0.0, 1.0);
    let mut b = BodyBuilder::new();
    let (a0, b0, a1, b1) = (p(r, 0.0, 0.0), p(-r, 0.0, 0.0), p(r, 0.0, h), p(-r, 0.0, h));
    let (va0, vb0, va1, vb1) = (b.vertex(a0), b.vertex(b0), b.vertex(a1), b.vertex(b1));
    let arc = |zc: f64, normal: Vector3| {
        Curve::Ellipse(EllipseCurve::new(p(0.0, 0.0, zc), normal, v(r, 0.0, 0.0), 1.0).unwrap())
    };
    let edge = |b: &mut BodyBuilder, curve, start, end, reversed| {
        b.edge(Edge {
            curve: Some(curve),
            start,
            end,
            reversed,
        })
    };
    let low = edge(&mut b, arc(0.0, z), va0, vb0, false);
    let high = edge(&mut b, arc(h, -z), va1, vb1, true);
    let right = edge(&mut b, line(a0, a1), va0, va1, false);
    let left = edge(&mut b, line(b0, b1), vb0, vb1, false);
    let floor = edge(&mut b, line(a0, b0), va0, vb0, false);
    let roof = edge(&mut b, line(b1, a1), vb1, va1, false);
    let side = loop_of(
        &mut b,
        &[(low, false), (left, false), (high, true), (right, true)],
    );
    let under = loop_of(&mut b, &[(low, true), (floor, false)]);
    let over = loop_of(&mut b, &[(high, false), (roof, false)]);
    let back = loop_of(
        &mut b,
        &[(floor, true), (right, false), (roof, true), (left, true)],
    );
    let surface = Surface::Cone(Cone::new(p(0.0, 0.0, 0.0), z, r, 0.0, 1.0).unwrap());
    let faces = vec![
        face(&mut b, surface, vec![side]),
        face(&mut b, plane(a0, -z), vec![under]),
        face(&mut b, plane(a1, z), vec![over]),
        face(&mut b, plane(a0, v(0.0, -1.0, 0.0)), vec![back]),
    ];
    let m = measured(&finish(b, vec![faces]));
    close(m.volume, PI * r * r * h / 2.0, 19.0, 1e-10);
    same_point(m.centroid, p(0.0, 4.0 * r / (3.0 * PI), h / 2.0), 3.0);
    close(m.bounds.max().y(), r, 3.0, 1e-9);
}

/// The torus around z of radii `big` and `small`, above z = 0: its face is
/// bounded by the outer and inner equators, which wind around the axis.
fn upper_torus(big: f64, small: f64) -> Body {
    let z = v(0.0, 0.0, 1.0);
    let (start, _) = crate::chart::frame(z);
    let mut b = BodyBuilder::new();
    let (outer_at, inner_at) = (
        b.vertex(p(0.0, 0.0, 0.0) + start * (big + small)),
        b.vertex(p(0.0, 0.0, 0.0) + start * (big - small)),
    );
    let outer = closed_edge(&mut b, circle(p(0.0, 0.0, 0.0), z, big + small), outer_at);
    let inner = closed_edge(&mut b, circle(p(0.0, 0.0, 0.0), z, big - small), inner_at);
    // The ring below, facing down: clockwise outside, counterclockwise in.
    let ring = [
        loop_of(&mut b, &[(outer, true)]),
        loop_of(&mut b, &[(inner, false)]),
    ];
    // The torus above: the outer equator counterclockwise, the inner back.
    let tube = [
        loop_of(&mut b, &[(outer, false)]),
        loop_of(&mut b, &[(inner, true)]),
    ];
    let torus = Surface::Torus(Torus::new(p(0.0, 0.0, 0.0), z, big, small).unwrap());
    let faces = vec![
        face(&mut b, plane(p(0.0, 0.0, 0.0), -z), ring.to_vec()),
        face(&mut b, torus, tube.to_vec()),
    ];
    finish(b, vec![faces])
}

#[test]
fn half_a_torus_either_way_up() {
    let (big, small) = (3.0, 1.0);
    let volume = PI * PI * big * small * small;
    // The half tube's: the ring's width weighs both sides alike.
    let rise = 4.0 * small / (3.0 * PI);
    let m = measured(&upper_torus(big, small));
    close(m.volume, volume, 30.0, 1e-10);
    close(
        m.area,
        2.0 * PI * PI * big * small + PI * ((big + small).powi(2) - (big - small).powi(2)),
        100.0,
        1e-10,
    );
    same_point(m.centroid, p(0.0, 0.0, rise), 4.0);
    same_point(m.bounds.max(), p(big + small, big + small, small), 4.0);
    let flip = Transform3::rotation(v(1.0, 0.0, 0.0), PI).unwrap();
    let m = measured(&upper_torus(big, small).transformed(flip, 1.0).unwrap());
    close(m.volume, volume, 30.0, 1e-10);
    same_point(m.centroid, p(0.0, 0.0, -rise), 4.0);
    close(m.bounds.min().z(), -small, 4.0, 1e-9);
}

/// A cone cut short: radius `r0` at z = 0, `r1` at `height`.
fn frustum(r0: f64, r1: f64, height: f64) -> Body {
    let z = v(0.0, 0.0, 1.0);
    let (start, _) = crate::chart::frame(z);
    let slant = (r0 - r1).hypot(height);
    let mut b = BodyBuilder::new();
    let lo = b.vertex(p(0.0, 0.0, 0.0) + start * r0);
    let hi = b.vertex(p(0.0, 0.0, height) + start * r1);
    let bottom = closed_edge(&mut b, circle(p(0.0, 0.0, 0.0), z, r0), lo);
    let top = closed_edge(&mut b, circle(p(0.0, 0.0, height), z, r1), hi);
    let under = loop_of(&mut b, &[(bottom, true)]);
    let over = loop_of(&mut b, &[(top, false)]);
    let low = loop_of(&mut b, &[(bottom, false)]);
    let high = loop_of(&mut b, &[(top, true)]);
    let side = Cone::new(p(0.0, 0.0, 0.0), z, r0, (r1 - r0) / slant, height / slant).unwrap();
    let faces = vec![
        face(&mut b, plane(p(0.0, 0.0, 0.0), -z), vec![under]),
        face(&mut b, plane(p(0.0, 0.0, height), z), vec![over]),
        face(&mut b, Surface::Cone(side), vec![low, high]),
    ];
    finish(b, vec![faces])
}

#[test]
fn a_frustum() {
    let (r0, r1, h) = (2.0, 1.0, 1.5);
    let m = measured(&frustum(r0, r1, h));
    let volume = PI * h / 3.0 * (r0 * r0 + r0 * r1 + r1 * r1);
    close(m.volume, volume, 12.0, 1e-10);
    close(
        m.area,
        PI * (r0 + r1) * (r0 - r1).hypot(h) + PI * (r0 * r0 + r1 * r1),
        30.0,
        1e-10,
    );
    let z = h * (r0 * r0 + 2.0 * r0 * r1 + 3.0 * r1 * r1) / (4.0 * (r0 * r0 + r0 * r1 + r1 * r1));
    same_point(m.centroid, p(0.0, 0.0, z), 2.0);
}

#[test]
fn a_rim_drawn_as_a_spline() {
    // A cylinder whose bottom rim is a rational spline circle: its
    // parameter is not the angle, so the rules refine before settling.
    let (r, h) = (2.0, 3.0);
    let z = v(0.0, 0.0, 1.0);
    let w = 0.5f64.sqrt();
    let ring = [
        (1.0, 0.0, 1.0),
        (1.0, 1.0, w),
        (0.0, 1.0, 1.0),
        (-1.0, 1.0, w),
        (-1.0, 0.0, 1.0),
        (-1.0, -1.0, w),
        (0.0, -1.0, 1.0),
        (1.0, -1.0, w),
        (1.0, 0.0, 1.0),
    ];
    let spline = crate::nurbs::NurbsCurve::new(
        2,
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 4.0],
        ring.iter().map(|&(x, y, _)| p(r * x, r * y, 0.0)).collect(),
        ring.iter().map(|&(_, _, wt)| wt).collect(),
    )
    .unwrap();
    let mut b = BodyBuilder::new();
    let lo = b.vertex(p(r, 0.0, 0.0));
    let hi = b.vertex(p(r, 0.0, h));
    let bottom = closed_edge(&mut b, Curve::Nurbs(spline), lo);
    let top = closed_edge(
        &mut b,
        Curve::Ellipse(EllipseCurve::new(p(0.0, 0.0, h), z, v(r, 0.0, 0.0), 1.0).unwrap()),
        hi,
    );
    let under = loop_of(&mut b, &[(bottom, true)]);
    let over = loop_of(&mut b, &[(top, false)]);
    let low = loop_of(&mut b, &[(bottom, false)]);
    let high = loop_of(&mut b, &[(top, true)]);
    let side = Surface::Cone(Cone::new(p(0.0, 0.0, 0.0), z, r, 0.0, 1.0).unwrap());
    let faces = vec![
        face(&mut b, plane(p(0.0, 0.0, 0.0), -z), vec![under]),
        face(&mut b, plane(p(0.0, 0.0, h), z), vec![over]),
        face(&mut b, side, vec![low, high]),
    ];
    let m = measured(&finish(b, vec![faces]));
    close(m.volume, PI * r * r * h, 40.0, 1e-10);
    close(m.area, 2.0 * PI * r * (h + r), 65.0, 1e-10);
    same_point(m.centroid, p(0.0, 0.0, h / 2.0), 3.0);
}

#[test]
fn the_bounds_follow_what_a_cut_leaves() {
    // Cut by x = x0, a ball keeps its poles only when x0 is past them.
    let r = 2.0;
    for x0 in [-1.6, -0.8, 0.3, 1.1, 1.7] {
        let m = measured(&cut_ball(r, x0));
        let top = if x0 >= 0.0 {
            r
        } else {
            (r * r - x0 * x0).sqrt()
        };
        close(m.bounds.max().z(), top, 2.0, 1e-9);
        close(m.bounds.min().z(), -top, 2.0, 1e-9);
        close(m.bounds.min().x(), -r, 2.0, 1e-9);
        close(m.bounds.max().x(), x0, 2.0, 1e-9);
    }
}

/// The piece of a torus around z between the angles `from` and `to`
/// around the axis, closed by its two round sections: the torus face's
/// loops wind around the tube.
fn torus_piece(big: f64, small: f64, from: f64, to: f64) -> Body {
    let z = v(0.0, 0.0, 1.0);
    let (e1, e2) = crate::chart::frame(z);
    let dir = |u: f64| e1 * u.cos() + e2 * u.sin();
    let turn = |u: f64| e2 * u.cos() - e1 * u.sin();
    let mut b = BodyBuilder::new();
    let origin = p(0.0, 0.0, 0.0);
    let section = |b: &mut BodyBuilder, u: f64| {
        let center = origin + dir(u) * big;
        let at = b.vertex(center + dir(u) * small);
        let curve = EllipseCurve::new(center, turn(u), dir(u) * small, 1.0).unwrap();
        (closed_edge(b, Curve::Ellipse(curve), at), center)
    };
    let (start, start_center) = section(&mut b, from);
    let (end, end_center) = section(&mut b, to);
    // The end section faces on around the axis, the start one back.
    let end_disk = loop_of(&mut b, &[(end, false)]);
    let start_disk = loop_of(&mut b, &[(start, true)]);
    let tube = [
        loop_of(&mut b, &[(end, true)]),
        loop_of(&mut b, &[(start, false)]),
    ];
    let torus = Surface::Torus(Torus::new(origin, z, big, small).unwrap());
    let faces = vec![
        face(&mut b, plane(end_center, turn(to)), vec![end_disk]),
        face(&mut b, plane(start_center, -turn(from)), vec![start_disk]),
        face(&mut b, torus, tube.to_vec()),
    ];
    finish(b, vec![faces])
}

#[test]
fn pieces_of_a_torus_small_and_large() {
    let (big, small) = (3.0, 1.0);
    for (from, to) in [(0.3, 0.3 + PI / 2.0), (0.3, 0.3 + 1.5 * PI), (-2.0, 2.5)] {
        let share = (to - from) / (2.0 * PI);
        let m = measured(&torus_piece(big, small, from, to));
        close(
            m.volume,
            share * 2.0 * PI * PI * big * small * small,
            60.0,
            1e-10,
        );
        let area = share * 4.0 * PI * PI * big * small + 2.0 * PI * small * small;
        close(m.area, area, 120.0, 1e-10);
    }
}
