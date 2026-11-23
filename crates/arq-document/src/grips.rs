//! Grips (ADR 0016 D7): the points of selected entities the user drags to
//! edit them. Lines show their ends and midpoint; circles their center and
//! quadrants; arcs their ends, midpoint and center; a block its insertion
//! point and a lightweight polyline each vertex (milestone 02.8); a TEXT or
//! MTEXT its anchor (ADR 0022).

use arq_geometry::{Geometry2, Point2, Tolerance};

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
    /// A spline's fit point (ADR 0043 D4).
    FitPoint,
    /// A spline's control vertex.
    ControlVertex,
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
            Self::FitPoint => "FIT",
            Self::ControlVertex => "CV",
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
    /// Zero-based polyline vertex, spline fit point or control vertex, or
    /// `None` for other grip kinds.
    pub vertex_index: Option<usize>,
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
            let mut add = |kind, point, vertex_index| {
                out.push(Grip {
                    entity: id,
                    kind,
                    point,
                    vertex_index,
                })
            };
            match e.plan().map(|p| p.geometry()) {
                Some(Geometry2::Line(l)) => {
                    add(GripKind::End, l.start(), None);
                    add(GripKind::Mid, l.midpoint(), None);
                    add(GripKind::End, l.end(), None);
                }
                Some(Geometry2::Circle(c)) => {
                    add(GripKind::Center, c.center(), None);
                    for quarter in 0..4 {
                        let angle = f64::from(quarter) * std::f64::consts::FRAC_PI_2;
                        add(GripKind::Quadrant, c.point_at_angle(angle), None);
                    }
                }
                // Its center, the ends of its axes on it, an arc's ends
                // (ADR 0041 D4, as AutoCAD).
                // An axis end at an arc's end is that end's grip only.
                Some(Geometry2::Ellipse(e)) => {
                    add(GripKind::Center, e.center(), None);
                    let ends = e.ends();
                    let at_end = |q: Point2| {
                        ends.is_some_and(|(a, b)| {
                            q.approx_eq(a, Tolerance::DEFAULT) || q.approx_eq(b, Tolerance::DEFAULT)
                        })
                    };
                    for q in e.quadrants().into_iter().filter(|&q| !at_end(q)) {
                        add(GripKind::Quadrant, q, None);
                    }
                    if let Some((a, b)) = ends {
                        add(GripKind::End, a, None);
                        add(GripKind::End, b, None);
                    }
                }
                Some(Geometry2::Arc(a)) => {
                    add(GripKind::End, a.start_point(), None);
                    add(GripKind::Mid, a.midpoint(), None);
                    add(GripKind::End, a.end_point(), None);
                    add(GripKind::Center, a.center(), None);
                }
                // Its fit points when it has them, else its own vertices,
                // each by its number (ADR 0043 D4, as AutoCAD).
                Some(Geometry2::Spline(s)) => match self.spline_fit(id) {
                    Some(fit) => {
                        for (i, &q) in fit.points().iter().enumerate() {
                            add(GripKind::FitPoint, q, Some(i));
                        }
                    }
                    None => {
                        for (i, &q) in s.own_vertices().iter().enumerate() {
                            add(GripKind::ControlVertex, q, Some(i));
                        }
                    }
                },
                None if e.is_transformable() => {
                    if let Some(acadrust::EntityType::LwPolyline(polyline)) =
                        self.cad().get_entity(handle(id))
                    {
                        if let Some(normal) = crate::ocs::plan_normal(
                            polyline.normal.x,
                            polyline.normal.y,
                            polyline.normal.z,
                        ) {
                            for (index, vertex) in polyline.vertices.iter().enumerate() {
                                let (x, y, _) = normal.map_point(
                                    vertex.location.x,
                                    vertex.location.y,
                                    polyline.elevation,
                                );
                                if let Ok(point) = Point2::new(x, y) {
                                    add(GripKind::Vertex, point, Some(index));
                                }
                            }
                        }
                    } else if let Some((insertion, point)) =
                        self.cad().get_entity(handle(id)).and_then(whole_grip)
                    {
                        let kind = if insertion {
                            GripKind::Insertion
                        } else {
                            GripKind::Vertex
                        };
                        add(kind, point, None);
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
