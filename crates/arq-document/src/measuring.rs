//! Measuring a drawing's solids (ADR 0024 D4, step 3; what MASSPROP
//! reports): the chosen solids, and the solids inside chosen block
//! references, placed where each copy of the reference puts them
//! (maintainer, 2026-11-01), measured together: their volumes and areas
//! added, the centroid weighted by volume, their bounds joined.
//!
//! A reference scaled unevenly along its axes is refused: it would turn a
//! sphere into an ellipsoid, which no face of the kernel describes. A
//! mirrored one is measured as its mirror image.

use std::fmt;

use acadrust::EntityType;
use acadrust::types::Transform;
use arq_geometry::{Bounds3, Point3, Transform3, Vector3};
use arq_kernel::{MassProperties, MeasureError};

use crate::document::Document;
use crate::drawing::{insert_copies, is_xref};
use crate::entity::EntityId;
use crate::error::{DocumentError, DocumentResult};
use crate::solid::{Untranslated, body_of};
use crate::translate::handle;

/// Why a chosen entity, or a solid inside it, was not measured.
#[derive(Clone, Debug, PartialEq)]
pub enum Unmeasured {
    /// It is no solid, and no block reference holding one.
    NotASolid,
    /// An ACIS body the kernel does not read yet.
    Untranslated(Untranslated),
    /// Its faces do not close around a volume, or one cannot be integrated
    /// yet.
    Measure(MeasureError),
    /// Inside a block reference scaled unevenly along its axes.
    UnevenScale,
}

impl fmt::Display for Unmeasured {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotASolid => write!(f, "not a solid"),
            Self::Untranslated(why) => write!(f, "{why}"),
            Self::Measure(why) => write!(f, "{why}"),
            Self::UnevenScale => write!(f, "inside a block scaled unevenly"),
        }
    }
}

/// What MASSPROP reports of a selection.
#[derive(Clone, Debug, PartialEq)]
pub struct SolidMeasures {
    /// The solids measured together; `None` when none was.
    pub total: Option<MassProperties>,
    /// How many solids that is, a reference's counted once per copy.
    pub solids: usize,
    /// The chosen entities refused, each with why: a block reference once
    /// for every solid inside it that was not measured.
    pub refused: Vec<(EntityId, Unmeasured)>,
}

/// The sums the total comes from.
#[derive(Default)]
struct Gathered {
    volume: f64,
    area: f64,
    /// The volume's first moments: Σ volume × centroid.
    moment: [f64; 3],
    bounds: Option<Bounds3>,
    solids: usize,
    refused: Vec<(EntityId, Unmeasured)>,
}

impl Gathered {
    fn add(&mut self, m: &MassProperties) {
        self.volume += m.volume;
        self.area += m.area;
        let c = [m.centroid.x(), m.centroid.y(), m.centroid.z()];
        for (moment, c) in self.moment.iter_mut().zip(c) {
            *moment += m.volume * c;
        }
        self.bounds = Some(self.bounds.map_or(m.bounds, |b| b.union(m.bounds)));
        self.solids += 1;
    }
}

impl Document {
    /// The solids among `ids`, and those inside the block references among
    /// them, measured together. Each id counts once; an id the drawing does
    /// not hold is an error.
    pub fn measure_solids(&self, ids: &[EntityId]) -> DocumentResult<SolidMeasures> {
        let mut gathered = Gathered::default();
        let mut seen = Vec::with_capacity(ids.len());
        for &id in ids {
            let entity = self
                .cad()
                .get_entity(handle(id))
                .ok_or(DocumentError::EntityNotFound(id))?;
            if seen.contains(&id) {
                continue;
            }
            seen.push(id);
            let (solids, refused) = (gathered.solids, gathered.refused.len());
            self.gather(entity, None, &mut Vec::new(), id, &mut gathered);
            if gathered.solids == solids && gathered.refused.len() == refused {
                gathered.refused.push((id, Unmeasured::NotASolid));
            }
        }
        let total = gathered.bounds.map(|bounds| {
            let v = gathered.volume;
            let [x, y, z] = gathered.moment.map(|m| m / v);
            MassProperties {
                volume: v,
                area: gathered.area,
                centroid: Point3::new(x, y, z).unwrap_or(bounds.center()),
                bounds,
            }
        });
        Ok(SolidMeasures {
            total,
            solids: gathered.solids,
            refused: gathered.refused,
        })
    }

    /// Measures `e`, placed by `placement` when it sits in a block, into
    /// `out`, refusals charged to the chosen entity `id`; `blocks` are the
    /// blocks being walked, so a block inside itself stops.
    fn gather(
        &self,
        e: &EntityType,
        placement: Option<&Transform>,
        blocks: &mut Vec<String>,
        id: EntityId,
        out: &mut Gathered,
    ) {
        if let Some(read) = body_of(e) {
            let body = match read {
                Ok(body) => body,
                Err(why) => return out.refused.push((id, Unmeasured::Untranslated(why))),
            };
            let body = match placement {
                None => body,
                Some(t) => match similarity(t).and_then(|(t, s)| body.transformed(t, s).ok()) {
                    Some(placed) => placed,
                    None => return out.refused.push((id, Unmeasured::UnevenScale)),
                },
            };
            match body.mass_properties() {
                Ok(m) => out.add(&m),
                Err(why) => out.refused.push((id, Unmeasured::Measure(why))),
            }
            return;
        }
        let EntityType::Insert(ins) = e else {
            return;
        };
        let Some(record) = self.cad().block_records.get(&ins.block_name) else {
            return;
        };
        if is_xref(record)
            || blocks
                .iter()
                .any(|b| b.eq_ignore_ascii_case(&ins.block_name))
        {
            return; // an external drawing, or a block inside itself
        }
        blocks.push(ins.block_name.clone());
        for copy in insert_copies(ins, record.base_point) {
            let t = placement.map_or(copy, |outer| copy.then(outer));
            for child in self.cad().entities_in_block(&ins.block_name) {
                self.gather(child, Some(&t), blocks, id, out);
            }
        }
        blocks.pop();
    }
}

/// `t` as a rigid motion with uniform scale (a mirror allowed), and that
/// scale; `None` when it stretches unevenly or skews.
fn similarity(t: &Transform) -> Option<(Transform3, f64)> {
    let m = t.matrix.m;
    let column = |c: usize| [m[0][c], m[1][c], m[2][c]];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let columns = [column(0), column(1), column(2)];
    let scale = dot(columns[0], columns[0]).sqrt();
    if !(scale.is_finite() && scale > 0.0) {
        return None;
    }
    let tolerance = 1e-9 * scale * scale;
    for i in 0..3 {
        if (dot(columns[i], columns[i]) - scale * scale).abs() > tolerance {
            return None;
        }
        for j in i + 1..3 {
            if dot(columns[i], columns[j]).abs() > tolerance {
                return None;
            }
        }
    }
    let linear = [
        [m[0][0], m[0][1], m[0][2]],
        [m[1][0], m[1][1], m[1][2]],
        [m[2][0], m[2][1], m[2][2]],
    ];
    let offset = Vector3::new(m[0][3], m[1][3], m[2][3]).ok()?;
    Some((Transform3::new(linear, Point3::ORIGIN, offset).ok()?, scale))
}

#[cfg(test)]
#[path = "measuring_tests.rs"]
mod tests;
