use super::*;
use crate::document::Document;

fn info(product: &str, version: &str, comment: &str) -> AppInfo {
    AppInfo {
        name: "AppInfoDataList".to_owned(),
        version: version.to_owned(),
        comment: comment.to_owned(),
        product: product.to_owned(),
    }
}

fn named(name: &str) -> AppInfo {
    info(&format!("<ProductInformation name=\"{name}\"/>"), "1.0", "")
}

#[test]
fn each_producer_is_known_by_what_its_app_info_says() {
    // AutoCAD and the ODA, as the public samples write them.
    let autocad = info(
        "\"<ProductInformation name =\\\"AutoCAD\\\" build_version=\\\"R.47\\\"/>\"",
        "22.0.48.M.294",
        "Autodesk DWG.  This file is a Trusted DWG last saved by an Autodesk application \
         or Autodesk licensed application.",
    );
    assert_eq!(Producer::of(Some(&autocad)), Producer::AutoCad);
    let teigha = info(
        "<ProductInformation name =\"Teigha®\" CompanyName = \"Open Design Alliance\"/>",
        "Teigha(R) 4.3.2.0",
        "This file was last saved by an Open Design Alliance (ODA) application or an ODA \
         licensed application.",
    );
    assert_eq!(Producer::of(Some(&teigha)), Producer::Oda);
    // The others, by the names they give themselves (to confirm).
    for (name, producer) in [
        ("progeCAD 2026 Professional", Producer::IntelliCad),
        ("IntelliCAD", Producer::IntelliCad),
        ("BricsCAD", Producer::BricsCad),
        ("ARES Commander", Producer::AresCommander),
        ("ARES", Producer::AresCommander),
        ("GstarCAD", Producer::GstarCad),
        ("Edificius", Producer::Edificius),
        ("Vectorworks", Producer::Vectorworks),
    ] {
        assert_eq!(Producer::of(Some(&named(name))), producer, "{name}");
    }
    // A program of no list, by its name; nothing at all, unknown.
    assert_eq!(
        Producer::of(Some(&named("acadrust"))),
        Producer::Other("acadrust".to_owned())
    );
    assert_eq!(Producer::of(None), Producer::Unknown);
    assert_eq!(
        Producer::of(Some(&info("", "", ""))),
        Producer::Unknown,
        "an empty section names nobody"
    );
}

#[test]
fn a_word_holding_ares_is_not_ares_commander() {
    let squares = info(
        "<ProductInformation name=\"Squares Studio\"/>",
        "1.0",
        "It shares nothing",
    );
    assert_eq!(
        Producer::of(Some(&squares)),
        Producer::Other("Squares Studio".to_owned())
    );
}

#[test]
fn a_drawing_made_here_or_saved_here_is_ours_and_a_dxf_says_nothing() {
    set_writer("TestCAD", "9.8.7");
    assert_eq!(Document::new().producer(), &Producer::Ours);
    // Saved as DWG, it names us, and is ours when read again.
    let doc = Document::new();
    let back = Document::read(
        doc.to_dwg_bytes().unwrap(),
        crate::FileFormat::Dwg,
        std::path::Path::new("ours.dwg"),
    )
    .unwrap();
    assert_eq!(back.producer(), &Producer::Ours);
    let app = back.cad().app_info.clone().unwrap();
    assert_eq!(app.product_attribute("name").as_deref(), Some("TestCAD"));
    assert_eq!(app.version, "9.8.7");
    // A DXF has no AppInfo: unknown.
    let dxf = Document::from_dxf_bytes(doc.to_dxf_bytes().unwrap()).unwrap();
    assert_eq!(dxf.producer(), &Producer::Unknown);
    // A drawing read from AutoCAD stays AutoCAD's, but is saved as ours.
    let mut from_autocad = Document::new();
    from_autocad.cad_mut().app_info = Some(named("AutoCAD"));
    let read = Document::read(
        from_autocad.to_dwg_bytes().unwrap(),
        crate::FileFormat::Dwg,
        std::path::Path::new("saved.dwg"),
    )
    .unwrap();
    assert_eq!(read.producer(), &Producer::Ours, "saved by us");
}

#[test]
fn keys_are_stable() {
    let all = [
        Producer::AutoCad,
        Producer::IntelliCad,
        Producer::BricsCad,
        Producer::AresCommander,
        Producer::GstarCad,
        Producer::Edificius,
        Producer::Vectorworks,
        Producer::Oda,
        Producer::Ours,
        Producer::Other("x".to_owned()),
        Producer::Unknown,
    ];
    let keys: Vec<&str> = all.iter().map(Producer::key).collect();
    assert_eq!(
        keys,
        [
            "autocad",
            "intellicad",
            "bricscad",
            "ares",
            "gstarcad",
            "edificius",
            "vectorworks",
            "oda",
            "ours",
            "other",
            "unknown"
        ]
    );
}
