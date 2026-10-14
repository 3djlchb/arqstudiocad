//! What a drawing holds, counted by kind: every entity and object type
//! (unknown and proxy objects included), the table records and the XDATA
//! attached to entities. Comparing the drawing with the file a save wrote
//! shows data the platform does not display yet but must not lose
//! silently (ADR 0017 D5.3).

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::sync::{Mutex, OnceLock};

use acadrust::objects::ObjectType;

use crate::document::Document;
use crate::format::FileFormat;

/// Counts by kind, e.g. `"entity LINE" → 8`, `"object XRECORD" → 263`.
type Inventory = BTreeMap<String, usize>;

/// Something a saved file holds less of than the drawing: data the format
/// or version written cannot keep.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loss {
    kind: String,
    before: usize,
    after: usize,
}

impl Loss {
    /// What is lost, e.g. `"object AECS_DISP_PROPS_MEMBER"`, `"entity
    /// POLYLINE"`, `"XDATA on entities"`, `"layers"`.
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// How many the drawing holds.
    pub fn before(&self) -> usize {
        self.before
    }

    /// How many the written file holds.
    pub fn after(&self) -> usize {
        self.after
    }
}

impl fmt::Display for Loss {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {} → {}", self.kind, self.before, self.after)
    }
}

impl Document {
    /// What this drawing holds, counted by kind.
    fn inventory(&self) -> Inventory {
        let cad = self.cad();
        let mut counts = Inventory::new();
        let mut add = |kind: String| *counts.entry(kind).or_default() += 1;
        for e in cad.entities() {
            add(format!("entity {}", e.as_entity().entity_type()));
            if !e.common().extended_data.is_empty() {
                add("XDATA on entities".to_owned());
            }
        }
        for o in cad.objects.values() {
            add(format!("object {}", object_name(o)));
        }
        let tables = [
            ("layers", cad.layers.len()),
            ("linetypes", cad.line_types.len()),
            ("text styles", cad.text_styles.len()),
            ("block definitions", cad.block_records.len()),
            ("dimension styles", cad.dim_styles.len()),
            ("registered applications", cad.app_ids.len()),
        ];
        for (table, len) in tables {
            if len > 0 {
                counts.insert(table.to_owned(), len);
            }
        }
        counts
    }

    /// What `written`, this drawing read back from a file, holds less of.
    /// Empty when nothing was lost. More of something (a writer adding its
    /// own dictionaries) is not a loss, and neither is a 2D POLYLINE
    /// written as an LWPOLYLINE (as AutoCAD does with PLINETYPE 2): fewer
    /// POLYLINEs are offset by as many more LWPOLYLINEs.
    pub fn losses_in(&self, written: &Document) -> Vec<Loss> {
        let mut found = written.inventory();
        let mut before = self.inventory();
        let count = |inventory: &Inventory, kind: &str| inventory.get(kind).copied().unwrap_or(0);
        let converted = count(&before, POLYLINE)
            .saturating_sub(count(&found, POLYLINE))
            .min(count(&found, LWPOLYLINE).saturating_sub(count(&before, LWPOLYLINE)));
        if converted > 0 {
            *before.entry(POLYLINE.to_owned()).or_default() -= converted;
            *found.entry(LWPOLYLINE.to_owned()).or_default() -= converted;
        }
        before
            .into_iter()
            .filter_map(|(kind, before)| {
                let after = count(&found, &kind);
                (after < before).then_some(Loss {
                    kind,
                    before,
                    after,
                })
            })
            .collect()
    }
}

impl Document {
    /// What saving this drawing as `format` lost, `written` being the file
    /// read back: [`losses_in`](Self::losses_in) less what an empty drawing
    /// loses the same way. The engine gives every drawing default objects
    /// (a multileader style and a table style, with their dictionaries)
    /// that an old version cannot hold: dropping them is not losing the
    /// user's data; anything beyond them is.
    pub(crate) fn save_losses(&self, written: &Document, format: FileFormat) -> Vec<Loss> {
        let defaults = default_losses(format, &self.version_code());
        self.losses_in(written)
            .into_iter()
            .filter(|loss| {
                let lost = loss.before - loss.after;
                lost > defaults.get(&loss.kind).copied().unwrap_or(0)
            })
            .collect()
    }
}

/// What an empty drawing of `version` loses when written as `format` and
/// read back, by kind; computed once per format and version.
fn default_losses(format: FileFormat, version: &str) -> BTreeMap<String, usize> {
    type Cache = Mutex<HashMap<(FileFormat, String), BTreeMap<String, usize>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let cache = CACHE.get_or_init(Mutex::default);
    let key = (format, version.to_owned());
    if let Some(known) = cache.lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
        return known.clone();
    }
    let mut empty = Document::new();
    if let Some(v) = acadrust::types::DxfVersion::parse(version) {
        empty.cad_mut().version = v;
    }
    let bytes = match format {
        FileFormat::Dwg => empty.to_dwg_bytes(),
        FileFormat::Dxf => empty.to_dxf_bytes(),
    };
    let losses: BTreeMap<String, usize> = bytes
        .ok()
        .and_then(|b| crate::reading::read(b, format, std::path::Path::new("empty")).ok())
        .map(|read| {
            empty
                .losses_in(&Document::from_engine(read.cad))
                .into_iter()
                .map(|l| (l.kind, l.before - l.after))
                .collect()
        })
        .unwrap_or_default();
    cache
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(key, losses.clone());
    losses
}

const POLYLINE: &str = "entity POLYLINE";
const LWPOLYLINE: &str = "entity LWPOLYLINE";

/// An object's DXF class name, upper case as in the file.
fn object_name(o: &ObjectType) -> String {
    match o {
        ObjectType::DynamicBlock(d) => d.dxf_name.to_uppercase(),
        ObjectType::Associative(a) => a.dxf_name.to_uppercase(),
        ObjectType::BlockVisibilityParameter(_) => "BLOCKVISIBILITYPARAMETER".to_owned(),
        ObjectType::Unknown { type_name, .. } => type_name.to_uppercase(),
        ObjectType::ProxyObject(_) => "ACAD_PROXY_OBJECT".to_owned(),
        other => {
            let debug = format!("{other:?}");
            let end = debug.find(['(', ' ', '{']).unwrap_or(debug.len());
            debug[..end].to_uppercase()
        }
    }
}

/// Lists losses for a message, at most `MAX` of them.
pub(crate) fn describe(losses: &[Loss]) -> String {
    const MAX: usize = 20;
    let mut text: Vec<String> = losses.iter().take(MAX).map(Loss::to_string).collect();
    if losses.len() > MAX {
        text.push(format!("and {} more", losses.len() - MAX));
    }
    text.join("; ")
}

#[cfg(test)]
#[path = "inventory_tests.rs"]
mod tests;
