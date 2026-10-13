//! DXF drawings written in tests, shared by this crate's test modules.

use crate::document::Document;

/// A DXF from group-code pairs: header, layers, blocks and entities.
pub(crate) fn dxf(
    layers: &[(&str, i32, i32)],
    blocks: &[&[&str]],
    entities: &[&[&str]],
) -> Document {
    let mut codes: Vec<String> = Vec::new();
    let mut add = |items: &[&str]| codes.extend(items.iter().map(|s| (*s).to_owned()));
    add(&[
        "0", "SECTION", "2", "HEADER", "9", "$ACADVER", "1", "AC1009", "0", "ENDSEC",
    ]);
    add(&[
        "0", "SECTION", "2", "TABLES", "0", "TABLE", "2", "LAYER", "70", "9",
    ]);
    for (name, flags, color) in layers {
        let (flags, color) = (flags.to_string(), color.to_string());
        add(&[
            "0",
            "LAYER",
            "2",
            name,
            "70",
            &flags,
            "62",
            &color,
            "6",
            "CONTINUOUS",
        ]);
    }
    add(&["0", "ENDTAB", "0", "ENDSEC", "0", "SECTION", "2", "BLOCKS"]);
    for block in blocks {
        add(block);
    }
    add(&["0", "ENDSEC", "0", "SECTION", "2", "ENTITIES"]);
    for entity in entities {
        add(entity);
    }
    add(&["0", "ENDSEC", "0", "EOF", ""]);
    Document::from_dxf_bytes(codes.join("\n").into_bytes()).unwrap()
}

pub(crate) const LAYERS: &[(&str, i32, i32)] = &[
    ("0", 0, 7),
    ("MUROS", 0, 1),
    ("A-PUERTAS", 0, 5),
    ("APAGADA", 0, -3),
    ("CONGELADA", 1, 4),
];

/// Block PUERTA, base (1,1): a LINE (1,1)-(2,1) on layer 0, colored by
/// block, and a LINE (1,1)-(1,2) on MUROS, colored by layer.
pub(crate) fn door_block() -> Vec<&'static str> {
    vec![
        "0", "BLOCK", "8", "0", "2", "PUERTA", "70", "0", "10", "1", "20", "1", "30", "0", "3",
        "PUERTA", //
        "0", "LINE", "8", "0", "62", "0", "10", "1", "20", "1", "30", "0", "11", "2", "21", "1",
        "31", "0", //
        "0", "LINE", "8", "MUROS", "62", "256", "10", "1", "20", "1", "30", "0", "11", "1", "21",
        "2", "31", "0", //
        "0", "ENDBLK", "8", "0",
    ]
}
