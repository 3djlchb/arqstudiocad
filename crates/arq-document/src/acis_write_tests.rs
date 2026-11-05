//! Tests of the ACIS writer (a submodule of `acis_write`): what it writes
//! reads back, through SAT text and SAB binary, as the same body.

use acadrust::entities::acis::types::SatDocument;
use arq_geometry::{Point3, Tolerance, Transform3, Vector3};
use arq_kernel::{Body, MassProperties, Sphere, Surface};

use super::*;
use crate::solid::translate;

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::new(x, y, z).unwrap()
}

fn a_box() -> Body {
    Body::cuboid(p(1.0, -2.0, 3.0), p(3.0, 1.0, 7.0)).unwrap()
}

#[track_caller]
fn same(a: &MassProperties, b: &MassProperties) {
    assert!(
        (a.volume - b.volume).abs() < 1e-12 * a.volume,
        "{} {}",
        a.volume,
        b.volume
    );
    assert!(
        (a.area - b.area).abs() < 1e-12 * a.area,
        "{} {}",
        a.area,
        b.area
    );
    let d = a.centroid - b.centroid;
    assert!(d.length() < 1e-12, "{:?} {:?}", a.centroid, b.centroid);
}

/// What the writer wrote, read back by the reader as text and as binary.
fn read_back(sat: &SatDocument) -> [Body; 2] {
    let text = SatDocument::parse(&sat.to_sat_string()).unwrap();
    let binary =
        acadrust::SabReader::read(&acadrust::entities::acis::SabWriter::write(sat)).unwrap();
    [translate(&text).unwrap(), translate(&binary).unwrap()]
}

#[test]
fn a_box_reads_back_the_same() {
    let body = a_box();
    let sat = sat_of(&body).unwrap();
    assert_eq!(
        (sat.faces().len(), sat.edges().len(), sat.vertices().len()),
        (6, 12, 8)
    );
    let measures = body.mass_properties().unwrap();
    for back in read_back(&sat) {
        back.check(Tolerance::DEFAULT).unwrap();
        assert_eq!(
            (
                back.faces().len(),
                back.edges().len(),
                back.vertices().len()
            ),
            (6, 12, 8)
        );
        same(&back.mass_properties().unwrap(), &measures);
    }
}

