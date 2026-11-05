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
use acadrust::entities::acis::SabWriter;
use acadrust::entities::acis::types::{SatDocument, SatPoint, SatRecord};
use acadrust::entities::solid3d::AcisData;
use arq_geometry::{Bounds2, Point2};

/// The ACIS data of a body, `None` for any other entity.
pub(crate) fn acis_data(e: &EntityType) -> Option<&AcisData> {
    match e {
        EntityType::Solid3D(s) => Some(&s.acis_data),
        EntityType::Surface(s) => Some(&s.acis_data),
        EntityType::Region(r) => Some(&r.acis_data),
        EntityType::Body(b) => Some(&b.acis_data),
        _ => None,
    }
}

/// The ACIS data of a body, to change; `None` for any other entity.
fn acis_data_mut(e: &mut EntityType) -> Option<&mut AcisData> {
    match e {
        EntityType::Solid3D(s) => Some(&mut s.acis_data),
        EntityType::Surface(s) => Some(&mut s.acis_data),
        EntityType::Region(r) => Some(&mut r.acis_data),
        EntityType::Body(b) => Some(&mut b.acis_data),
        _ => None,
    }
}

/// The handle of a body's construction history (AutoCAD's, R2007+), if
/// it has one.
pub(crate) fn history_of(e: &EntityType) -> Option<acadrust::Handle> {
    let h = match e {
        EntityType::Solid3D(s) => s.history_handle,
        EntityType::Surface(s) => s.history_handle,
        EntityType::Region(r) => r.history_handle,
        EntityType::Body(b) => b.history_handle,
        _ => None,
    }?;
    (!h.is_null()).then_some(h)
}

/// Whether the entity is an ACIS body.
pub(crate) fn is_body(e: &EntityType) -> bool {
    acis_data(e).is_some()
}

/// The body's ACIS records, SAT or SAB; `None` if it is not a body or its
/// data does not parse.
pub(crate) fn parsed(e: &EntityType) -> Option<SatDocument> {
    let data = acis_data(e)?;
    if data.is_binary {
        acadrust::SabReader::read(&data.sab_data).ok()
    } else {
        SatDocument::parse(&data.sat_data).ok()
    }
}

/// A transform record's 3 × 3 rows, translation and scale, `p · M · s + t`
/// applied to row vectors; SAT text keeps them as numbers, SAB as rows of
/// three, some ShapeManager files as one string (read as the codec does).
pub(crate) fn transform_numbers(record: &SatRecord) -> Option<([[f64; 3]; 3], [f64; 3], f64)> {
    let mut v: Vec<f64> = Vec::with_capacity(13);
    for token in &record.tokens {
        if v.len() >= 13 {
            break;
        }
        if let Some((components, len)) = token.coordinate_components() {
            v.extend_from_slice(&components[..len]);
        } else if let Some(f) = token.as_float() {
            v.push(f);
        } else if let Some(text) = token.as_string() {
            v.extend(
                text.split_ascii_whitespace()
                    .map_while(|w| w.parse::<f64>().ok()),
            );
        }
    }
    (v.len() >= 13 && v[..13].iter().all(|x| x.is_finite())).then(|| {
        (
            [[v[0], v[1], v[2]], [v[3], v[4], v[5]], [v[6], v[7], v[8]]],
            [v[9], v[10], v[11]],
            v[12],
        )
    })
}

/// The determinant of a 3 × 3 matrix.
pub(crate) fn determinant(m: [[f64; 3]; 3]) -> f64 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

/// After the codec composes a move, turn, mirror or scale into a body's
/// placement (its scale folded into the matrix), states it as ACIS does:
/// a matrix of unit rows, the size in the scale. The data stays SAT or
/// SAB as it was.
pub(crate) fn tidy_placement(e: &mut EntityType) {
    let Some(data) = acis_data_mut(e) else {
        return;
    };
    let Some(mut doc) = data.parse() else {
        return;
    };
    let Some((m, t, s)) = doc
        .records
        .iter()
        .find(|r| r.entity_type == "transform")
        .and_then(transform_numbers)
    else {
        return;
    };
    let size = s * determinant(m).abs().cbrt();
    if !(size.is_finite() && size > 0.0) || (size - s).abs() <= 1e-15 * size {
        return;
    }
    let unit = m.map(|row| row.map(|x| x * s / size));
    doc.set_placement(unit, t, size);
    if data.is_binary {
        data.sab_data = SabWriter::write(&doc);
    } else {
        data.sat_data = AcisData::strip_sat_terminator(&doc.to_sat_string());
    }
}

/// The body's vertex positions where its placement puts them, sorted;
/// `None` if it is not a body or its data does not parse.
pub(crate) fn vertices(e: &EntityType) -> Option<Vec<[f64; 3]>> {
    let doc = parsed(e)?;
    let place = doc
        .records
        .iter()
        .find(|r| r.entity_type == "transform")
        .and_then(transform_numbers);
    let mut points: Vec<[f64; 3]> = doc
        .vertices()
        .iter()
        .filter_map(|v| doc.resolve(v.point()))
        .filter_map(SatPoint::from_record)
        .map(|p| {
            let (x, y, z) = p.position();
            match place {
                None => [x, y, z],
                Some((m, t, s)) => {
                    let row = |j: usize| (x * m[0][j] + y * m[1][j] + z * m[2][j]) * s + t[j];
                    [row(0), row(1), row(2)]
                }
            }
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

#[cfg(test)]
#[path = "acis_transform_tests.rs"]
mod transform_tests;
