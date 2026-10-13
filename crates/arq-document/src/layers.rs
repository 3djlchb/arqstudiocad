//! Layers: the DWG LAYER table, behind our own API.
//!
//! Layer names ignore case, as in AutoCAD. Entities refer to their layer by
//! name inside the DWG; renaming a layer updates every reference.

use acadrust::Handle;
use acadrust::tables::layer::Layer as CodecLayer;
use acadrust::tables::normalize_name;

use crate::change::LayerNote;
use crate::document::Document;
use crate::entity::Color;
use crate::error::{DocumentError, DocumentResult};
use crate::translate::{color_from_codec, color_to_codec};

/// Identity of a layer: its DWG handle (ADR 0011, D3.6).
pub type LayerId = u64;

/// A snapshot of one layer.
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    id: LayerId,
    name: String,
    color: Color,
    linetype: String,
    visible: bool,
    frozen: bool,
    locked: bool,
    plottable: bool,
}

impl Layer {
    /// Its id (DWG handle).
    pub fn id(&self) -> LayerId {
        self.id
    }
    /// Its name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Its color: an ACI index or RGB.
    pub fn color(&self) -> Color {
        self.color
    }
    /// Its linetype name (read-only for now, ADR 0011 D3.11).
    pub fn linetype(&self) -> &str {
        &self.linetype
    }
    /// Whether it is on (shown).
    pub fn is_visible(&self) -> bool {
        self.visible
    }
    /// Whether it is frozen (hidden and excluded from regeneration).
    pub fn is_frozen(&self) -> bool {
        self.frozen
    }
    /// Whether it is locked (visible but not editable).
    pub fn is_locked(&self) -> bool {
        self.locked
    }
    /// Whether it is plotted.
    pub fn is_plottable(&self) -> bool {
        self.plottable
    }
}

pub(crate) fn read_layer(l: &CodecLayer) -> Layer {
    Layer {
        id: l.handle.value(),
        name: l.name.clone(),
        color: color_from_codec(l.color),
        linetype: l.line_type.clone(),
        visible: !l.flags.off,
        frozen: l.flags.frozen,
        locked: l.flags.locked,
        plottable: l.is_plottable,
    }
}

/// DWG symbol-table name rules, plus no surrounding spaces and at most 255
/// characters.
fn validate_name(name: &str) -> DocumentResult<()> {
    let forbidden = |c: char| {
        c.is_control()
            || matches!(
                c,
                '<' | '>' | '/' | '\\' | '"' | ':' | ';' | '?' | '*' | '|' | ',' | '=' | '`'
            )
    };
    if name.is_empty()
        || name.trim() != name
        || name.chars().count() > 255
        || name.chars().any(forbidden)
    {
        return Err(DocumentError::InvalidName(name.to_owned()));
    }
    Ok(())
}

/// "0", "Defpoints" and external-reference layers keep their name and exist.
fn is_protected(name: &str) -> bool {
    let key = normalize_name(name);
    key == normalize_name("0") || key == normalize_name("Defpoints") || name.contains('|')
}

impl Document {
    /// All layers, "0" first and the rest by name (ignoring case), from
    /// the own model.
    pub fn layers(&self) -> Vec<Layer> {
        let mut layers: Vec<Layer> = self.model.layers.values().cloned().collect();
        layers.sort_by_key(|l| (l.name != "0", l.name.to_lowercase()));
        layers
    }

    /// The layer with this id.
    pub fn layer(&self, id: LayerId) -> Option<Layer> {
        self.model.layers.get(&id).cloned()
    }

    /// The layer with this name (ignoring case).
    pub fn layer_by_name(&self, name: &str) -> Option<Layer> {
        self.model_layer_by_name(name).cloned()
    }

    fn codec_layer(&self, id: LayerId) -> Option<&CodecLayer> {
        self.cad().layers.iter().find(|l| l.handle.value() == id)
    }

