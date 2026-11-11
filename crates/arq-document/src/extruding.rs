//! EXTRUDE (ADR 0024 step 4c): closed profiles made solids, as AutoCAD's
//! EXTRUDE with its height. A closed polyline of segments and arcs, or a
//! circle, in plan at any elevation; each becomes a 3DSOLID on its own
//! layer and is erased, as AutoCAD's DELOBJ does by default (maintainer,
//! 2026-11-05). Ellipses, splines and regions wait for NURBS in the ACIS
//! writer.

use std::f64::consts::TAU;
use std::fmt;

use acadrust::EntityType;
use acadrust::entities::LwPolyline;
use arq_geometry::{Arc, Geometry2, GeometryError, Line, Point2, Point3, Tolerance};
use arq_kernel::{Body, ExtrusionError, ProfileError, ProfilePiece};

use crate::document::Document;
use crate::entity::EntityId;
use crate::error::{DocumentError, DocumentResult};
use crate::ocs::plan_normal;
use crate::translate::{handle, plan_of};

/// Why an entity was not extruded.
#[derive(Clone, Debug, PartialEq)]
pub enum Unextruded {
    /// Not a closed polyline or a circle.
    NotAProfile,
    /// A polyline that is not closed.
    Open,
    /// On a tilted plane, not in plan.
    Tilted,
    /// A closed polyline that crosses itself or encloses nothing.
    Profile(ProfileError),
}

impl fmt::Display for Unextruded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAProfile => write!(f, "only closed polylines and circles are extruded"),
            Self::Open => write!(f, "the polyline is not closed"),
            Self::Tilted => write!(f, "the profile is not in plan"),
            Self::Profile(e) => write!(f, "{e}"),
        }
    }
}

/// What EXTRUDE made: the new solids, and the entities left as they were.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Extruded {
    /// The solids made, in the order of their profiles.
    pub solids: Vec<EntityId>,
    /// The entities not extruded, and why.
    pub refused: Vec<(EntityId, Unextruded)>,
}

