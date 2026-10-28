//! Copying a block definition from another drawing (ADR 0025 D6): the
//! Blocks palette inserts a template's block by first copying it here, with
//! the blocks it nests and the layers, linetypes and text styles they use
//! that this drawing lacks. A layer or style this drawing has is used as it
//! is, as AutoCAD does. A block whose name this drawing already uses for
//! other contents is never resolved silently: the caller chooses.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use acadrust::tables::TableEntry;
use acadrust::{EntityType, Handle};

use crate::block_editor::block_of;
use crate::blocks::check_block_name;
use crate::document::Document;
use crate::error::{DocumentError, DocumentResult};
use crate::layers::LayerId;

/// What to do with a block of the other drawing whose name this drawing
/// already uses for different contents (ADR 0025 D6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClashChoice {
    /// Keep this drawing's definition; the other's is not copied.
    UseDrawing,
    /// Replace this drawing's definition by the other's: every reference
    /// to it changes.
    Redefine,
    /// Copy the other's under this new name.
    Rename(String),
}

/// A block of the other drawing to copy here: as a new block `to`, or
/// over this drawing's block `to` (redefining it).
struct Step {
    from: String,
    to: String,
    redefine: bool,
}

/// The entity kinds a copy carries whole: their links to other objects are
/// only names (layer, linetype, text style, block) and handles within the
/// copied definitions. Anything else is refused rather than copied half.
fn copyable(e: &EntityType) -> bool {
    matches!(
        e,
        EntityType::Point(_)
            | EntityType::Line(_)
            | EntityType::Circle(_)
            | EntityType::Arc(_)
            | EntityType::Ellipse(_)
            | EntityType::Polyline(_)
            | EntityType::Polyline2D(_)
            | EntityType::Polyline3D(_)
            | EntityType::LwPolyline(_)
            | EntityType::Text(_)
            | EntityType::MText(_)
            | EntityType::Spline(_)
            | EntityType::Hatch(_)
            | EntityType::Solid(_)
            | EntityType::Face3D(_)
            | EntityType::Insert(_)
            | EntityType::Ray(_)
            | EntityType::XLine(_)
            | EntityType::AttributeDefinition(_)
    )
}

impl Document {
    /// The blocks copying block `name` from `source` would bring whose
    /// names this drawing already uses for other contents, `name` first if
    /// it is one: each needs a [`ClashChoice`]. A block with the same
    /// contents here is simply used, so the blocks it nests are not asked
    /// about.
    pub fn block_clashes(&self, source: &Document, name: &str) -> DocumentResult<Vec<String>> {
        let top = source.insertable(name)?;
        let mut clashes = Vec::new();
        let mut seen = BTreeSet::new();
        let mut pending = vec![top];
        while let Some(block) = pending.pop() {
            if !seen.insert(block.to_uppercase()) {
                continue;
            }
            if self.cad().block_records.get(&block).is_some() {
                if self.same_block(source, &block) {
                    continue;
                }
                clashes.push(block.clone());
            }
            pending.extend(source.nested_blocks(&block)?.into_iter().rev());
        }
        Ok(clashes)
    }

