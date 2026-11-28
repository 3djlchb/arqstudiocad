use super::*;

#[test]
fn parses_simple_and_complex_lin_definitions() {
    let parsed =
        parse_linetype_file("; sample\n*DASHED,Dashed __ __\nA,0.5,-0.25,0,-0.25\n").unwrap();
    assert_eq!(parsed[0].name, "DASHED");
    assert_eq!(parsed[0].description, "Dashed __ __");
    assert_eq!(parsed[0].pattern, [0.5, -0.25, 0.0, -0.25]);
    let parsed = parse_linetype_file(
        "*TEXT,Text\nA,.5,-.2,[\"HW, GAS\",STANDARD,S=.1,U=0,X=-.1,Y=-.05],-.2\n",
    )
    .unwrap();
    assert_eq!(parsed[0].pattern, [0.5, -0.2, -0.2]);
    assert_eq!(parsed[0].complex[0].segment_index, 1);
    assert_eq!(
        parsed[0].complex[0].content,
        LinetypeComplexContent::Text {
            text: "HW, GAS".to_owned(),
            style_name: "STANDARD".to_owned(),
        }
    );
    assert_eq!(
        parsed[0].complex[0].rotation_mode,
        LinetypeRotationMode::Upright
    );
    assert_eq!(parsed[0].complex[0].offset, [-0.1, -0.05]);

    let shape = parse_linetype_file(
        "*FENCE,Fence\nA,6.35,-2.54,[CIRC1,ltypeshp.shx,X=-2.54,S=2.54],-2.54\n",
    )
    .unwrap();
    assert_eq!(
        shape[0].complex[0].content,
        LinetypeComplexContent::Shape {
            name: "CIRC1".to_owned(),
            shape_file: "ltypeshp.shx".to_owned(),
        }
    );
    assert!(parse_linetype_file("*BAD,Bad\nA,1,[CIRC1],-1\n").is_err());

    let escaped = parse_linetype_file(
        r#"*QUOTED,Quoted text
A,1,["A \"B\"",STANDARD],-1
"#,
    )
    .unwrap();
    assert_eq!(
        escaped[0].complex[0].content,
        LinetypeComplexContent::Text {
            text: "A \"B\"".to_owned(),
            style_name: "STANDARD".to_owned(),
        }
    );
}

#[test]
fn resolved_shx_shape_number_is_written_to_the_linetype_record() {
    let mut definition = parse_linetype_file("*FENCE,Fence\nA,1,-1,[CIRC1,ltypeshp.shx,S=1],-1\n")
        .unwrap()
        .remove(0);
    let LinetypeComplexContent::Shape { name, .. } = &mut definition.complex[0].content else {
        panic!("expected SHX shape descriptor");
    };
    *name = "#42".to_owned();

    let entry = codec_linetype(&definition);
    let complex = entry.elements[1].complex.as_ref().unwrap();
    assert_eq!(
        complex.content,
        CodecLinetypeComplexContent::Shape { shape_number: 42 }
    );
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
        complex: Vec::new(),
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