impl Document {
    /// Extrudes each of `ids` that is a closed profile `height` along Z
    /// (negative: downward) into a 3DSOLID on the profile's layer, and
    /// erases the profile; the rest are left, each with why. A height of 0
    /// is an error and changes nothing; an id the drawing does not hold is
    /// [`DocumentError::EntityNotFound`].
    pub fn extrude_entities(&mut self, ids: &[EntityId], height: f64) -> DocumentResult<Extruded> {
        if !(height.is_finite() && height != 0.0) {
            return Err(GeometryError::ZeroLength.into());
        }
        let mut made = Vec::new();
        let mut out = Extruded::default();
        for &id in ids {
            let entity = self
                .cad()
                .get_entity(handle(id))
                .ok_or(DocumentError::EntityNotFound(id))?;
            let layer = entity.as_entity().layer().to_owned();
            match extrusion_of(entity, height) {
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

/// The solid `e` extrudes to, or why it does not.
fn extrusion_of(e: &EntityType, height: f64) -> Result<Body, Unextruded> {
    let made = |r: Result<Body, ExtrusionError>| {
        r.map_err(|e| match e {
            ExtrusionError::Profile(p) => Unextruded::Profile(p),
            // The height was checked; anything else is a degenerate profile.
            ExtrusionError::Geometry(_) => Unextruded::Profile(ProfileError::Flat),
        })
    };
    match e {
        EntityType::Circle(_) => {
            let plan = plan_of(e).ok_or(Unextruded::Tilted)?;
            let Geometry2::Circle(c) = plan.geometry() else {
                return Err(Unextruded::NotAProfile);
            };
            let center = Point3::new(c.center().x(), c.center().y(), plan.elevation())
                .map_err(|_| Unextruded::NotAProfile)?;
            // The height was checked: a circle always makes a cylinder.
            Body::cylinder(center, c.radius(), height)
                .map_err(|_| Unextruded::Profile(ProfileError::Flat))
        }
        EntityType::LwPolyline(p) => {
            let (pieces, elevation) = pieces_of(p)?;
            made(Body::extrusion(&pieces, elevation, height))
        }
        _ => Err(Unextruded::NotAProfile),
    }
}

/// A closed profile's pieces in plan, and its elevation: a closed
/// polyline's, or a circle as two half circles (REVOLVE turns either).
pub(crate) fn plan_profile(e: &EntityType) -> Result<(Vec<ProfilePiece>, f64), Unextruded> {
    match e {
        EntityType::Circle(_) => {
            let plan = plan_of(e).ok_or(Unextruded::Tilted)?;
            let Geometry2::Circle(c) = plan.geometry() else {
                return Err(Unextruded::NotAProfile);
            };
            let half = |from: f64, to: f64| {
                Arc::new(c.center(), c.radius(), from, to)
                    .map(|arc| ProfilePiece {
                        curve: Geometry2::Arc(arc),
                        backward: false,
                    })
                    .map_err(|_| Unextruded::NotAProfile)
            };
            Ok((
                vec![half(0.0, TAU / 2.0)?, half(TAU / 2.0, 0.0)?],
                plan.elevation(),
            ))
        }
        EntityType::LwPolyline(p) => pieces_of(p),
        _ => Err(Unextruded::NotAProfile),
    }
}

/// A closed polyline's pieces in world coordinates, and its elevation.
fn pieces_of(p: &LwPolyline) -> Result<(Vec<ProfilePiece>, f64), Unextruded> {
    let normal = plan_normal(p.normal.x, p.normal.y, p.normal.z).ok_or(Unextruded::Tilted)?;
    let tol = Tolerance::DEFAULT;
    let world = |x: f64, y: f64| {
        let (x, y, _) = normal.map_point(x, y, p.elevation);
        Point2::new(x, y).map_err(|_| Unextruded::NotAProfile)
    };
    let n = p.vertices.len();
    let first = p.vertices.first().ok_or(Unextruded::Open)?;
    let last = p.vertices.last().ok_or(Unextruded::Open)?;
    let ends_meet = world(first.location.x, first.location.y)?
        .approx_eq(world(last.location.x, last.location.y)?, tol);
    if !p.is_closed && !ends_meet {
        return Err(Unextruded::Open);
    }
    // Seen from below its OCS, the plan is mirrored: arcs turn the other way.
    let mirrored = normal.map_point(1.0, 0.0, 0.0).0 < 0.0;
    let mut pieces = Vec::with_capacity(n);
    for k in 0..n {
        let (v, w) = (&p.vertices[k], &p.vertices[(k + 1) % n]);
        let (a, b) = (
            world(v.location.x, v.location.y)?,
            world(w.location.x, w.location.y)?,
        );
        if a.approx_eq(b, tol) {
            continue; // a repeated vertex, or the closing one
        }
        let bulge = if mirrored { -v.bulge } else { v.bulge };
        pieces.push(piece(a, b, bulge).ok_or(Unextruded::NotAProfile)?);
    }
    let elevation = normal.map_point(0.0, 0.0, p.elevation).2;
    Ok((pieces, elevation))
}

/// The piece from `a` to `b` with a polyline's `bulge`: the tangent of a
/// quarter of its arc's angle, positive counterclockwise; 0 is straight.
fn piece(a: Point2, b: Point2, bulge: f64) -> Option<ProfilePiece> {
    if bulge == 0.0 || !bulge.is_finite() {
        return Some(ProfilePiece {
            curve: Geometry2::Line(Line::new(a, b).ok()?),
            backward: false,
        });
    }
    let (arc, backward) = Arc::from_bulge(a, b, bulge).ok()?;
    Some(ProfilePiece {
        curve: Geometry2::Arc(arc),
        backward,
    })
}

#[cfg(test)]
#[path = "extruding_tests.rs"]
mod tests;