    /// Applies `edit` to the layer, recording it as modified.
    fn edit_layer(
        &mut self,
        id: LayerId,
        edit: impl FnOnce(&mut CodecLayer),
    ) -> DocumentResult<()> {
        if self.codec_layer(id).is_none() {
            return Err(DocumentError::LayerIdNotFound(id));
        }
        self.touch_structure();
        if let Some(layer) = self
            .cad_mut()
            .layers
            .iter_mut()
            .find(|l| l.handle.value() == id)
        {
            edit(layer);
        }
        self.sync_layers();
        self.note_layer(id, LayerNote::Modified);
        Ok(())
    }

    fn layer_name(&self, id: LayerId) -> DocumentResult<String> {
        self.codec_layer(id)
            .map(|l| l.name.clone())
            .ok_or(DocumentError::LayerIdNotFound(id))
    }

    /// Creates a layer with AutoCAD's defaults (white, Continuous, on,
    /// plottable); returns its id.
    pub fn create_layer(&mut self, name: &str) -> DocumentResult<LayerId> {
        validate_name(name)?;
        if self.cad().layers.contains(name) {
            return Err(DocumentError::LayerExists(name.to_owned()));
        }
        self.touch_structure();
        let mut layer = CodecLayer::new(name);
        layer.handle = self.cad_mut().allocate_handle();
        let id = layer.handle.value();
        self.cad_mut()
            .layers
            .add(layer)
            .map_err(DocumentError::Codec)?;
        self.sync_layers();
        self.note_layer(id, LayerNote::Created);
        Ok(id)
    }

    /// Renames a layer; every entity on it follows. Changing only the case
    /// of the name is allowed.
    pub fn rename_layer(&mut self, id: LayerId, new_name: &str) -> DocumentResult<()> {
        let old = self.layer_name(id)?;
        validate_name(new_name)?;
        if old == new_name {
            return Ok(());
        }
        if is_protected(&old) {
            return Err(DocumentError::ProtectedLayer(old));
        }
        let same_key = normalize_name(&old) == normalize_name(new_name);
        if !same_key && self.cad().layers.contains(new_name) {
            return Err(DocumentError::LayerExists(new_name.to_owned()));
        }
        self.touch_structure();
        self.record_all_entities();
        self.cad_mut()
            .rename_layer(&old, new_name)
            .map_err(DocumentError::Codec)?;
        // Entities keep their layer's id; the model names it from the table.
        self.sync_layers();
        self.note_layer(id, LayerNote::Modified);
        Ok(())
    }

    /// Sets a layer's color (an ACI index or RGB).
    pub fn set_layer_color(&mut self, id: LayerId, color: Color) -> DocumentResult<()> {
        if !matches!(color, Color::Index(1..) | Color::Rgb { .. }) {
            return Err(DocumentError::InvalidLayerColor);
        }
        self.edit_layer(id, |l| l.color = color_to_codec(color))
    }

    /// Turns a layer on or off.
    pub fn set_layer_visible(&mut self, id: LayerId, visible: bool) -> DocumentResult<()> {
        self.edit_layer(id, |l| l.flags.off = !visible)
    }

    /// Freezes or thaws a layer. The current layer cannot be frozen.
    pub fn set_layer_frozen(&mut self, id: LayerId, frozen: bool) -> DocumentResult<()> {
        if frozen && self.current_layer().id == id {
            return Err(DocumentError::ProtectedLayer(self.layer_name(id)?));
        }
        self.edit_layer(id, |l| l.flags.frozen = frozen)
    }

    /// Locks or unlocks a layer.
    pub fn set_layer_locked(&mut self, id: LayerId, locked: bool) -> DocumentResult<()> {
        self.edit_layer(id, |l| l.flags.locked = locked)
    }

    /// Whether a layer is plotted.
    pub fn set_layer_plottable(&mut self, id: LayerId, plottable: bool) -> DocumentResult<()> {
        self.edit_layer(id, |l| l.is_plottable = plottable)
    }

    /// Deletes an empty layer. "0", "Defpoints", external-reference layers and
    /// the current layer cannot be deleted; nor can a layer with entities.
    pub fn delete_layer(&mut self, id: LayerId) -> DocumentResult<()> {
        let name = self.layer_name(id)?;
        if is_protected(&name) || self.current_layer().id == id {
            return Err(DocumentError::ProtectedLayer(name));
        }
        let key = normalize_name(&name);
        let in_use = self
            .cad()
            .entities()
            .any(|e| normalize_name(&e.common().layer) == key);
        if in_use {
            return Err(DocumentError::LayerInUse(name));
        }
        self.touch_structure();
        self.cad_mut().layers.remove(&name);
        self.sync_layers();
        self.note_layer(id, LayerNote::Deleted);
        Ok(())
    }

