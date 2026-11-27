use super::*;

#[test]
fn parses_simple_lin_definitions_and_rejects_complex_ones() {
    let parsed =
        parse_linetype_file("; sample\n*DASHED,Dashed __ __\nA,0.5,-0.25,0,-0.25\n").unwrap();
    assert_eq!(parsed[0].name, "DASHED");
    assert_eq!(parsed[0].description, "Dashed __ __");
    assert_eq!(parsed[0].pattern, [0.5, -0.25, 0.0, -0.25]);
    let error = parse_linetype_file("*TEXT,Text\nA,0.5,[TXT,STANDARD,S=0.1],-0.2\n").unwrap_err();
    assert!(error.contains("complex text/shape"));
}

#[test]
fn linetype_loading_is_all_or_nothing_and_part_of_undo_recording() {
    let mut doc = crate::Document::new();
    doc.begin_change().unwrap();
    let values = parse_linetype_file("*DASHED,Dashed\nA,0.5,-0.25\n").unwrap();
    doc.load_linetypes(&values).unwrap();
    let change = doc.commit_change().unwrap();
    assert_eq!(doc.linetypes(), ["Continuous", "DASHED"]);
    assert!(change.summary().document_changed());

    doc.begin_change().unwrap();
    let invalid = LinetypeDefinition {
        name: "BROKEN".into(),
        description: String::new(),
        pattern: vec![f64::NAN],
    };
    assert!(doc.load_linetypes(&[invalid]).is_err());
    doc.rollback_change().unwrap();
    assert!(doc.linetypes().iter().all(|name| name != "BROKEN"));
}

#[test]
fn simple_linetype_definitions_survive_dxf_round_trip() {
    let mut doc = crate::Document::new();
    let definitions = parse_linetype_file("*DASHDOT,Dash dot\nA,0.5,-0.25,0,-0.25\n").unwrap();
    doc.begin_change().unwrap();
    doc.load_linetypes(&definitions).unwrap();
    doc.commit_change().unwrap();

    let bytes = doc.to_dxf_bytes().unwrap();
    let reopened = crate::Document::from_dxf_bytes(bytes).unwrap();
    let round_trip = reopened
        .linetype_definitions()
        .into_iter()
        .find(|item| item.name == "DASHDOT")
        .unwrap();
    assert_eq!(round_trip.description, "Dash dot");
    assert_eq!(round_trip.pattern, [0.5, -0.25, 0.0, -0.25]);
}
