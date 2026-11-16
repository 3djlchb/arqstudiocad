//! A polyline's constant width survives in DXF for every reader: group 43
//! alone, no zero vertex widths (40/41) beside it, which the DXF reference
//! says make 43 unused. ProgeCAD read our constant-width polylines at width
//! 0 until the codec fork's `lwpolyline-dxf-width` correction (2026-11-15).
use super::*;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

/// The group codes and values of the DXF's LWPOLYLINE entities.
fn lwpolyline_groups(doc: &Document) -> Vec<(i32, String)> {
    let text = String::from_utf8(doc.to_dxf_bytes().unwrap()).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let pairs: Vec<(i32, String)> = lines
        .chunks(2)
        .filter(|pair| pair.len() == 2)
        .map(|pair| (pair[0].parse().unwrap_or(-1), pair[1].to_owned()))
        .collect();
    let mut inside = false;
    let mut found = Vec::new();
    for (code, value) in pairs {
        if code == 0 {
            inside = value == "LWPOLYLINE";
        }
        if inside {
            found.push((code, value));
        }
    }
    assert!(!found.is_empty(), "an LWPOLYLINE in the DXF");
    found
}

#[test]
fn a_constant_width_is_saved_to_dxf_as_group_43_alone() {
    let mut doc = Document::new();
    let square = [
        (p(10.0, 0.0), 0.0),
        (p(12.0, 0.0), 0.0),
        (p(12.0, 2.0), 0.0),
        (p(10.0, 2.0), 0.0),
    ];
    let style = PolylineStyle {
        width: 0.25,
        ..PolylineStyle::default()
    };
    doc.add_polyline_with(&square, true, style).unwrap();
    let groups = lwpolyline_groups(&doc);
    let widths: Vec<&str> = groups
        .iter()
        .filter(|(c, _)| *c == 43)
        .map(|(_, v)| v.as_str())
        .collect();
    assert_eq!(widths, ["0.25"]);
    assert!(
        groups.iter().all(|(c, _)| *c != 40 && *c != 41),
        "no zero vertex widths beside the constant width: {groups:?}"
    );
    let back = Document::from_dxf_bytes(doc.to_dxf_bytes().unwrap()).unwrap();
    let again = lwpolyline_groups(&back);
    assert_eq!(
        again.iter().filter(|(c, _)| *c == 43).count(),
        1,
        "read back and saved again"
    );
}
