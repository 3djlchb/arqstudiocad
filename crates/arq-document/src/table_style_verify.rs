//! Prove legacy companion conversion; never exempt DATAOBJECT by kind alone.
use acadrust::objects::{
    CellStyleMap, DataObject, DataObjectData, Dictionary, ObjectType, TableStyle,
};

use crate::Document;

const KEY: &str = "ACAD_ROUNDTRIP_2008_TABLESTYLE_CELLSTYLEMAP";

fn same_row(
    source: &Document,
    written: &Document,
    a: &acadrust::objects::RowCellStyle,
    b: &acadrust::objects::RowCellStyle,
) -> bool {
    // DXF stores the row's text style by name; DWG stores its handle. Both
    // must resolve to the same existing record before comparing row values.
    let resolve = |doc: &Document, row: &acadrust::objects::RowCellStyle| {
        doc.cad()
            .text_styles
            .get(&row.text_style_name)
            .filter(|style| {
                row.text_style_handle
                    .is_none_or(|handle| handle == style.handle)
            })
            .map(|style| style.handle)
    };
    let Some(handle) = resolve(source, a) else {
        return false;
    };
    if resolve(written, b) != Some(handle) {
        return false;
    }
    let (mut a, mut b) = (a.clone(), b.clone());
    a.text_style_handle = None;
    b.text_style_handle = None;
    for row in [&mut a, &mut b] {
        // Legacy rows do not encode the stored color of a disabled fill,
        // or double-line spacing for single borders. The complete modern
        // cells (including these stored fields) are compared separately.
        if !row.fill_enabled {
            row.fill_color = acadrust::types::Color::ByBlock;
        }
        for border in [
            &mut row.left_border,
            &mut row.right_border,
            &mut row.top_border,
            &mut row.bottom_border,
            &mut row.horizontal_inside_border,
            &mut row.vertical_inside_border,
        ] {
            if border.border_type == acadrust::objects::TableBorderType::Single {
                border.double_line_spacing = 0.0;
            }
        }
    }
    a == b
}

fn same_base(a: &TableStyle, b: &TableStyle) -> bool {
    use acadrust::objects::{NamedTableCellStyle, TableCellStyleData, TableContentFormat};
    use acadrust::types::Color;
    match (&a.modern_style, &b.modern_style) {
        (Some(expected), Some(actual)) => expected == actual,
        (Some(_), None) => false,
        (None, None) => true,
        (None, Some(actual)) => {
            // A legacy style has no base record. Accept only the canonical
            // base with its explicit margins, not arbitrary extra properties.
            let expected = NamedTableCellStyle {
                cell_style: TableCellStyleData {
                    style_type: 5,
                    data_flags: 1,
                    background_color: Color::None,
                    content_layout: 1,
                    content_format: TableContentFormat {
                        value_data_type: 512,
                        cell_alignment: 1,
                        text_height: 0.18,
                        ..Default::default()
                    },
                    margin_override_flags: 1,
                    vertical_margin: a.vertical_margin,
                    horizontal_margin: a.horizontal_margin,
                    bottom_margin: a.vertical_margin,
                    right_margin: a.horizontal_margin,
                    horizontal_spacing: 0.18,
                    vertical_spacing: 0.18,
                    ..Default::default()
                },
                id: 4,
                style_type: 2,
                name: "Table".to_owned(),
            };
            actual == &expected
        }
    }
}

fn same_style(source: &Document, written: &Document, a: &TableStyle, b: &TableStyle) -> bool {
    a.name == b.name
        && a.owner_handle == b.owner_handle
        && a.description == b.description
        && a.version == b.version
        && a.annotative == b.annotative
        && a.flow_direction == b.flow_direction
        && (a.flags.bits() & !8) == (b.flags.bits() & !8)
        && a.horizontal_margin == b.horizontal_margin
        && a.vertical_margin == b.vertical_margin
        && a.has_title_row() == b.has_title_row()
        && a.has_header_row() == b.has_header_row()
        && same_row(source, written, &a.data_row_style, &b.data_row_style)
        && same_row(source, written, &a.title_row_style, &b.title_row_style)
        && same_row(source, written, &a.header_row_style, &b.header_row_style)
        && a.modern_unknown_byte == b.modern_unknown_byte
        && a.modern_unknown_long2 == b.modern_unknown_long2
        && a.modern_cell_style_handle == b.modern_cell_style_handle
        && a.modern_overrides == b.modern_overrides
        && same_base(a, b)
}

fn companion<'a>(
    doc: &'a Document,
    style: &TableStyle,
) -> Option<(&'a Dictionary, &'a DataObject, &'a CellStyleMap)> {
    let cad = doc.cad();
    let dictionary = cad.extension_dictionary_handle(style.handle)?;
    let ObjectType::Dictionary(entries) = cad.objects.get(&dictionary)? else {
        return None;
    };
    let ObjectType::DataObject(object) = cad.objects.get(&entries.get(KEY)?)? else {
        return None;
    };
    let DataObjectData::CellStyleMap(cells) = &object.data else {
        return None;
    };
    Some((entries, object, cells))
}

fn converted(source: &Document, written: &Document, style: &TableStyle) -> bool {
    if written.version_code().as_str() < "AC1024" {
        return false;
    }
    let Some((dictionary, object, cells)) = companion(source, style) else {
        return false;
    };
    let Some(ObjectType::TableStyle(back)) = written.cad().objects.get(&style.handle) else {
        return false;
    };
    let Some(ObjectType::Dictionary(back_dictionary)) =
        written.cad().objects.get(&dictionary.handle)
    else {
        return false;
    };
    dictionary.owner == style.handle
        && object.owner == dictionary.handle
        && object.xdictionary_handle.is_none()
        && object.reactors.iter().all(|h| *h == dictionary.handle)
        && style.represents_cell_style_map(cells)
        && back.represents_cell_style_map(cells)
        && same_style(source, written, style, back)
        && !written.cad().objects.contains_key(&object.handle)
        && back_dictionary.owner == dictionary.owner
        && back_dictionary.get(KEY).is_none()
        && written.cad().extension_dictionary_handle(style.handle) == Some(dictionary.handle)
}

impl Document {
    pub(crate) fn converted_table_style_maps(&self, written: &Document) -> usize {
        self.cad().objects.values().filter(|object| {
            matches!(object, ObjectType::TableStyle(style) if converted(self, written, style))
        }).count()
    }

    pub(crate) fn compare_table_style_maps(
        &self,
        written: &Document,
        differences: &mut Vec<String>,
    ) {
        for object in self.cad().objects.values() {
            let ObjectType::TableStyle(style) = object else {
                continue;
            };
            let Some((dictionary, map, _)) = companion(self, style) else {
                continue;
            };
            if converted(self, written, style) {
                continue;
            }
            let retained = matches!(written.cad().objects.get(&style.handle), Some(ObjectType::TableStyle(back)) if same_style(self, written, style, back))
                && written.cad().objects.get(&map.handle)
                    == Some(&ObjectType::DataObject(map.clone()))
                && matches!(written.cad().objects.get(&dictionary.handle), Some(ObjectType::Dictionary(back)) if back.owner == dictionary.owner && back.get(KEY) == Some(map.handle))
                && written.cad().extension_dictionary_handle(style.handle)
                    == Some(dictionary.handle);
            if !retained {
                differences.push(format!(
                    "Table style {} companion conversion changed data or links",
                    style.name
                ));
            }
        }
    }
}

#[cfg(test)]
#[path = "table_style_verify_tests.rs"]
mod tests;