    /// Copies block `name` of `source` into this drawing, with the blocks
    /// it nests and the layers, linetypes and text styles they use that
    /// this drawing lacks; returns the name it has here. A dynamic block
    /// keeps its parameters and actions (ADR 0017 D6).
    ///
    /// A block this drawing already has with the same contents is used as
    /// it is. Each one with other contents ([`block_clashes`](Self::block_clashes))
    /// is resolved by `choices`, by name ignoring case; one left unresolved
    /// is an error. Redefining or renaming a dynamic block waits for the
    /// parametric engine (milestone 04.1): only `UseDrawing` serves it.
    /// All or nothing.
    pub fn copy_block(
        &mut self,
        source: &Document,
        name: &str,
        choices: &BTreeMap<String, ClashChoice>,
    ) -> DocumentResult<String> {
        let top = source.insertable(name)?;
        let choice = |block: &str| {
            choices
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(block))
                .map(|(_, c)| c)
        };
        // Plan: which blocks to copy, under which names, before touching anything.
        let mut steps: Vec<Step> = Vec::new();
        let mut names: HashMap<String, String> = HashMap::new();
        let mut unresolved: Vec<String> = Vec::new();
        let mut seen = BTreeSet::new();
        let mut pending = vec![top.clone()];
        while let Some(block) = pending.pop() {
            if !seen.insert(block.to_uppercase()) {
                continue;
            }
            let here = self.cad().block_records.get(&block).map(|r| r.name.clone());
            let step = match here {
                None => Step {
                    from: block.clone(),
                    to: block.clone(),
                    redefine: false,
                },
                Some(here) if self.same_block(source, &block) => {
                    names.insert(block.to_uppercase(), here);
                    continue;
                }
                Some(here) => {
                    let dynamic = source.is_dynamic_block(&block) || self.is_dynamic_block(&here);
                    match choice(&block) {
                        None => {
                            unresolved.push(block.clone());
                            pending.extend(source.nested_blocks(&block)?.into_iter().rev());
                            continue;
                        }
                        Some(ClashChoice::UseDrawing) => {
                            names.insert(block.to_uppercase(), here);
                            continue;
                        }
                        Some(_) if dynamic => {
                            return Err(DocumentError::UnsupportedOperation(format!(
                                "{block} is a dynamic block: it can only be used as this \
                                 drawing has it until the parametric engine lands"
                            )));
                        }
                        Some(ClashChoice::Redefine) => Step {
                            from: block.clone(),
                            to: here,
                            redefine: true,
                        },
                        Some(ClashChoice::Rename(new)) => {
                            check_block_name(new)?;
                            let taken = self.cad().block_records.get(new).is_some()
                                || steps.iter().any(|s| s.to.eq_ignore_ascii_case(new));
                            if taken {
                                return Err(DocumentError::InvalidBlockName(format!(
                                    "a block named {new:?} exists"
                                )));
                            }
                            Step {
                                from: block.clone(),
                                to: new.clone(),
                                redefine: false,
                            }
                        }
                    }
                }
            };
            names.insert(block.to_uppercase(), step.to.clone());
            steps.push(step);
            pending.extend(source.nested_blocks(&block)?.into_iter().rev());
        }
        if !unresolved.is_empty() {
            return Err(DocumentError::InvalidBlockName(format!(
                "this drawing has other blocks named {}: choose what to do with each",
                unresolved.join(", ")
            )));
        }
        for step in &steps {
            source.check_copyable(&step.from)?;
        }
        let result = names
            .get(&top.to_uppercase())
            .cloned()
            .unwrap_or(top.clone());

