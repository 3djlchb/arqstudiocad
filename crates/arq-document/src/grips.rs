//! Grips (ADR 0016 D7): the points of selected entities the user drags to
//! edit them. Lines show their ends and midpoint; circles their center and
//! quadrants; arcs their ends, midpoint and center; a block its insertion
//! point and a lightweight polyline its first vertex, which only move them
//! until vertex editing (milestone 02.8).

use arq_geometry::{Geometry2, Point2};

use crate::document::Document;
use crate::entity::EntityId;
use crate::translate::{handle, whole_grip};

/// What a grip stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GripKind {
    /// An end of a line or an arc.
    End,
    /// The middle of a line or an arc.
    Mid,
    /// The center of a circle or an arc.
    Center,
    /// A quadrant of a circle (East, North, West, South).
    Quadrant,
    /// A block's insertion point.
    Insertion,
    /// A polyline's first vertex.
    Vertex,
}

impl GripKind {
    /// A short English key, as the object snaps' (`"END"`…).
    pub fn name(self) -> &'static str {
        match self {
            Self::End => "END",
            Self::Mid => "MID",
            Self::Center => "CEN",
            Self::Quadrant => "QUA",
            Self::Insertion => "INS",
            Self::Vertex => "VTX",
        }
    }
}

/// One grip: its entity, what it stands for and where it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grip {
    /// The entity it edits.
    pub entity: EntityId,
    /// What it stands for.
    pub kind: GripKind,
    /// Where it is, in world coordinates.
    pub point: Point2,
}

impl Document {
    /// The grips of `ids`, entity by entity in the order given, repeats
    /// left out; an entity that does not exist, or of a kind without grips
    /// yet (texts, hatches…), has none.
    pub fn grips(&self, ids: &[EntityId]) -> Vec<Grip> {
        let mut seen = Vec::with_capacity(ids.len());
        let mut out = Vec::new();
        for &id in ids {
            if seen.contains(&id) {
                continue;
            }
            seen.push(id);
            let Some(e) = self.entity(id) else {
                continue;
            };
            let mut add = |kind, point| {
                out.push(Grip {
                    entity: id,
                    kind,
                    point,
                })
            };
            match e.plan().map(|p| p.geometry()) {
                Some(Geometry2::Line(l)) => {
                    add(GripKind::End, l.start());
                    add(GripKind::Mid, l.midpoint());
                    add(GripKind::End, l.end());
                }
                Some(Geometry2::Circle(c)) => {
                    add(GripKind::Center, c.center());
                    for quarter in 0..4 {
                        let angle = f64::from(quarter) * std::f64::consts::FRAC_PI_2;
                        add(GripKind::Quadrant, c.point_at_angle(angle));
                    }
                }
                Some(Geometry2::Arc(a)) => {
                    add(GripKind::End, a.start_point());
                    add(GripKind::Mid, a.midpoint());
                    add(GripKind::End, a.end_point());
                    add(GripKind::Center, a.center());
                }
                None if e.is_transformable() => {
                    let found = self.cad().get_entity(handle(id)).and_then(whole_grip);
                    if let Some((insertion, point)) = found {
                        let kind = if insertion {
                            GripKind::Insertion
                        } else {
                            GripKind::Vertex
                        };
                        add(kind, point);
                    }
                }
                None => {}
            }
        }
        out
    }
}

#[cfg(test)]
#[path = "grips_tests.rs"]
mod tests;