    /// The layer new entities go to (AutoCAD's CLAYER).
    pub fn current_layer(&self) -> Layer {
        self.model
            .layers
            .get(&self.model.current)
            .cloned()
            .expect("the current layer is in the model")
    }

    /// Makes a layer current. A frozen layer cannot be current.
    pub fn set_current_layer(&mut self, id: LayerId) -> DocumentResult<()> {
        let layer = self.layer(id).ok_or(DocumentError::LayerIdNotFound(id))?;
        if layer.frozen {
            return Err(DocumentError::FrozenLayer(layer.name));
        }
        self.touch_structure();
        let header = &mut self.cad_mut().header;
        header.current_layer_name = layer.name;
        header.current_layer_handle = Handle::new(id);
        self.model.current = id;
        self.note_document();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use arq_geometry::{Line, Point2};

    use super::*;
    use crate::entity::PlanGeometry;

    fn line(doc: &mut Document, layer: LayerId) -> u64 {
        let l = Line::new(
            Point2::new(0.0, 0.0).unwrap(),
            Point2::new(1.0, 0.0).unwrap(),
        )
        .unwrap();
        doc.add_entity(PlanGeometry::new(l.into()), layer).unwrap()
    }

    #[test]
    fn new_drawing_has_layer_zero_with_a_real_id() {
        let doc = Document::new();
        let zero = doc.layer_by_name("0").expect("layer 0");
        assert_ne!(zero.id(), 0, "a real handle from the start");
        assert_eq!(doc.current_layer(), zero);
        assert!(zero.is_visible() && !zero.is_frozen() && !zero.is_locked() && zero.is_plottable());
    }

    #[test]
    fn create_with_defaults_and_unique_names_ignoring_case() {
        let mut doc = Document::new();
        let walls = doc.create_layer("A-MUROS").unwrap();
        let l = doc.layer(walls).unwrap();
        assert_eq!(
            (l.name(), l.color(), l.linetype()),
            ("A-MUROS", Color::Index(7), "Continuous")
        );
        assert_eq!(
            doc.layer_by_name("a-muros").map(|l| l.id()),
            Some(walls),
            "names ignore case"
        );
        assert_eq!(
            doc.create_layer("a-muros"),
            Err(DocumentError::LayerExists("a-muros".into()))
        );
        let names: Vec<_> = {
            doc.create_layer("B-VIGAS").unwrap();
            doc.layers().iter().map(|l| l.name().to_owned()).collect()
        };
        assert_eq!(names, ["0", "A-MUROS", "B-VIGAS"]);
    }

    #[test]
    fn invalid_names_are_rejected() {
        let mut doc = Document::new();
        for bad in [
            "",
            " MUROS",
            "MUROS ",
            "A<B",
            "A/B",
            "A|B",
            "A*B",
            "A=B",
            "tab\tname",
        ] {
            assert_eq!(
                doc.create_layer(bad),
                Err(DocumentError::InvalidName(bad.into())),
                "{bad:?}"
            );
        }
        assert!(doc.create_layer(&"x".repeat(256)).is_err());
        assert!(
            doc.create_layer("Planta Baja - Ejes (1-5)").is_ok(),
            "spaces, dashes, parentheses"
        );
        assert!(doc.create_layer("Cotas_ñandú").is_ok(), "non-ASCII letters");
    }

    #[test]
    fn rename_moves_entities_and_allows_case_only_changes() {
        let mut doc = Document::new();
        let walls = doc.create_layer("muros").unwrap();
        let e = line(&mut doc, walls);
        doc.rename_layer(walls, "A-MUROS").unwrap();
        assert_eq!(doc.entity(e).unwrap().layer(), "A-MUROS");
        assert_eq!(doc.layer(walls).unwrap().name(), "A-MUROS", "same id");
        doc.rename_layer(walls, "a-muros").unwrap();
        assert_eq!(doc.layer(walls).unwrap().name(), "a-muros");
        let other = doc.create_layer("OTRA").unwrap();
        assert_eq!(
            doc.rename_layer(other, "A-MUROS"),
            Err(DocumentError::LayerExists("A-MUROS".into()))
        );
        let zero = doc.layer_by_name("0").unwrap().id();
        assert_eq!(
            doc.rename_layer(zero, "CERO"),
            Err(DocumentError::ProtectedLayer("0".into()))
        );
    }

    #[test]
    fn properties() {
        let mut doc = Document::new();
        let id = doc.create_layer("EJES").unwrap();
        doc.set_layer_color(
            id,
            Color::Rgb {
                r: 200,
                g: 40,
                b: 10,
            },
        )
        .unwrap();
        doc.set_layer_visible(id, false).unwrap();
        doc.set_layer_locked(id, true).unwrap();
        doc.set_layer_plottable(id, false).unwrap();
        doc.set_layer_frozen(id, true).unwrap();
        let l = doc.layer(id).unwrap();
        assert_eq!(
            l.color(),
            Color::Rgb {
                r: 200,
                g: 40,
                b: 10
            }
        );
        assert!(!l.is_visible() && l.is_locked() && !l.is_plottable() && l.is_frozen());
        for bad in [Color::ByLayer, Color::ByBlock, Color::None, Color::Index(0)] {
            assert_eq!(
                doc.set_layer_color(id, bad),
                Err(DocumentError::InvalidLayerColor)
            );
        }
        assert_eq!(
            doc.set_layer_visible(999_999, true),
            Err(DocumentError::LayerIdNotFound(999_999))
        );
    }

    #[test]
    fn current_layer_rules() {
        let mut doc = Document::new();
        let id = doc.create_layer("TRABAJO").unwrap();
        doc.set_current_layer(id).unwrap();
        assert_eq!(doc.current_layer().id(), id);
        assert!(
            matches!(
                doc.set_layer_frozen(id, true),
                Err(DocumentError::ProtectedLayer(_))
            ),
            "current cannot freeze"
        );
        assert!(
            matches!(doc.delete_layer(id), Err(DocumentError::ProtectedLayer(_))),
            "current cannot be deleted"
        );
        let frozen = doc.create_layer("CONGELADA").unwrap();
        doc.set_layer_frozen(frozen, true).unwrap();
        assert_eq!(
            doc.set_current_layer(frozen),
            Err(DocumentError::FrozenLayer("CONGELADA".into()))
        );
    }

    #[test]
    fn delete_only_empty_unprotected_layers() {
        let mut doc = Document::new();
        let empty = doc.create_layer("VACIA").unwrap();
        let used = doc.create_layer("USADA").unwrap();
        line(&mut doc, used);
        assert_eq!(
            doc.delete_layer(used),
            Err(DocumentError::LayerInUse("USADA".into()))
        );
        doc.delete_layer(empty).unwrap();
        assert!(doc.layer(empty).is_none());
        let zero = doc.layer_by_name("0").unwrap().id();
        assert_eq!(
            doc.delete_layer(zero),
            Err(DocumentError::ProtectedLayer("0".into()))
        );
    }

    #[test]
    fn layers_survive_dwg_and_dxf_with_the_same_ids() {
        let dir = tempfile::tempdir().unwrap();
        let mut doc = Document::new();
        let a = doc.create_layer("A-MUROS").unwrap();
        doc.set_layer_color(a, Color::Index(3)).unwrap();
        doc.set_layer_locked(a, true).unwrap();
        let b = doc.create_layer("Ejes (1-5)").unwrap();
        doc.set_layer_visible(b, false).unwrap();
        doc.set_current_layer(a).unwrap();
        let e = line(&mut doc, a);
        for name in ["capas.dwg", "capas.dxf"] {
            let path = dir.path().join(name);
            doc.save(&path).unwrap();
            let back = Document::open(&path).unwrap();
            assert_eq!(back.layer(a), doc.layer(a), "{name}");
            assert_eq!(back.layer(b), doc.layer(b), "{name}");
            assert_eq!(back.current_layer().id(), a, "{name}");
            assert_eq!(back.entity(e).unwrap().layer(), "A-MUROS", "{name}");
        }
    }
}
