//! Tests of the ACIS writer (a submodule of `acis_write`): what it writes
//! reads back, through SAT text and SAB binary, as the same body.

use acadrust::entities::acis::types::SatDocument;
use arq_geometry::{Point3, Tolerance, Transform3, Vector3};
use arq_kernel::{Body, MassProperties, Surface};

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
    // A whole elliptic cylinder's side: not written yet.
    let mut b = arq_kernel::BodyBuilder::new();
    let section = arq_kernel::EllipseCurve::new(
        p(0.0, 0.0, 0.0),
        Vector3::new(0.0, 0.0, 1.0).unwrap(),
        Vector3::new(2.0, 0.0, 0.0).unwrap(),
        0.5,
    )
    .unwrap();
    let side = Surface::EllipticCylinder(arq_kernel::EllipticCylinder::new(section));
    let face = b.face(arq_kernel::Face {
        surface: side,
        loops: vec![],
        reversed: false,
    });
    let shell = b.shell(arq_kernel::Shell { faces: vec![face] });
    b.lump(arq_kernel::Lump {
        shells: vec![shell],
    });
    let oval = b.build(Tolerance::DEFAULT).unwrap();
    assert_eq!(
        sat_of(&oval).err(),
        Some(Unwritable::Unsupported("elliptic cylinder"))
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

/// A cylinder, a cone to a point, a frustum, one downward and one upside
/// down, each standing on (2, 3, 0).
fn round_solids() -> Vec<(&'static str, Body)> {
    let at = p(2.0, 3.0, 0.0);
    vec![
        ("cylinder", Body::cylinder(at, 1.5, 4.0).unwrap()),
        ("cone", Body::cone(at, 3.0, 0.0, 4.0).unwrap()),
        ("frustum", Body::cone(at, 3.0, 1.0, 4.0).unwrap()),
        ("downward", Body::cone(at, 3.0, 1.0, -4.0).unwrap()),
        ("upside down", Body::cone(at, 0.0, 2.0, 4.0).unwrap()),
    ]
}

#[test]
fn cylinders_and_cones_read_back_the_same() {
    for (name, body) in round_solids() {
        let sat = sat_of(&body).unwrap_or_else(|e| panic!("{name}: {e}"));
        let measures = body.mass_properties().unwrap();
        for back in read_back(&sat) {
            back.check(Tolerance::DEFAULT)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(back.faces().len(), body.faces().len(), "{name}");
            same(&back.mass_properties().unwrap(), &measures);
        }
    }
}

#[test]
fn circles_span_a_turn_and_an_apex_nothing() {
    let sat = sat_of(&Body::cone(p(0.0, 0.0, 0.0), 3.0, 0.0, 4.0).unwrap()).unwrap();
    let edges: Vec<_> = sat.records.iter().filter(|r| r.is_a("edge")).collect();
    assert_eq!(edges.len(), 2);
    for edge in edges {
        let (from, to) = (edge.token_float(2).unwrap(), edge.token_float(4).unwrap());
        if edge.token_pointer(6).unwrap().is_null() {
            assert_eq!((from, to), (1.0, 0.0), "the apex");
        } else {
            assert!(
                (to - from - std::f64::consts::TAU).abs() < 1e-12,
                "{from} {to}"
            );
        }
    }
    let cones = sat
        .records
        .iter()
        .filter(|r| r.is_a("cone-surface"))
        .count();
    assert_eq!(cones, 1);
}

#[test]
fn cylinders_and_cones_are_saved_and_read_back() {
    use acadrust::types::DxfVersion;
    let dir = tempfile::tempdir().unwrap();
    for version in [DxfVersion::AC1015, DxfVersion::AC1024, DxfVersion::AC1032] {
        let mut doc = Document::new();
        doc.cad_mut().version = version;
        let mut volumes = Vec::new();
        for (_, body) in round_solids() {
            volumes.push(body.mass_properties().unwrap().volume);
            doc.add_solid(&body).unwrap();
        }
        for ext in ["dwg", "dxf"] {
            let path = dir.path().join(format!("redondos-{version:?}.{ext}"));
            doc.save(&path)
                .unwrap_or_else(|e| panic!("{version:?} {ext}: {e}"));
            let back = Document::open(&path).unwrap();
            let read: Vec<f64> = back
                .entities()
                .into_iter()
                .filter(|e| e.kind() == "3DSOLID")
                .map(|e| {
                    back.solid(e.id())
                        .unwrap()
                        .unwrap()
                        .mass_properties()
                        .unwrap()
                        .volume
                })
                .collect();
            assert_eq!(read.len(), volumes.len(), "{version:?} {ext}");
            for (a, b) in read.iter().zip(&volumes) {
                assert!((a - b).abs() < 1e-9 * b, "{version:?} {ext}: {a} {b}");
            }
        }
    }
}

#[test]
fn an_edge_against_its_circle_is_written_the_way_it_runs() {
    // A cylinder whose base circle is defined turned over, its edge marked
    // as running against it: the same cylinder, written along the edge.
    use arq_kernel::{
        BodyBuilder, Coedge, Cone, Edge, EllipseCurve, Face, Loop, Lump, Plane, Shell,
    };
    let z = Vector3::new(0.0, 0.0, 1.0).unwrap();
    let x = Vector3::new(1.0, 0.0, 0.0).unwrap();
    let (r, h) = (1.5, 2.0);
    let mut b = BodyBuilder::new();
    let low = b.vertex(p(r, 0.0, 0.0));
    let high = b.vertex(p(r, 0.0, h));
    let circle = |center: Point3, normal: Vector3| {
        Some(Curve::Ellipse(
            EllipseCurve::new(center, normal, x * r, 1.0).unwrap(),
        ))
    };
    let bottom = b.edge(Edge {
        curve: circle(p(0.0, 0.0, 0.0), -z),
        start: low,
        end: low,
        reversed: true,
    });
    let top = b.edge(Edge {
        curve: circle(p(0.0, 0.0, h), z),
        start: high,
        end: high,
        reversed: false,
    });
    let mut one = |edge, reversed| {
        let c = b.coedge(Coedge { edge, reversed });
        b.add_loop(Loop { coedges: vec![c] })
    };
    let (under, over, side_low, side_high) = (
        one(bottom, true),
        one(top, false),
        one(bottom, false),
        one(top, true),
    );
    let plane = |at: Point3, n: Vector3| Surface::Plane(Plane::new(at, n).unwrap());
    let faces = vec![
        b.face(Face {
            surface: plane(p(0.0, 0.0, 0.0), -z),
            loops: vec![under],
            reversed: false,
        }),
        b.face(Face {
            surface: plane(p(0.0, 0.0, h), z),
            loops: vec![over],
            reversed: false,
        }),
        b.face(Face {
            surface: Surface::Cone(Cone::new(p(0.0, 0.0, 0.0), z, r, 0.0, 1.0).unwrap()),
            loops: vec![side_low, side_high],
            reversed: false,
        }),
    ];
    let shell = b.shell(Shell { faces });
    b.lump(Lump {
        shells: vec![shell],
    });
    let body = b.build(Tolerance::DEFAULT).unwrap();
    let measures = body.mass_properties().unwrap();
    assert!((measures.volume - std::f64::consts::PI * r * r * h).abs() < 1e-9);
    for back in read_back(&sat_of(&body).unwrap()) {
        same(&back.mass_properties().unwrap(), &measures);
    }
}

#[test]
fn no_cone_is_written_with_a_circle_of_no_size() {
    // An upside-down cone's base is a point: its surface is written from
    // its top circle, which ACIS needs a size for.
    for (name, body) in round_solids() {
        let sat = sat_of(&body).unwrap();
        for cone in sat.records.iter().filter(|r| r.is_a("cone-surface")) {
            let major = [7, 8, 9].map(|i| cone.token_float(i).unwrap());
            let length = major.iter().map(|c| c * c).sum::<f64>().sqrt();
            assert!(length > 0.0, "{name}");
        }
    }
}

#[test]
fn spheres_and_tori_read_back_and_are_saved() {
    use acadrust::types::DxfVersion;
    let solids = [
        Body::sphere(p(1.0, 2.0, 3.0), 2.0).unwrap(),
        Body::torus(p(10.0, 0.0, 0.0), 3.0, 1.0).unwrap(),
    ];
    for body in &solids {
        let measures = body.mass_properties().unwrap();
        for back in read_back(&sat_of(body).unwrap()) {
            assert_eq!((back.faces().len(), back.loops().len()), (1, 0));
            same(&back.mass_properties().unwrap(), &measures);
        }
    }
    let dir = tempfile::tempdir().unwrap();
    for version in [DxfVersion::AC1015, DxfVersion::AC1032] {
        let mut doc = Document::new();
        doc.cad_mut().version = version;
        doc.add_sphere(p(1.0, 2.0, 3.0), 2.0).unwrap();
        doc.add_torus(p(10.0, 0.0, 0.0), 3.0, 1.0).unwrap();
        for ext in ["dwg", "dxf"] {
            let path = dir.path().join(format!("esfera-{version:?}.{ext}"));
            doc.save(&path)
                .unwrap_or_else(|e| panic!("{version:?} {ext}: {e}"));
            let back = Document::open(&path).unwrap();
            let volumes: Vec<f64> = back
                .entities()
                .into_iter()
                .filter(|e| e.kind() == "3DSOLID")
                .map(|e| {
                    back.solid(e.id())
                        .unwrap()
                        .unwrap()
                        .mass_properties()
                        .unwrap()
                        .volume
                })
                .collect();
            let pi = std::f64::consts::PI;
            let expected = [32.0 / 3.0 * pi, 6.0 * pi * pi];
            assert_eq!(volumes.len(), 2, "{version:?} {ext}");
            for (a, b) in volumes.iter().zip(expected) {
                assert!((a - b).abs() < 1e-9 * b, "{version:?} {ext}: {a} {b}");
            }
        }
    }
    assert!(matches!(
        Document::new().add_torus(p(0.0, 0.0, 0.0), 1.0, 2.0),
        Err(DocumentError::Geometry(_))
    ));
}

#[test]
fn a_void_and_a_second_lump_survive_dwg_2000() {
    // Saved in DWG 2000 the ACIS is SAT 400, which lacks ACIS 7.0's added
    // pointer: a shell is `$attrib $next $subshell $face $wire $lump`, a
    // lump `$attrib $next $shell $body`. Read back by the codec alone the
    // layout cannot be judged (a wrong writer and a wrong reader agree),
    // so the records are checked as written: ProgeCAD read a hollow box
    // as solid when the void sat where the subshell goes.
    use acadrust::types::DxfVersion;
    let hollow = Body::cuboid(p(0.0, 0.0, 0.0), p(3.0, 3.0, 3.0))
        .unwrap()
        .difference(&Body::cuboid(p(1.0, 1.0, 1.0), p(2.0, 2.0, 2.0)).unwrap())
        .unwrap()
        .unwrap();
    let apart = Body::cuboid(p(10.0, 0.0, 0.0), p(11.0, 1.0, 1.0))
        .unwrap()
        .union(&Body::cuboid(p(13.0, 0.0, 0.0), p(14.0, 1.0, 1.0)).unwrap())
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut doc = Document::new();
    doc.cad_mut().version = DxfVersion::AC1015;
    doc.add_solid(&hollow).unwrap();
    doc.add_solid(&apart).unwrap();
    let path = dir.path().join("huecos-2000.dwg");
    doc.save(&path).unwrap();
    let back = Document::open(&path).unwrap();
    let mut seen = 0;
    for e in back.cad().entities() {
        let Some(data) = crate::acis::acis_data(e) else {
            continue;
        };
        let text = &data.sat_data;
        assert!(text.starts_with("400 "), "SAT 400 in DWG 2000");
        // The first pointer after the attribute: a shell's or lump's next.
        let next = |kind: &str| -> Vec<String> {
            text.lines()
                .filter(|l| l.starts_with(kind))
                .map(|l| l.split_whitespace().nth(2).unwrap().to_owned())
                .collect()
        };
        let (shells, lumps) = (next("shell "), next("lump "));
        if lumps.len() == 1 {
            assert_eq!(shells.len(), 2, "the outside and the void");
            assert_ne!(shells[0], "$-1", "the outer shell leads to the void");
            seen += 1;
        } else {
            assert_eq!(lumps.len(), 2);
            assert_ne!(lumps[0], "$-1", "the first lump leads to the second");
            seen += 1;
        }
        let m = back
            .solid(e.as_entity().handle().value())
            .unwrap()
            .unwrap()
            .mass_properties()
            .unwrap();
        let near = |v: f64| (m.volume - v).abs() < 1e-9 * v;
        assert!(near(26.0) || near(2.0), "{}", m.volume);
    }
    assert_eq!(seen, 2);
}
