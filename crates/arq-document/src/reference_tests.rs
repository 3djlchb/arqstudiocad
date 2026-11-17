use crate::Document;
use crate::test_dxf::{LAYERS, dxf};
use arq_geometry::snap::SnapKind::{Center, Endpoint, Midpoint};
use arq_geometry::{Point3, Tolerance};

fn has(
    points: &[(arq_geometry::snap::SnapKind, Point3)],
    kind: arq_geometry::snap::SnapKind,
    xyz: (f64, f64, f64),
) -> bool {
    points.iter().any(|(k, p)| {
        *k == kind
            && p.approx_eq(
                Point3::new(xyz.0, xyz.1, xyz.2).unwrap(),
                Tolerance::default(),
            )
    })
}

#[test]
fn exact_references_keep_line_slope_tilted_curves_and_polyline_bulges() {
    let doc = dxf(
        LAYERS,
        &[],
        &[
            &[
                "0", "LINE", "8", "0", "10", "1", "20", "2", "30", "3", "11", "5", "21", "6", "31",
                "11",
            ],
            &[
                "0", "CIRCLE", "8", "0", "10", "0", "20", "0", "30", "7", "40", "2", "210", "0",
                "220", "1", "230", "0",
            ],
            &[
                "0", "ARC", "8", "0", "10", "0", "20", "0", "30", "9", "40", "2", "50", "0", "51",
                "90",
            ],
            &[
                "0",
                "LWPOLYLINE",
                "8",
                "0",
                "90",
                "2",
                "38",
                "12",
                "10",
                "0",
                "20",
                "0",
                "42",
                "1",
                "10",
                "4",
                "20",
                "0",
            ],
        ],
    );
    let ids: Vec<_> = doc.entities().iter().map(|e| e.id()).collect();
    let p = doc.reference_points3(&ids);
    assert!(has(&p, Endpoint, (1., 2., 3.)));
    assert!(has(&p, Midpoint, (3., 4., 7.)));
    assert!(has(&p, Center, (0., 7., 0.)), "tilted OCS center is in WCS");
    assert!(has(&p, Endpoint, (2., 0., 9.)));
    assert!(has(&p, Midpoint, (2_f64.sqrt(), 2_f64.sqrt(), 9.)));
    assert!(has(&p, Endpoint, (4., 0., 12.)));
    assert!(
        has(&p, Midpoint, (2., -2., 12.)),
        "arc midpoint, not chord midpoint"
    );
}

#[test]
fn nested_block_references_keep_full_xyz_placements_and_visibility() {
    let a = &[
        "0", "BLOCK", "8", "0", "2", "A", "70", "0", "10", "0", "20", "0", "30", "0", "3", "A",
        "0", "LINE", "8", "0", "10", "0", "20", "0", "30", "2", "11", "2", "21", "0", "31", "6",
        "0", "ENDBLK", "8", "0",
    ][..];
    let b = &[
        "0", "BLOCK", "8", "0", "2", "B", "70", "0", "10", "0", "20", "0", "30", "0", "3", "B",
        "0", "INSERT", "8", "0", "2", "A", "10", "5", "20", "0", "30", "1", "0", "ENDBLK", "8",
        "0",
    ][..];
    let doc = dxf(
        LAYERS,
        &[a, b],
        &[
            &[
                "0", "INSERT", "8", "0", "2", "B", "10", "10", "20", "20", "30", "30", "41", "2",
                "42", "3", "43", "4",
            ],
            &[
                "0", "LINE", "8", "APAGADA", "10", "0", "20", "0", "30", "99", "11", "1", "21",
                "0", "31", "99",
            ],
            &[
                "0",
                "LINE",
                "8",
                "CONGELADA",
                "10",
                "0",
                "20",
                "0",
                "30",
                "88",
                "11",
                "1",
                "21",
                "0",
                "31",
                "88",
            ],
            &[
                "0", "LINE", "8", "0", "60", "1", "10", "0", "20", "0", "30", "77", "11", "1",
                "21", "0", "31", "77",
            ],
        ],
    );
    let ids: Vec<_> = doc.entities().iter().map(|e| e.id()).collect();
    let p = doc.reference_points3(&ids);
    assert_eq!(p.len(), 3);
    assert!(has(&p, Endpoint, (20., 20., 42.)));
    assert!(has(&p, Endpoint, (24., 20., 58.)));
    assert!(has(&p, Midpoint, (22., 20., 50.)));
}

#[test]
fn solid_references_are_topological_vertices_not_isoline_samples() {
    let mut doc = Document::new();
    let id = doc
        .add_box(
            Point3::new(1., 2., 3.).unwrap(),
            Point3::new(5., 6., 11.).unwrap(),
        )
        .unwrap();
    let p = doc.reference_points3(&[id]);
    assert_eq!(p.len(), 8);
    assert!(p.iter().all(|(k, _)| *k == Endpoint));
    assert!(has(&p, Endpoint, (5., 6., 11.)));
    assert!(doc.reference_points3(&[u64::MAX]).is_empty());
}
