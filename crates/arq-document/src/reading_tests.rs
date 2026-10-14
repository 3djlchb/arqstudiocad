//! Regression drawings for damaged files (ADR 0017 D5.1, D5.4): drawings
//! written here in every DWG version and as DXF, then cut, zeroed or
//! corrupted the ways that once went unnoticed.

use std::path::Path;

use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DwgWriter, DxfWriter, EntityType, Line};

use super::read;
use crate::error::DocumentError;
use crate::format::FileFormat;

const LINES: usize = 300;

fn drawing(version: DxfVersion) -> CadDocument {
    let mut cad = CadDocument::new();
    cad.version = version;
    for i in 0..LINES {
        let x = i as f64;
        cad.add_entity(EntityType::Line(Line::from_coords(
            x, 0.0, 0.0, x, 10.0, 0.0,
        )))
        .unwrap();
    }
    cad
}

fn dwg(version: DxfVersion) -> Vec<u8> {
    DwgWriter::write_to_vec(&drawing(version)).unwrap()
}

fn dxf() -> Vec<u8> {
    DxfWriter::new(&drawing(DxfVersion::AC1032))
        .write_to_vec()
        .unwrap()
}

fn cut(bytes: &[u8], percent: usize) -> Vec<u8> {
    bytes[..bytes.len() * percent / 100].to_vec()
}

fn zero_tail(bytes: &[u8]) -> Vec<u8> {
    let mut b = bytes.to_vec();
    let from = b.len() * 3 / 4;
    b[from..].fill(0);
    b
}

fn flip_middle(bytes: &[u8]) -> Vec<u8> {
    let mut b = bytes.to_vec();
    let m = b.len() / 2;
    for byte in &mut b[m..m + 64] {
        *byte ^= 0x5a;
    }
    b
}

const DWG_VERSIONS: [DxfVersion; 8] = [
    DxfVersion::AC1012,
    DxfVersion::AC1014,
    DxfVersion::AC1015,
    DxfVersion::AC1018,
    DxfVersion::AC1021,
    DxfVersion::AC1024,
    DxfVersion::AC1027,
    DxfVersion::AC1032,
];

/// The one rule: a damaged file either fails as `Damaged` or opens marked
/// as recovered; it never opens as a valid drawing with less in it.
fn never_silent(bytes: Vec<u8>, format: FileFormat, what: &str) {
    match read(bytes, format, Path::new("x")) {
        Ok((cad, None)) => assert_eq!(
            cad.entities().count(),
            LINES,
            "{what}: read as intact with entities missing"
        ),
        Ok((_, Some(recovery))) => assert!(!recovery.problems().is_empty(), "{what}"),
        Err(DocumentError::Damaged { problems, .. }) => assert!(!problems.is_empty(), "{what}"),
        Err(other) => panic!("{what}: {other}"),
    }
}

#[test]
fn intact_files_read_clean() {
    for version in DWG_VERSIONS {
        let (cad, recovery) = read(dwg(version), FileFormat::Dwg, Path::new("x")).unwrap();
        assert_eq!(recovery, None, "{version:?}");
        assert_eq!(cad.entities().count(), LINES, "{version:?}");
    }
    let (cad, recovery) = read(dxf(), FileFormat::Dxf, Path::new("x")).unwrap();
    assert_eq!((cad.entities().count(), recovery), (LINES, None));
}

#[test]
fn an_empty_dxf_is_a_valid_drawing() {
    let empty = DxfWriter::new(&CadDocument::new()).write_to_vec().unwrap();
    assert_eq!(
        read(empty, FileFormat::Dxf, Path::new("x")).unwrap().1,
        None
    );
}

/// R13 to 2000 cut short read as an empty drawing without an error.
#[test]
fn truncated_old_dwg_is_damaged_not_empty() {
    for version in [DxfVersion::AC1012, DxfVersion::AC1014, DxfVersion::AC1015] {
        for bytes in [
            cut(&dwg(version), 50),
            cut(&dwg(version), 99),
            zero_tail(&dwg(version)),
        ] {
            let error = read(bytes, FileFormat::Dwg, Path::new("plano.dwg")).unwrap_err();
            let DocumentError::Damaged { path, problems } = &error else {
                panic!("{version:?}: {error}")
            };
            assert_eq!(path, Path::new("plano.dwg"));
            assert!(
                problems
                    .iter()
                    .any(|p| p == "no drawing data could be read"),
                "{version:?}: {problems:?}"
            );
        }
    }
}

/// A corrupt 2007 DWG made the engine panic.
#[test]
fn an_engine_panic_is_a_damaged_file() {
    let error = read(
        flip_middle(&dwg(DxfVersion::AC1021)),
        FileFormat::Dwg,
        Path::new("x"),
    )
    .unwrap_err();
    let DocumentError::Damaged { problems, .. } = error else {
        panic!("{error}")
    };
    assert!(
        problems
            .iter()
            .any(|p| p.starts_with("the reader stopped unexpectedly")),
        "{problems:?}"
    );
}

/// What the strict read refuses but recovery can read opens marked.
#[test]
fn partly_readable_files_open_recovered() {
    let (cad, recovery) = read(
        flip_middle(&dwg(DxfVersion::AC1015)),
        FileFormat::Dwg,
        Path::new("x"),
    )
    .unwrap();
    let recovery = recovery.expect("marked as recovered");
    assert!(!recovery.problems().is_empty());
    assert!(cad.entities().count() <= LINES);
}

#[test]
fn no_damage_goes_unnoticed() {
    for version in DWG_VERSIONS {
        let bytes = dwg(version);
        for (what, damaged) in [
            ("cut 10%", cut(&bytes, 10)),
            ("cut 50%", cut(&bytes, 50)),
            ("cut 90%", cut(&bytes, 90)),
            ("cut 99%", cut(&bytes, 99)),
            ("zeroed tail", zero_tail(&bytes)),
            ("flipped middle", flip_middle(&bytes)),
        ] {
            never_silent(damaged, FileFormat::Dwg, &format!("{version:?} {what}"));
        }
    }
    let bytes = dxf();
    for (what, damaged) in [
        ("cut 50%", cut(&bytes, 50)),
        ("zeroed tail", zero_tail(&bytes)),
        ("flipped middle", flip_middle(&bytes)),
    ] {
        never_silent(damaged, FileFormat::Dxf, &format!("DXF {what}"));
    }
}

#[test]
fn long_problem_lists_are_capped() {
    let many: Vec<String> = (0..30).map(|i| format!("p{i}")).collect();
    let capped = super::capped(many);
    assert_eq!(capped.len(), super::MAX_PROBLEMS + 1);
    assert_eq!(capped.last().unwrap(), "and 10 more");
}
