//! REVOLVE (ADR 0024 step 4d): closed profiles turned about an axis in
//! their plane, as AutoCAD's REVOLVE. A closed polyline of segments and
//! arcs, or a circle, in plan at any elevation; the axis drawn in plan at
//! the profile's elevation; any angle, 360° a whole turn (maintainer,
//! 2026-11-05). Each becomes a 3DSOLID on its own layer and is erased, as
//! EXTRUDE does.

use std::f64::consts::TAU;
use std::fmt;

use arq_geometry::{Arc, Geometry2, GeometryError, Line, Point2, Point3, Tolerance, Vector3};
use arq_kernel::{Body, ProfileError, ProfilePiece, RevolutionError, RevolutionFrame};

use crate::document::Document;
use crate::entity::EntityId;
use crate::error::{DocumentError, DocumentResult};
use crate::extruding::{Unextruded, plan_profile};
use crate::translate::handle;

/// Why an entity was not revolved.
#[derive(Clone, Debug, PartialEq)]
pub enum Unrevolved {
    /// Not a closed polyline or a circle.
    NotAProfile,
    /// A polyline that is not closed.
    Open,
    /// On a tilted plane, not in plan.
    Tilted,
    /// A closed polyline that crosses itself or encloses nothing.
    Profile(ProfileError),
    /// The profile reaches across the axis.
    CrossesAxis,
    /// An arc would sweep a torus crossing itself.
    SelfCrossingTorus,
}

impl fmt::Display for Unrevolved {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAProfile => write!(f, "only closed polylines and circles are revolved"),
            Self::Open => write!(f, "the polyline is not closed"),
            Self::Tilted => write!(f, "the profile is not in plan"),
            Self::Profile(e) => write!(f, "{e}"),
            Self::CrossesAxis => write!(f, "the profile reaches across the axis"),
            Self::SelfCrossingTorus => write!(f, "an arc would sweep a torus crossing itself"),
        }
    }
}

/// What REVOLVE made: the new solids, and the entities left as they were.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Revolved {
    /// The solids made, in the order of their profiles.
    pub solids: Vec<EntityId>,
    /// The entities not revolved, and why.
    pub refused: Vec<(EntityId, Unrevolved)>,
}

impl Document {
    /// Turns each of `ids` that is a closed profile `angle` radians about
    /// the axis from `axis_start` to `axis_end` (in plan, at the profile's
    /// elevation; counterclockwise about that direction, negative the other
    /// way) into a 3DSOLID on its layer, erasing the profile; the rest are
    /// left, each with why. An axis of no length or an angle of 0 is an
    /// error and changes nothing.
    pub fn revolve_entities(
        &mut self,
        ids: &[EntityId],
        axis_start: Point2,
        axis_end: Point2,
        angle: f64,
    ) -> DocumentResult<Revolved> {
        if !(angle.is_finite() && angle != 0.0) {
            return Err(GeometryError::ZeroSweep.into());
        }
        if axis_start.approx_eq(axis_end, Tolerance::DEFAULT) {
            return Err(GeometryError::ZeroLength.into());
        }
        // Turning back is turning forward about the axis reversed.
        let (from, to) = if angle > 0.0 {
            (axis_start, axis_end)
        } else {
            (axis_end, axis_start)
        };
        let turn = angle.abs().min(TAU);
        let mut made = Vec::new();
        let mut out = Revolved::default();
        for &id in ids {
            let entity = self
                .cad()
                .get_entity(handle(id))
                .ok_or(DocumentError::EntityNotFound(id))?;
            let layer = entity.as_entity().layer().to_owned();
            let body = plan_profile(entity)
                .map_err(|why| match why {
                    Unextruded::NotAProfile => Unrevolved::NotAProfile,
                    Unextruded::Open => Unrevolved::Open,
                    Unextruded::Tilted => Unrevolved::Tilted,
                    Unextruded::Profile(p) => Unrevolved::Profile(p),
                })
                .and_then(|(pieces, elevation)| revolution(&pieces, elevation, from, to, turn));
            match body {
                Ok(body) => made.push((id, body, layer)),
                Err(why) => out.refused.push((id, why)),
            }
        }
        for (id, body, layer) in made {
            out.solids.push(self.add_solid_on(&body, &layer)?);
            self.delete_entity(id)?;
        }
        Ok(out)
    }
}

