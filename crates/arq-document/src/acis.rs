//! ACIS bodies (3DSOLID, SURFACE, REGION, BODY) seen through their
//! vertices, read from the SAT or SAB data the drawing keeps.
//!
//! The engine boxes a body from its wireframe cache (the isolines a DWG
//! stores beside the body). DXF keeps the body as SAT text but no such
//! cache, so a body read from DXF had no extent: it would be drawn as a
//! point at the origin, and a save as DXF could not be checked. The
//! vertices are the same in both encodings (measured on LibreDWG's
//! `ATMOS-DC22S.dwg` and `Surface.dwg`: every coordinate equal).

use acadrust::EntityType;
use acadrust::entities::acis::types::{SatDocument, SatPoint};
use acadrust::entities::solid3d::AcisData;
use arq_geometry::{Bounds2, Point2};

/// The ACIS data of a body, `None` for any other entity.
fn acis_data(e: &EntityType) -> Option<&AcisData> {
    match e {
        EntityType::Solid3D(s) => Some(&s.acis_data),
        EntityType::Surface(s) => Some(&s.acis_data),
        EntityType::Region(r) => Some(&r.acis_data),
        EntityType::Body(b) => Some(&b.acis_data),
        _ => None,
    }
}

/// Whether the entity is an ACIS body.
pub(crate) fn is_body(e: &EntityType) -> bool {
    acis_data(e).is_some()
}

/// The body's vertex positions, sorted; `None` if it is not a body or its
/// data does not parse.
pub(crate) fn vertices(e: &EntityType) -> Option<Vec<[f64; 3]>> {
    let data = acis_data(e)?;
    let doc = if data.is_binary {
        acadrust::SabReader::read(&data.sab_data).ok()?
    } else {
        SatDocument::parse(&data.sat_data).ok()?
    };
    let mut points: Vec<[f64; 3]> = doc
        .vertices()
        .iter()
        .filter_map(|v| doc.resolve(v.point()))
        .filter_map(SatPoint::from_record)
        .map(|p| {
            let (x, y, z) = p.position();
            [x, y, z]
        })
        .filter(|p| p.iter().all(|c| c.is_finite()))
        .collect();
    points.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Some(points)
}

/// The plan extent of a body's vertices; `None` without vertices.
pub(crate) fn bounds(e: &EntityType) -> Option<Bounds2> {
    let points = vertices(e)?;
    let mut iter = points.iter();
    let first = iter.next()?;
    let (mut min, mut max) = ([first[0], first[1]], [first[0], first[1]]);
    for p in iter {
        min = [min[0].min(p[0]), min[1].min(p[1])];
        max = [max[0].max(p[0]), max[1].max(p[1])];
    }
    Some(Bounds2::new(
        Point2::new(min[0], min[1]).ok()?,
        Point2::new(max[0], max[1]).ok()?,
    ))
}

#[cfg(test)]
#[path = "acis_tests.rs"]
mod tests;
