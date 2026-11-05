//! Solids built from a few numbers (ADR 0024 step 4): the primitives the
//! 3D commands create: a box, a wedge, a cylinder and a cone so far.

use std::collections::HashMap;

use arq_geometry::{GeometryError, Point3, Tolerance, Vector3};

use crate::brep::{Body, BodyBuilder, BrepError, Coedge, Edge, EdgeId, Face, Loop, Lump, Shell};
use crate::curve::{Curve, EllipseCurve, LineCurve};
use crate::surface::{Cone, Plane, Surface};

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

impl Body {
    /// A cylinder standing on its base circle, centered at `center` with
    /// `radius`, `height` along Z (negative: downward): AutoCAD's
    /// CYLINDER.
    pub fn cylinder(center: Point3, radius: f64, height: f64) -> Result<Body, BrepError> {
        Self::cone(center, radius, radius, height)
    }

    /// A cone standing on its base circle, centered at `center` with
    /// radius `base`, `height` along Z (negative: downward), its top circle
    /// of radius `top`: AutoCAD's CONE. A radius of 0 is a point (the
    /// apex), not both; equal radii make a cylinder. A radius below 0 is
    /// [`GeometryError::InvalidRadius`], a height of 0
    /// [`GeometryError::ZeroLength`].
    pub fn cone(center: Point3, base: f64, top: f64, height: f64) -> Result<Body, BrepError> {
        let radii_ok = base.is_finite() && top.is_finite() && base >= 0.0 && top >= 0.0;
        if !radii_ok || base.max(top) == 0.0 {
            return Err(BrepError::Geometry(GeometryError::InvalidRadius));
        }
        if !height.is_finite() || height == 0.0 {
            return Err(BrepError::Geometry(GeometryError::ZeroLength));
        }
        let geometry = BrepError::Geometry;
        let axis = Vector3::new(0.0, 0.0, height.signum()).map_err(geometry)?;
        let across = Vector3::new(1.0, 0.0, 0.0).map_err(geometry)?;
        let rise = height.abs();
        let top_center = center + axis * rise;
        // The side, from the larger end so its reference circle has a size:
        // widening by (top - base) per rise along the axis.
        let slant = (top - base).hypot(rise);
        let (origin, radius) = if base >= top {
            (center, base)
        } else {
            (top_center, top)
        };
        let side = Cone::new(origin, axis, radius, (top - base) / slant, rise / slant)
            .map_err(geometry)?;

        let mut b = BodyBuilder::new();
        let mut points = vec![center, top_center];
        // An end: a circle, its vertex at parameter 0, or the apex.
        let mut end = |b: &mut BodyBuilder, at: Point3, r: f64| -> Result<EdgeId, BrepError> {
            if r > 0.0 {
                let start = at + across * r;
                points.push(start);
                let vertex = b.vertex(start);
                let circle = EllipseCurve::new(at, axis, across * r, 1.0).map_err(geometry)?;
                Ok(b.edge(Edge {
                    curve: Some(Curve::Ellipse(circle)),
                    start: vertex,
                    end: vertex,
                    reversed: false,
                }))
            } else {
                let apex = b.vertex(at);
                Ok(b.edge(Edge {
                    curve: None,
                    start: apex,
                    end: apex,
                    reversed: false,
                }))
            }
        };
        let low = end(&mut b, center, base)?;
        let high = end(&mut b, top_center, top)?;
        let one = |b: &mut BodyBuilder, edge: EdgeId, reversed: bool| {
            let coedge = b.coedge(Coedge { edge, reversed });
            b.add_loop(Loop {
                coedges: vec![coedge],
            })
        };
        // Seen from outside, the base circle turns clockwise about the axis
        // and the top one counterclockwise; the side runs them the other
        // way, the base forward and the top back.
        let mut faces = Vec::new();
        if base > 0.0 {
            let lp = one(&mut b, low, true);
            let plane = Plane::new(center, -axis).map_err(geometry)?;
            faces.push(b.face(Face {
                surface: Surface::Plane(plane),
                loops: vec![lp],
                reversed: false,
            }));
        }
        if top > 0.0 {
            let lp = one(&mut b, high, false);
            let plane = Plane::new(top_center, axis).map_err(geometry)?;
            faces.push(b.face(Face {
                surface: Surface::Plane(plane),
                loops: vec![lp],
                reversed: false,
            }));
        }
        let loops = vec![one(&mut b, low, false), one(&mut b, high, true)];
        faces.push(b.face(Face {
            surface: Surface::Cone(side),
            loops,
            reversed: false,
        }));
        let shell = b.shell(Shell { faces });
        b.lump(Lump {
            shells: vec![shell],
        });
        b.build(tolerance(&points)?)
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