        self.touch_structure();
        let entities: Vec<EntityType> = steps
            .iter()
            .flat_map(|s| source.cad().entities_in_block(&s.from).cloned())
            .collect();
        self.add_missing_symbols(source, &entities)?;
        // The records first, so the copies' references to each other resolve.
        let mut owners = Vec::new();
        for step in &steps {
            let record = source
                .cad()
                .block_records
                .get(&step.from)
                .expect("planned from the source's blocks");
            let owner = if step.redefine {
                self.clear_block(&step.to)?
            } else {
                self.add_block_record(&step.to, record.base_point)?
            };
            let (base, description, units) =
                (record.base_point, record.description.clone(), record.units);
            let (explodable, uniform) = (record.explodable, record.scale_uniformly);
            if let Some(here) = self.cad_mut().block_records.get_mut(&step.to) {
                here.base_point = base;
                here.description = description;
                here.units = units;
                here.explodable = explodable;
                here.scale_uniformly = uniform;
            }
            // Its data, and the currency of its cost (ADR 0027 D6); a block
            // redefined takes the source's, none included.
            let data = source.block_data(&step.from).ok().flatten();
            if let Some(cost) = data.as_ref().and_then(|d| d.cost.as_ref()) {
                self.add_currency(source.currency_of(&cost.currency, cost.decimals));
            }
            if data.is_some() || step.redefine {
                self.write_block_data(&step.to, &data.unwrap_or_default());
            }
            owners.push(owner);
        }
        // The contents, remembering where each source object went.
        let mut moved: HashMap<Handle, Handle> = HashMap::new();
        let mut copies: Vec<Handle> = Vec::new();
        for (step, &owner) in steps.iter().zip(&owners) {
            let contents: Vec<EntityType> = source
                .cad()
                .entities_in_block(&step.from)
                .cloned()
                .collect();
            for mut entity in contents {
                let from = entity.common().handle;
                let attributes: Vec<Handle> = match &entity {
                    EntityType::Insert(i) => i.attributes.iter().map(|a| a.common.handle).collect(),
                    _ => Vec::new(),
                };
                detach(&mut entity, &names);
                let id = self.add_copy(entity, owner)?;
                let to = Handle::new(id);
                moved.insert(from, to);
                if let Some(EntityType::Insert(i)) = self.cad().get_entity(to) {
                    for (a, b) in attributes.iter().zip(&i.attributes) {
                        moved.insert(*a, b.common.handle);
                    }
                }
                copies.push(to);
            }
        }
        // Links between copied objects follow them; links outside the copy
        // pointed at nothing that came along, and are dropped.
        let follow = |h: &Handle| moved.get(h).copied();
        for &h in &copies {
            match self.cad_mut().get_entity_mut(h) {
                Some(EntityType::Hatch(hatch)) => {
                    for path in &mut hatch.paths {
                        path.boundary_handles =
                            path.boundary_handles.iter().filter_map(follow).collect();
                    }
                }
                Some(EntityType::Insert(insert)) => {
                    for a in &mut insert.attributes {
                        a.attdef_handle = follow(&a.attdef_handle).unwrap_or(Handle::NULL);
                    }
                }
                _ => {}
            }
        }
        // A dynamic block's parameters, grips and actions.
        for (step, &owner) in steps.iter().zip(&owners) {
            let record = source
                .cad()
                .block_records
                .get(&step.from)
                .map(|r| r.handle());
            if let Some(graph) = record.and_then(|r| source.cad().export_block_graph(r)) {
                self.cad_mut().import_block_graph(&graph, owner, &moved);
            }
        }
        for step in steps.iter().filter(|s| s.redefine) {
            self.redraw_references(&step.to);
        }
        Ok(result)
    }

    /// Block `name` as this drawing spells it, if INSERT can place it.
    fn insertable(&self, name: &str) -> DocumentResult<String> {
        self.block_names()
            .into_iter()
            .find(|n| n.eq_ignore_ascii_case(name))
            .ok_or_else(|| DocumentError::InvalidBlockName(format!("no block named {name:?}")))
    }

    /// The blocks block `block` references directly, each once, in order.
    /// One that cannot be copied (anonymous, a layout, an external
    /// reference, missing) is refused.
    fn nested_blocks(&self, block: &str) -> DocumentResult<Vec<String>> {
        let mut found: Vec<String> = Vec::new();
        for name in self.cad().entities_in_block(block).filter_map(block_of) {
            let record = self.cad().block_records.get(&name);
            let plain = record.is_some_and(|r| {
                !r.is_anonymous()
                    && !r.is_layout()
                    && r.xref_path.is_empty()
                    && !r.is_model_space()
                    && !r.is_paper_space()
            });
            if !plain {
                return Err(DocumentError::UnsupportedOperation(format!(
                    "block {block} holds block {name}, which cannot be copied between drawings yet"
                )));
            }
            let name = record.map(|r| r.name.clone()).unwrap_or(name);
            if !found.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
                found.push(name);
            }
        }
        Ok(found)
    }

    /// Whether this drawing's block `block` has the same base point and
    /// contents as `source`'s, ignoring what only names objects of one
    /// drawing (handles, owners).
    fn same_block(&self, source: &Document, block: &str) -> bool {
        let (Some(a), Some(b)) = (
            self.cad().block_records.get(block),
            source.cad().block_records.get(block),
        ) else {
            return false;
        };
        let comparable = |d: &Document| -> Vec<EntityType> {
            d.cad()
                .entities_in_block(block)
                .map(|e| {
                    let mut e = e.clone();
                    detach(&mut e, &HashMap::new());
                    forget_links(&mut e);
                    e
                })
                .collect()
        };
        a.base_point == b.base_point && comparable(self) == comparable(source)
    }

    /// Refuses a block this drawing could not copy whole into another:
    /// contents of a kind a copy does not carry yet, links to materials,
    /// plot styles, visual styles or color books, or a dynamic block whose
    /// graph cannot be moved.
    fn check_copyable(&self, block: &str) -> DocumentResult<()> {
        let refuse = |what: String| {
            Err(DocumentError::UnsupportedOperation(format!(
                "block {block} holds {what}, which cannot be copied between drawings yet"
            )))
        };
        for e in self.cad().entities_in_block(block) {
            if !copyable(e) {
                return refuse(format!("a {}", e.as_entity().entity_type()));
            }
            let c = e.common();
            let linked = [
                c.material_handle,
                c.plotstyle_handle,
                c.color_book_handle,
                c.full_visual_style_handle,
                c.face_visual_style_handle,
                c.edge_visual_style_handle,
            ]
            .into_iter()
            .flatten()
            .any(|h| !h.is_null());
            if linked {
                return refuse(format!(
                    "a {} with a material, plot style, visual style or color book",
                    e.as_entity().entity_type()
                ));
            }
        }
        let record = self.cad().block_records.get(block).map(|r| r.handle());
        if self.is_dynamic_block(block)
            && record
                .and_then(|r| self.cad().export_block_graph(r))
                .is_none()
        {
            return refuse("dynamic-block data".to_owned());
        }
        Ok(())
    }

    /// Adds the text styles, linetypes, layers and application names
    /// `entities` (of `source`) use that this drawing lacks, as `source`
    /// has them. The caller has touched the structure.
    fn add_missing_symbols(
        &mut self,
        source: &Document,
        entities: &[EntityType],
    ) -> DocumentResult<()> {
        let codec = |e: acadrust::DxfError| DocumentError::Codec(e.to_string());
        let mut layers = BTreeSet::new();
        let mut linetypes = BTreeSet::new();
        let mut styles = BTreeSet::new();
        let mut apps = BTreeSet::new();
        for e in entities {
            let c = e.common();
            layers.insert(c.layer.clone());
            linetypes.insert(c.linetype.clone());
            apps.extend(
                c.extended_data
                    .records()
                    .iter()
                    .map(|r| r.application_name.clone()),
            );
            match e {
                EntityType::Text(t) => drop(styles.insert(t.style.clone())),
                EntityType::MText(t) => drop(styles.insert(t.style.clone())),
                EntityType::AttributeDefinition(d) => drop(styles.insert(d.text_style.clone())),
                _ => {}
            }
            if let EntityType::Insert(i) = e {
                for a in &i.attributes {
                    layers.insert(a.common.layer.clone());
                    linetypes.insert(a.common.linetype.clone());
                    styles.insert(a.text_style.clone());
                }
            }
        }
        let mut layers: Vec<_> = layers
            .iter()
            .filter(|n| self.cad().layers.get(n).is_none())
            .filter_map(|n| source.cad().layers.get(n).cloned())
            .collect();
        // A layer to add names its material and plot style by name, as
        // this drawing has them; a standard one it does not store is its
        // implicit default (a new drawing stores no Global material).
        for layer in &mut layers {
            source.cad().normalize_imported_layer_defaults(layer);
            let refuse = |what: &str| {
                Err(DocumentError::UnsupportedOperation(format!(
                    "layer {} {what}, which cannot be copied between drawings yet",
                    layer.name
                )))
            };
            if layer.name.contains('|') {
                return refuse("belongs to an external reference");
            }
            let (from, here) = (&source.cad().header, &self.cad().header);
            let material = relink(
                source,
                self,
                (
                    from.acad_material_dict_handle,
                    here.acad_material_dict_handle,
                ),
                layer.material,
                &["ByBlock", "ByLayer", "Global"],
            );
            let plot_style = relink(
                source,
                self,
                (
                    from.acad_plotstylename_dict_handle,
                    here.acad_plotstylename_dict_handle,
                ),
                layer.plotstyle_handle,
                &["Normal"],
            );
            match (material, plot_style) {
                (Some(material), Some(plot_style)) => {
                    layer.material = material;
                    layer.plotstyle_handle = plot_style;
                }
                (None, _) => return refuse("has a material this drawing lacks"),
                (_, None) => return refuse("has a plot style this drawing lacks"),
            }
        }
        linetypes.extend(layers.iter().map(|l| l.line_type.clone()));
        let linetypes: Vec<_> = linetypes
            .iter()
            .filter(|n| self.cad().line_types.get(n).is_none())
            .filter_map(|n| source.cad().line_types.get(n).cloned())
            .collect();
        // The text styles complex linetypes draw their text and shapes with.
        let style_of: HashMap<Handle, String> = source
            .cad()
            .text_styles
            .iter()
            .map(|s| (s.handle, s.name.clone()))
            .collect();
        for element in linetypes.iter().flat_map(|l| &l.elements) {
            if let Some(name) = element
                .complex
                .as_ref()
                .and_then(|c| style_of.get(&c.style_handle))
            {
                styles.insert(name.clone());
            }
        }
        for name in &styles {
            if name.is_empty() || self.cad().text_styles.get(name).is_some() {
                continue;
            }
            if let Some(style) = source.cad().text_styles.get(name) {
                let mut style = style.clone();
                style.handle = self.cad_mut().allocate_handle();
                self.cad_mut().text_styles.add(style).map_err(codec)?;
            }
        }
        for mut linetype in linetypes {
            linetype.handle = self.cad_mut().allocate_handle();
            for element in &mut linetype.elements {
                if let Some(complex) = element.complex.as_mut() {
                    let here = style_of
                        .get(&complex.style_handle)
                        .and_then(|n| self.cad().text_styles.get(n))
                        .map_or(Handle::NULL, |s| s.handle);
                    complex.style_handle = here;
                }
            }
            self.cad_mut().line_types.add(linetype).map_err(codec)?;
        }
        let mut created: Vec<LayerId> = Vec::new();
        for mut layer in layers {
            layer.handle = self.cad_mut().allocate_handle();
            created.push(layer.handle.value());
            self.cad_mut().layers.add(layer).map_err(codec)?;
        }
        for app in apps {
            if !app.is_empty() && !self.cad().app_ids.contains(&app) {
                let mut id = acadrust::tables::AppId::new(&app);
                id.set_handle(self.cad_mut().allocate_handle());
                self.cad_mut().app_ids.add(id).map_err(codec)?;
            }
        }
        if !created.is_empty() {
            self.sync_layers();
            for id in created {
                self.note_layer(id, crate::change::LayerNote::Created);
            }
        }
        Ok(())
    }
}

