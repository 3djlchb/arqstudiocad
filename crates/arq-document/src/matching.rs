//! MATCHPROP, as AutoCAD's: one entity's properties given to others
//! (milestone 02.14, `docs/design/matchprop-command.md`). The basic
//! properties every entity has (color, layer, linetype, linetype scale,
//! lineweight, transparency) and its thickness, and a polyline's width and
//! linetype generation.

use acadrust::EntityType;
use acadrust::entities::LwPolyline;

use crate::document::Document;
use crate::entity::EntityId;
use crate::error::{DocumentError, DocumentResult};
use crate::translate::handle;

/// Which properties MATCHPROP gives (AutoCAD's Property Settings).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchSettings {
    /// The color, a color book's included.
    pub color: bool,
    /// The layer.
    pub layer: bool,
    /// The linetype.
    pub linetype: bool,
    /// The linetype scale.
    pub linetype_scale: bool,
    /// The lineweight.
    pub lineweight: bool,
    /// The transparency.
    pub transparency: bool,
    /// The thickness, between entities that have one (lines, arcs,
    /// circles, polylines, texts).
    pub thickness: bool,
    /// Between polylines, the width (only a uniform one, as AutoCAD) and
    /// the linetype generation.
    pub polyline: bool,
}

impl MatchSettings {
    /// No property.
    pub const NONE: Self = Self {
        color: false,
        layer: false,
        linetype: false,
        linetype_scale: false,
        lineweight: false,
        transparency: false,
        thickness: false,
        polyline: false,
    };

    /// Every property, as AutoCAD starts.
    pub const ALL: Self = Self {
        color: true,
        layer: true,
        linetype: true,
        linetype_scale: true,
        lineweight: true,
        transparency: true,
        thickness: true,
        polyline: true,
    };
}

impl Default for MatchSettings {
    fn default() -> Self {
        Self::ALL
    }
}

/// Sets `*field` to `value`; true when that changed it.
fn put<T: PartialEq>(field: &mut T, value: T) -> bool {
    if *field == value {
        return false;
    }
    *field = value;
    true
}

/// The thickness of the entities that have one.
fn thickness(e: &EntityType) -> Option<f64> {
    match e {
        EntityType::Line(x) => Some(x.thickness),
        EntityType::Arc(x) => Some(x.thickness),
        EntityType::Circle(x) => Some(x.thickness),
        EntityType::LwPolyline(x) => Some(x.thickness),
        EntityType::Text(x) => Some(x.thickness),
        _ => None,
    }
}

fn thickness_mut(e: &mut EntityType) -> Option<&mut f64> {
    match e {
        EntityType::Line(x) => Some(&mut x.thickness),
        EntityType::Arc(x) => Some(&mut x.thickness),
        EntityType::Circle(x) => Some(&mut x.thickness),
        EntityType::LwPolyline(x) => Some(&mut x.thickness),
        EntityType::Text(x) => Some(&mut x.thickness),
        _ => None,
    }
}

/// A polyline's width when every segment has the same one, a constant
/// width or the same at every vertex; None when it varies.
fn uniform_width(p: &LwPolyline) -> Option<f64> {
    if p.constant_width > 0.0 {
        return Some(p.constant_width);
    }
    let segments = if p.is_closed {
        p.vertices.len()
    } else {
        p.vertices.len().saturating_sub(1)
    };
    let first = p.vertices.first().map_or(0.0, |v| v.start_width);
    p.vertices[..segments]
        .iter()
        .all(|v| v.start_width == first && v.end_width == first)
        .then_some(first)
}

impl Document {
    /// Gives entity `target` the properties of entity `source` that
    /// `settings` names (MATCHPROP): those of every entity, the thickness
    /// when both have one, and, between two lightweight polylines, the
    /// width when the source's is uniform (a varying one is not given, as
    /// in AutoCAD) and the linetype generation. Returns whether anything
    /// changed; a target that already matches is left untouched.
    pub fn match_properties(
        &mut self,
        source: EntityId,
        target: EntityId,
        settings: MatchSettings,
    ) -> DocumentResult<bool> {
        let from = self
            .cad()
            .get_entity(handle(source))
            .ok_or(DocumentError::EntityNotFound(source))?
            .clone();
        let common = from.common().clone();
        // Worked on a copy: the entity is written only when something
        // changed, as reaching it to write records a change.
        let mut copy = self
            .cad()
            .get_entity(handle(target))
            .ok_or(DocumentError::EntityNotFound(target))?
            .clone();
        let to = &mut copy;
        let mut changed = false;
        {
            let c = to.common_mut();
            if settings.color {
                changed |= put(&mut c.color, common.color);
                changed |= put(&mut c.color_name, common.color_name.clone());
                changed |= put(&mut c.color_book_handle, common.color_book_handle);
            }
            if settings.layer {
                changed |= put(&mut c.layer, common.layer.clone());
                changed |= put(&mut c.layer_handle, common.layer_handle);
            }
            if settings.linetype {
                changed |= put(&mut c.linetype, common.linetype.clone());
                changed |= put(&mut c.linetype_handle, common.linetype_handle);
            }
            if settings.linetype_scale {
                changed |= put(&mut c.linetype_scale, common.linetype_scale);
            }
            if settings.lineweight {
                changed |= put(&mut c.line_weight, common.line_weight);
            }
            if settings.transparency {
                changed |= put(&mut c.transparency, common.transparency);
            }
        }
        if settings.thickness
            && let (Some(value), Some(field)) = (thickness(&from), thickness_mut(to))
        {
            changed |= put(field, value);
        }
        if settings.polyline
            && let (EntityType::LwPolyline(from), EntityType::LwPolyline(to)) = (&from, &mut *to)
        {
            changed |= put(&mut to.plinegen, from.plinegen);
            if let Some(width) = uniform_width(from) {
                changed |= put(&mut to.constant_width, width);
                for vertex in &mut to.vertices {
                    changed |= put(&mut vertex.start_width, 0.0);
                    changed |= put(&mut vertex.end_width, 0.0);
                }
            }
        }
        if changed {
            let entity = self
                .cad_mut()
                .get_entity_mut(handle(target))
                .expect("read above");
            *entity = copy;
            self.sync_entity(target);
        }
        Ok(changed)
    }
}

#[cfg(test)]
#[path = "matching_tests.rs"]
mod tests;