/// A plan profile at `elevation` turned `turn` about the axis from `from`
/// to `to`.
fn revolution(
    pieces: &[ProfilePiece],
    elevation: f64,
    from: Point2,
    to: Point2,
    turn: f64,
) -> Result<Body, Unrevolved> {
    let length = from.distance_to(to);
    let along = ((to.x() - from.x()) / length, (to.y() - from.y()) / length);
    // The radial direction: square to the axis, toward the profile.
    let mut out = (-along.1, along.0);
    let side = |p: Point2| (p.x() - from.x()) * out.0 + (p.y() - from.y()) * out.1;
    let tol = Tolerance::DEFAULT.linear() * length.max(1.0);
    // From a corner off the axis, or, all on it, an arc's middle.
    let lean = pieces
        .iter()
        .map(|p| side(p.start()))
        .chain(pieces.iter().filter_map(|p| match p.curve {
            Geometry2::Arc(a) => Some(side(a.midpoint())),
            _ => None,
        }))
        .find(|s| s.abs() > tol)
        .unwrap_or(1.0);
    if lean < 0.0 {
        out = (-out.0, -out.1);
    }
    // The profile in its half-plane: x from the axis, y along it. A
    // mirroring map turns arcs the other way.
    let map = |p: Point2| {
        let (dx, dy) = (p.x() - from.x(), p.y() - from.y());
        Point2::new(dx * out.0 + dy * out.1, dx * along.0 + dy * along.1)
    };
    let mirrored = out.0 * along.1 - out.1 * along.0 < 0.0;
    let mut half = Vec::with_capacity(pieces.len());
    for piece in pieces {
        let curve = match piece.curve {
            Geometry2::Line(l) => {
                let line = Line::new(
                    map(l.start()).map_err(|_| Unrevolved::NotAProfile)?,
                    map(l.end()).map_err(|_| Unrevolved::NotAProfile)?,
                );
                (
                    Geometry2::Line(line.map_err(|_| Unrevolved::NotAProfile)?),
                    piece.backward,
                )
            }
            Geometry2::Arc(a) => {
                let c = map(a.center()).map_err(|_| Unrevolved::NotAProfile)?;
                let angle = |p: Point2| (p.y() - c.y()).atan2(p.x() - c.x());
                let (s, e) = (
                    map(a.start_point()).map_err(|_| Unrevolved::NotAProfile)?,
                    map(a.end_point()).map_err(|_| Unrevolved::NotAProfile)?,
                );
                let (start, end, backward) = if mirrored {
                    (angle(e), angle(s), !piece.backward)
                } else {
                    (angle(s), angle(e), piece.backward)
                };
                let arc =
                    Arc::new(c, a.radius(), start, end).map_err(|_| Unrevolved::NotAProfile)?;
                (Geometry2::Arc(arc), backward)
            }
            Geometry2::Circle(_) => return Err(Unrevolved::NotAProfile),
        };
        half.push(ProfilePiece {
            curve: curve.0,
            backward: curve.1,
        });
    }
    let frame = RevolutionFrame {
        origin: Point3::new(from.x(), from.y(), elevation).map_err(|_| Unrevolved::NotAProfile)?,
        axis: Vector3::new(along.0, along.1, 0.0).map_err(|_| Unrevolved::NotAProfile)?,
        radial: Vector3::new(out.0, out.1, 0.0).map_err(|_| Unrevolved::NotAProfile)?,
    };
    Body::revolution(&half, frame, turn).map_err(|e| match e {
        RevolutionError::Profile(p) => Unrevolved::Profile(p),
        RevolutionError::CrossesAxis => Unrevolved::CrossesAxis,
        RevolutionError::SelfCrossingTorus => Unrevolved::SelfCrossingTorus,
        RevolutionError::Geometry(_) => Unrevolved::Profile(ProfileError::Flat),
    })
}

#[cfg(test)]
#[path = "revolving_tests.rs"]
mod tests;
