//! UNION, SUBTRACT and INTERSECT (ADR 0032 D5): 3D solids combined by the
//! kernel's booleans, of planes, cylinders and cones (ADR 0033 deliveries 1
//! and 2, taken by the commands as each is done).
//!
//! - The result is written into the first solid chosen, which keeps its
//!   handle and its properties (layer, color, linetype, lineweight); the
//!   other solids are erased (maintainer, 2026-11-07).
//! - Solids apart make one solid of separate pieces (maintainer,
//!   2026-11-07).
//! - SUBTRACT, as AutoCAD's: the solids to subtract from are united, and
//!   every solid to subtract is taken away from them (maintainer,
//!   2026-11-07).
//! - Nothing changes, and the caller is told why, when a solid has faces
//!   the booleans do not take yet (a sphere's, a torus's, a NURBS
//!   surface's) or curved faces that meet in a way they do not combine yet,
//!   when a solid cannot be read, when fewer solids are chosen than the
//!   operation needs, or when nothing would be left (maintainer,
//!   2026-11-07). Entities that are not 3D solids are left out.

use acadrust::EntityType;
use acadrust::entities::Solid3D;
use arq_kernel::{Body, BooleanError};

use crate::acis_write::sat_of;
use crate::document::Document;
use crate::entity::EntityId;
use crate::error::{DocumentError, DocumentResult};
use crate::solid::body_of;
use crate::translate::handle;

/// How solids were combined, or why nothing changed.
#[derive(Clone, Debug, PartialEq)]
pub enum Combination {
    /// The solid now holding the result: the first one chosen.
    Made(EntityId),
    /// Fewer solids than the operation needs: two for UNION and
    /// INTERSECT, one of each kind for SUBTRACT.
    TooFew,
    /// Nothing would be left.
    Empty,
    /// These solids have faces the booleans do not take yet, or curved
    /// faces meeting as they do not combine yet (ADR 0033 delivery 4).
    Curved(Vec<EntityId>),
    /// These solids' ACIS data cannot be read yet.
    Unreadable(Vec<EntityId>),
}

/// What UNION, SUBTRACT or INTERSECT did: the outcome, and the entities
/// chosen that were not 3D solids, left out.
#[derive(Clone, Debug, PartialEq)]
pub struct Combined {
    /// The result, or why nothing changed.
    pub outcome: Combination,
    /// Entities left out: not 3D solids.
    pub not_solids: Vec<EntityId>,
}

/// Which boolean.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op {
    Union,
    Intersection,
    Difference,
}

impl Document {
    /// UNION: the 3D solids among `ids` united into the first of them; the
    /// others erased.
    pub fn union_solids(&mut self, ids: &[EntityId]) -> DocumentResult<Combined> {
        self.combine(ids, &[], Op::Union)
    }

    /// INTERSECT: what the 3D solids among `ids` have in common, in the
    /// first of them; the others erased.
    pub fn intersect_solids(&mut self, ids: &[EntityId]) -> DocumentResult<Combined> {
        self.combine(ids, &[], Op::Intersection)
    }

    /// SUBTRACT: the 3D solids among `from` united, less every 3D solid
    /// among `subtract`, in the first of `from`; the others erased. A solid
    /// in both counts as one to subtract from.
    pub fn subtract_solids(
        &mut self,
        from: &[EntityId],
        subtract: &[EntityId],
    ) -> DocumentResult<Combined> {
        self.combine(from, subtract, Op::Difference)
    }