#[test]
fn a_mirrored_box_reads_back_inside_out_no_more() {
    // Mirrored, its loops run the other way round: the coedges' senses.
    let mirror = Transform3::new(
        [[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        Point3::ORIGIN,
        Vector3::new(0.0, 0.0, 0.0).unwrap(),
    )
    .unwrap();
    let body = a_box().transformed(mirror, 1.0).unwrap();
    let measures = body.mass_properties().unwrap();
    assert!(measures.volume > 0.0);
    for back in read_back(&sat_of(&body).unwrap()) {
        same(&back.mass_properties().unwrap(), &measures);
    }
}

#[test]
fn each_coedge_has_its_edge_partner() {
    let sat = sat_of(&a_box()).unwrap();
    let coedges: Vec<_> = sat
        .records
        .iter()
        .filter(|r| r.is_a("coedge"))
        .map(|r| {
            (
                r.index,
                r.token_pointer(3).unwrap(),
                r.token_pointer(4).unwrap(),
            )
        })
        .collect();
    assert_eq!(coedges.len(), 24);
    for &(at, partner, edge) in &coedges {
        let other = coedges.iter().find(|c| c.0 == partner.0).expect("a coedge");
        assert!(other.0 != at && other.2 == edge && other.1.0 == at, "{at}");
    }
}

#[test]
fn what_cannot_be_written_yet_says_why() {
    assert_eq!(sat_of(&Body::default()).err(), Some(Unwritable::Empty));
    let mut b = arq_kernel::BodyBuilder::new();
    let ball = Surface::Sphere(Sphere::new(p(0.0, 0.0, 0.0), 1.0).unwrap());
    let face = b.face(arq_kernel::Face {
        surface: ball,
        loops: vec![],
        reversed: false,
    });
    let shell = b.shell(arq_kernel::Shell { faces: vec![face] });
    b.lump(arq_kernel::Lump {
        shells: vec![shell],
    });
    let sphere = b.build(Tolerance::DEFAULT).unwrap();
    assert_eq!(
        sat_of(&sphere).err(),
        Some(Unwritable::Unsupported("sphere"))
    );
    assert_eq!(
        Unwritable::Unsupported("sphere").to_string(),
        "a sphere is not written as ACIS yet"
    );
}

#[test]
fn a_box_is_saved_in_every_version_and_reads_back() {
    use acadrust::types::DxfVersion;
    let dir = tempfile::tempdir().unwrap();
    for version in [
        DxfVersion::AC1015,
        DxfVersion::AC1018,
        DxfVersion::AC1024,
        DxfVersion::AC1027,
        DxfVersion::AC1032,
    ] {
        let mut doc = Document::new();
        doc.cad_mut().version = version;
        let id = doc.add_box(p(1.0, -2.0, 0.0), p(3.0, 1.0, -4.0)).unwrap();
        assert_eq!(doc.entity(id).unwrap().kind(), "3DSOLID");
        for ext in ["dwg", "dxf"] {
            let path = dir.path().join(format!("caja-{version:?}.{ext}"));
            doc.save(&path)
                .unwrap_or_else(|e| panic!("{version:?} {ext}: {e}"));
            let back = Document::open(&path).unwrap();
            let solids: Vec<_> = back
                .entities()
                .into_iter()
                .filter(|e| e.kind() == "3DSOLID")
                .collect();
            assert_eq!(solids.len(), 1, "{version:?} {ext}");
            let body = back.solid(solids[0].id()).unwrap().unwrap();
            let m = body.mass_properties().unwrap();
            assert!(
                (m.volume - 24.0).abs() < 1e-9,
                "{version:?} {ext}: {}",
                m.volume
            );
            assert!(
                (m.centroid.z() + 2.0).abs() < 1e-9,
                "{version:?} {ext}: below the plane"
            );
        }
    }
}

#[test]
fn a_box_is_one_change_on_the_current_layer() {
    let mut doc = Document::new();
    let layer = doc.create_layer("SOLIDOS").unwrap();
    doc.set_current_layer(layer).unwrap();
    doc.begin_change().unwrap();
    let id = doc.add_box(p(0.0, 0.0, 0.0), p(1.0, 1.0, 1.0)).unwrap();
    let change = doc.commit_change().unwrap();
    assert_eq!(change.summary().created(), [id]);
    assert_eq!(doc.entity(id).unwrap().layer(), "SOLIDOS");
    assert!(matches!(
        doc.add_box(p(0.0, 0.0, 0.0), p(1.0, 1.0, 0.0)),
        Err(DocumentError::Geometry(_))
    ));
}

#[test]
fn the_records_hold_what_acis_reads_and_ours_does_not() {
    // AutoCAD reads more of a record than our reader needs: each coedge's
    // loop, each edge's parameters on its line, each plane's u direction.
    let sat = sat_of(&a_box()).unwrap();
    let at = |ptr: SatPointer| &sat.records[usize::try_from(ptr.0).unwrap()];
    for lp in sat.records.iter().filter(|r| r.is_a("loop")) {
        let first = lp.token_pointer(2).unwrap();
        let mut coedge = first;
        loop {
            let record = at(coedge);
            assert_eq!(record.token_pointer(6).unwrap().0, lp.index, "its loop");
            coedge = record.token_pointer(1).unwrap();
            if coedge == first {
                break;
            }
        }
    }
    for edge in sat.records.iter().filter(|r| r.is_a("edge")) {
        let point = |token: usize| {
            let vertex = at(edge.token_pointer(token).unwrap());
            let p = at(vertex.token_pointer(2).unwrap());
            [1, 2, 3].map(|i| p.token_float(i).unwrap())
        };
        let (a, b) = (point(1), point(3));
        let length = (0..3).map(|i| (b[i] - a[i]).powi(2)).sum::<f64>().sqrt();
        let span = edge.token_float(4).unwrap() - edge.token_float(2).unwrap();
        assert!((span - length).abs() < 1e-12, "{span} {length}");
    }
    for plane in sat.records.iter().filter(|r| r.is_a("plane-surface")) {
        let normal = [4, 5, 6].map(|i| plane.token_float(i).unwrap());
        let u = [7, 8, 9].map(|i| plane.token_float(i).unwrap());
        let dot: f64 = (0..3).map(|i| normal[i] * u[i]).sum();
        let length: f64 = u.iter().map(|c| c * c).sum::<f64>().sqrt();
        assert!(dot.abs() < 1e-12 && (length - 1.0).abs() < 1e-12, "{u:?}");
    }
}

#[test]
fn a_wedge_is_saved_and_reads_back() {
    use acadrust::types::DxfVersion;
    let dir = tempfile::tempdir().unwrap();
    for version in [DxfVersion::AC1015, DxfVersion::AC1032] {
        let mut doc = Document::new();
        doc.cad_mut().version = version;
        doc.add_wedge(p(0.0, 0.0, 0.0), p(4.0, 2.0, 3.0)).unwrap();
        for ext in ["dwg", "dxf"] {
            let path = dir.path().join(format!("cuna-{version:?}.{ext}"));
            doc.save(&path)
                .unwrap_or_else(|e| panic!("{version:?} {ext}: {e}"));
            let back = Document::open(&path).unwrap();
            let id = back
                .entities()
                .into_iter()
                .find(|e| e.kind() == "3DSOLID")
                .unwrap()
                .id();
            let body = back.solid(id).unwrap().unwrap();
            assert_eq!(body.faces().len(), 5, "{version:?} {ext}");
            let m = body.mass_properties().unwrap();
            assert!(
                (m.volume - 12.0).abs() < 1e-9,
                "{version:?} {ext}: {}",
                m.volume
            );
        }
    }
    assert!(matches!(
        Document::new().add_wedge(p(0.0, 0.0, 0.0), p(4.0, 0.0, 3.0)),
        Err(DocumentError::Geometry(_))
    ));
}
