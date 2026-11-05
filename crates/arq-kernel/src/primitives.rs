//! Solids built from a few numbers (ADR 0024 step 4): the primitives the
//! 3D commands create. A box and a wedge so far.

use std::collections::HashMap;

use arq_geometry::{GeometryError, Point3, Tolerance};

use crate::brep::{Body, BodyBuilder, BrepError, Coedge, Edge, Face, Loop, Lump, Shell};
use crate::curve::{Curve, LineCurve};
use crate::surface::{Plane, Surface};

impl Body {
    /// A box with its edges along the axes, between two opposite
    /// corners, in either order: AutoCAD's BOX. A side of no length is
    /// [`GeometryError::ZeroLength`].
    pub fn cuboid(corner: Point3, opposite: Point3) -> Result<Body, BrepError> {
        let lo = [
            corner.x().min(opposite.x()),
            corner.y().min(opposite.y()),
            corner.z().min(opposite.z()),
        ];
        let hi = [
            corner.x().max(opposite.x()),
            corner.y().max(opposite.y()),
            corner.z().max(opposite.z()),
        ];
        if (0..3).any(|i| hi[i] <= lo[i]) {
            return Err(BrepError::Geometry(GeometryError::ZeroLength));
        }
        let at = |x: usize, y: usize, z: usize| {
            Point3::new([lo[0], hi[0]][x], [lo[1], hi[1]][y], [lo[2], hi[2]][z])
                .map_err(BrepError::Geometry)
        };
        let corners = [
            at(0, 0, 0)?,
            at(1, 0, 0)?,
            at(1, 1, 0)?,
            at(0, 1, 0)?,
            at(0, 0, 1)?,
            at(1, 0, 1)?,
            at(1, 1, 1)?,
            at(0, 1, 1)?,
        ];
        // Each face's corners counterclockwise seen from outside, so its
        // outside is the plane's normal: bottom, top, then round the sides.
        let faces: [&[usize]; 6] = [
            &[0, 3, 2, 1],
            &[4, 5, 6, 7],
            &[0, 1, 5, 4],
            &[1, 2, 6, 5],
            &[2, 3, 7, 6],
            &[3, 0, 4, 7],
        ];
        planar(&corners, &faces, tolerance(&corners)?)
    }

    /// A wedge in the box between two opposite corners, AutoCAD's WEDGE:
    /// its base at `first`'s height, as tall as `opposite`'s; its face at
    /// `first`'s X stands upright, and its sloped face, opposite `first`,
    /// comes down to the base at `opposite`'s X. A side of no length is
    /// [`GeometryError::ZeroLength`].
    pub fn wedge(first: Point3, opposite: Point3) -> Result<Body, BrepError> {
        let (x0, y0, z0) = (first.x(), first.y(), first.z());
        let (x1, y1, top) = (opposite.x(), opposite.y(), opposite.z());
        if x1 == x0 || y1 == y0 || top == z0 {
            return Err(BrepError::Geometry(GeometryError::ZeroLength));
        }
        let at = |x: f64, y: f64, z: f64| Point3::new(x, y, z).map_err(BrepError::Geometry);
        let corners = [
            at(x0, y0, z0)?,
            at(x1, y0, z0)?,
            at(x1, y1, z0)?,
            at(x0, y1, z0)?,
            at(x0, y0, top)?,
            at(x0, y1, top)?,
        ];
        // Counterclockwise from outside when X, Y and the height all run
        // forward: the base, the upright face, the slope, the two ends.
        let mut faces: Vec<Vec<usize>> = vec![
            vec![0, 3, 2, 1],
            vec![0, 4, 5, 3],
            vec![1, 2, 5, 4],
            vec![0, 1, 4],
            vec![3, 5, 2],
        ];
        // Each one running backward mirrors it: the corners turn the other
        // way round.
        let backward = [x1 < x0, y1 < y0, top < z0].iter().filter(|&&b| b).count();
        if backward % 2 == 1 {
            faces.iter_mut().for_each(|f| f.reverse());
        }
        let faces: Vec<&[usize]> = faces.iter().map(Vec::as_slice).collect();
        planar(&corners, &faces, tolerance(&corners)?)
    }
}

/// How close a vertex must be to its curves and surfaces: a billionth of
/// the farthest coordinate, at least of the body's size.
fn tolerance(points: &[Point3]) -> Result<Tolerance, BrepError> {
    let reach = points
        .iter()
        .flat_map(|p| [p.x().abs(), p.y().abs(), p.z().abs()])
        .fold(0.0, f64::max);
    let size = points
        .iter()
        .flat_map(|a| points.iter().map(move |b| (*a - *b).length()))
        .fold(0.0, f64::max);
    Tolerance::new(1e-9 * reach.max(size), Tolerance::DEFAULT.angular())
        .map_err(BrepError::Geometry)
}

/// A closed body of flat faces, each listing its corners counterclockwise
/// seen from outside; each pair of neighbours shares one straight edge.
fn planar(points: &[Point3], faces: &[&[usize]], tolerance: Tolerance) -> Result<Body, BrepError> {
    let mut b = BodyBuilder::new();
    let vertices: Vec<_> = points.iter().map(|&p| b.vertex(p)).collect();
    let mut edges = HashMap::new();
    let mut ids = Vec::with_capacity(faces.len());
    for &corners in faces {
        let mut coedges = Vec::with_capacity(corners.len());
        for k in 0..corners.len() {
            let (i, j) = (corners[k], corners[(k + 1) % corners.len()]);
            let key = (i.min(j), i.max(j));
            let edge = match edges.get(&key) {
                Some(&edge) => edge,
                None => {
                    let (from, to) = (points[key.0], points[key.1]);
                    let line = LineCurve::new(from, to - from).map_err(BrepError::Geometry)?;
                    let edge = b.edge(Edge {
                        curve: Some(Curve::Line(line)),
                        start: vertices[key.0],
                        end: vertices[key.1],
                        reversed: false,
                    });
                    edges.insert(key, edge);
                    edge
                }
            };
            coedges.push(b.coedge(Coedge {
                edge,
                reversed: i > j,
            }));
        }
        let lp = b.add_loop(Loop { coedges });
        let (a, c, d) = (points[corners[0]], points[corners[1]], points[corners[2]]);
        let plane = Plane::new(a, (c - a).cross(d - a)).map_err(BrepError::Geometry)?;
        ids.push(b.face(Face {
            surface: Surface::Plane(plane),
            loops: vec![lp],
            reversed: false,
        }));
    }
    let shell = b.shell(Shell { faces: ids });
    b.lump(Lump {
        shells: vec![shell],
    });
    b.build(tolerance)
}

#[cfg(test)]
#[path = "primitives_tests.rs"]
mod tests;