    fn combine(
        &mut self,
        first: &[EntityId],
        second: &[EntityId],
        op: Op,
    ) -> DocumentResult<Combined> {
        let mut not_solids = Vec::new();
        let mut unreadable = Vec::new();
        let mut curved = Vec::new();
        let mut rounded = Vec::new();
        let mut seen = Vec::new();
        let mut bodies: [Vec<(EntityId, Body)>; 2] = [Vec::new(), Vec::new()];
        for (set, ids) in [first, second].into_iter().enumerate() {
            for &id in ids {
                if seen.contains(&id) {
                    continue;
                }
                seen.push(id);
                let entity = self
                    .cad()
                    .get_entity(handle(id))
                    .ok_or(DocumentError::EntityNotFound(id))?;
                if !matches!(entity, EntityType::Solid3D(_)) {
                    not_solids.push(id);
                    continue;
                }
                match body_of(entity) {
                    Some(Ok(body)) if body.combinable() => {
                        if !body.is_flat() {
                            rounded.push(id);
                        }
                        bodies[set].push((id, body));
                    }
                    Some(Ok(_)) => curved.push(id),
                    _ => unreadable.push(id),
                }
            }
        }
        let done = |outcome| {
            Ok(Combined {
                outcome,
                not_solids: not_solids.clone(),
            })
        };
        if !unreadable.is_empty() {
            return done(Combination::Unreadable(unreadable));
        }
        if !curved.is_empty() {
            return done(Combination::Curved(curved));
        }
        let [first, second] = bodies;
        let enough = match op {
            Op::Union | Op::Intersection => first.len() >= 2,
            Op::Difference => !first.is_empty() && !second.is_empty(),
        };
        if !enough {
            return done(Combination::TooFew);
        }
        let unite = |set: &[(EntityId, Body)]| -> Result<Body, BooleanError> {
            let mut all = set.iter();
            let mut body = all.next().expect("checked").1.clone();
            for (_, next) in all {
                body = body.union(next)?;
            }
            Ok(body)
        };
        let combined = || -> Result<Option<Body>, BooleanError> {
            Ok(match op {
                Op::Union => Some(unite(&first)?),
                Op::Intersection => {
                    let mut all = first.iter();
                    let mut body = Some(all.next().expect("checked").1.clone());
                    for (_, next) in all {
                        body = match body {
                            Some(b) => b.intersection(next)?,
                            None => None,
                        };
                    }
                    body
                }
                Op::Difference => unite(&first)?.difference(&unite(&second)?)?,
            })
        };
        let result = match combined() {
            Ok(result) => result,
            // Curved faces meeting as the booleans do not combine yet: the
            // curved solids chosen are named, nothing changes.
            Err(BooleanError::Curved) => return done(Combination::Curved(rounded)),
            Err(e) => return Err(DocumentError::UnsupportedOperation(e.to_string())),
        };
        let Some(result) = result else {
            return done(Combination::Empty);
        };
        let keep = first[0].0;
        self.replace_solid(keep, &result)?;
        for &(id, _) in first.iter().skip(1).chain(&second) {
            self.delete_entity(id)?;
        }
        done(Combination::Made(keep))
    }

    /// Writes `body` into the 3D solid `id`, which keeps its handle and
    /// properties; its construction history is set to none, as a moved
    /// solid's is.
    fn replace_solid(&mut self, id: EntityId, body: &Body) -> DocumentResult<()> {
        let sat =
            sat_of(body).map_err(|why| DocumentError::UnsupportedOperation(why.to_string()))?;
        let fresh = Solid3D::from_sat(&sat.to_sat_string());
        let entity = self
            .cad_mut()
            .get_entity_mut(handle(id))
            .ok_or(DocumentError::EntityNotFound(id))?;
        let history = crate::acis::history_of(entity);
        let EntityType::Solid3D(solid) = entity else {
            return Err(DocumentError::EntityNotFound(id));
        };
        solid.acis_data = fresh.acis_data;
        solid.point_of_reference = fresh.point_of_reference;
        // Drawn from the new body, not the old one's.
        solid.wires.clear();
        solid.silhouettes.clear();
        if let Some(history) = history {
            self.drop_history(history);
        }
        self.sync_entity(id);
        Ok(())
    }
}

#[cfg(test)]
#[path = "combining_tests.rs"]
mod tests;