/// The handle in `here` of the entry `handle` names in `source`'s
/// dictionary, found by name in `here`'s (`dictionaries`: source's, here's).
/// Null stays null, and so does one of the `standard` names `here` does not
/// store; `None` when `here` has no entry of another name.
fn relink(
    source: &Document,
    here: &Document,
    dictionaries: (Handle, Handle),
    handle: Handle,
    standard: &[&str],
) -> Option<Handle> {
    use acadrust::objects::ObjectType;
    if handle.is_null() {
        return Some(Handle::NULL);
    }
    let entries = |d: &Document, dictionary: Handle| match d.cad().objects.get(&dictionary) {
        Some(ObjectType::Dictionary(d)) => d.entries.clone(),
        Some(ObjectType::DictionaryWithDefault(d)) => d.entries.clone(),
        _ => Vec::new(),
    };
    let name = entries(source, dictionaries.0)
        .into_iter()
        .find(|(_, h)| *h == handle)?
        .0;
    let found = entries(here, dictionaries.1)
        .into_iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(&name))
        .map(|(_, h)| h);
    let implicit = standard.iter().any(|s| s.eq_ignore_ascii_case(&name));
    found.or(implicit.then_some(Handle::NULL))
}

/// Makes `e`, of another drawing, ready to be added here: it keeps no
/// handle naming an object of that drawing, except the links between
/// copied objects (a hatch's boundary, an attribute's definition) that
/// [`Document::copy_block`] follows afterwards; references to blocks take
/// their names here (`names`, by upper-case name).
fn detach(e: &mut EntityType, names: &HashMap<String, String>) {
    let c = e.common_mut();
    c.handle = Handle::NULL;
    c.owner_handle = Handle::NULL;
    c.layer_handle = None;
    c.linetype_handle = None;
    match e {
        EntityType::Insert(i) => {
            if let Some(name) = names.get(&i.block_name.to_uppercase()) {
                i.block_name = name.clone();
            }
            i.view_rep_handle = None;
            i.seqend_handle = None;
            for a in &mut i.attributes {
                a.common.layer_handle = None;
                a.common.linetype_handle = None;
            }
        }
        EntityType::Polyline3D(p) => {
            for v in &mut p.vertices {
                v.handle = Handle::NULL;
            }
        }
        _ => {}
    }
}

/// Clears what [`detach`] keeps for the copy to follow, and what only one
/// drawing's file holds, so two drawings' blocks compare by contents.
fn forget_links(e: &mut EntityType) {
    let c = e.common_mut();
    c.raw_record = None;
    c.reactors.clear();
    c.xdictionary_handle = None;
    c.graphic_data = None;
    match e {
        EntityType::Hatch(h) => h.paths.iter_mut().for_each(|p| p.boundary_handles.clear()),
        EntityType::Insert(i) => {
            i.block_name = i.block_name.to_uppercase();
            for a in &mut i.attributes {
                a.common.handle = Handle::NULL;
                a.common.owner_handle = Handle::NULL;
                a.common.raw_record = None;
                a.common.reactors.clear();
                a.common.xdictionary_handle = None;
                a.attdef_handle = Handle::NULL;
            }
        }
        _ => {}
    }
}

#[cfg(test)]
#[path = "block_copy_tests.rs"]
mod tests;
