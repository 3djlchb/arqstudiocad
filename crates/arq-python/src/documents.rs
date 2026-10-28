//! `CadApplication` and the `CadDocument` handle (ADR 0012 D4.3, D4.6).
//!
//! A `CadDocument` is the shared store plus a document id: it holds no
//! reference into the drawing, and every call locks the document for that
//! call only. Opening and saving release the GIL while the file is read or
//! written.
//!
//! Rule for every call: never run Python code while a document is locked.
//! A thread that released the GIL may hold that lock, and it must be able to
//! finish without needing the GIL back. Even creating Python objects counts:
//! it may run the garbage collector, whose finalizers may call back into the
//! same document. So the closures given to `read`/`edit` copy Rust values
//! out, and Python objects are built after the lock is released.

use std::path::PathBuf;
use std::sync::Arc;

use arq_core::{
    BlockDefinition, BlockObjects, CadApplication as CoreApplication, CadChangeSet as ChangeSet,
    CadDocument as CoreDocument, CadError, ClashChoice, DocumentId, EntityId, LayerId,
};
use arq_geometry::{Geometry2, Line};
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;

use crate::changes::{CadChangeSet, CadGroup};
use crate::errors::IntoPy;
use crate::info::{CadBlockInfo, CadEntityInfo, CadLayerInfo};
use crate::values::{
    CadArc, CadBounds2, CadCircle, CadColor, CadGrip, CadLine, CadSnap, PyPoint, PyVector,
    snap_kinds,
};

/// The open documents of one application.
#[pyclass(frozen, module = "arqstudiocad._core")]
pub struct CadApplication {
    store: Arc<CoreApplication>,
}

#[pymethods]
impl CadApplication {
    #[new]
    fn new() -> Self {
        Self {
            store: Arc::new(CoreApplication::new()),
        }
    }

    /// Creates an empty drawing called `name` (its title until saved).
    fn create_document(&self, name: &str) -> CadDocument {
        let id = self.store.create_document(name);
        self.handle(id)
    }

    /// Opens a DWG or DXF file; the extension decides the format. Other
    /// Python threads keep running while the file is read.
    fn open_document(&self, py: Python<'_>, path: PathBuf) -> PyResult<CadDocument> {
        let store = Arc::clone(&self.store);
        let id = py.detach(move || store.open_document(path)).py()?;
        Ok(self.handle(id))
    }

    /// Saves a document as DWG or DXF, by the file extension. Other Python
    /// threads keep running while the file is written.
    fn save_document(
        &self,
        py: Python<'_>,
        document_id: DocumentId,
        path: PathBuf,
    ) -> PyResult<()> {
        save(py, &self.store, document_id, path)
    }

    /// Closes a document, discarding unsaved changes; check
    /// `CadDocument.is_modified` first.
    fn close_document(&self, document_id: DocumentId) -> PyResult<()> {
        self.store.close_document(document_id).py()
    }

    /// The open document with this id.
    fn document(&self, document_id: DocumentId) -> PyResult<CadDocument> {
        // Asking for the name checks that it is open.
        self.store.document_name(document_id).py()?;
        Ok(self.handle(document_id))
    }

    /// Ids of the open documents, in opening order.
    fn document_ids(&self) -> Vec<DocumentId> {
        self.store.document_ids()
    }

    /// The open documents, in opening order.
    fn documents(&self) -> Vec<CadDocument> {
        self.store
            .document_ids()
            .into_iter()
            .map(|id| self.handle(id))
            .collect()
    }

    fn __repr__(&self) -> String {
        format!("CadApplication(documents={:?})", self.store.document_ids())
    }
}

impl CadApplication {
    fn handle(&self, id: DocumentId) -> CadDocument {
        CadDocument {
            store: Arc::clone(&self.store),
            id,
        }
    }
}

fn save(
    py: Python<'_>,
    store: &Arc<CoreApplication>,
    id: DocumentId,
    path: PathBuf,
) -> PyResult<()> {
    let store = Arc::clone(store);
    py.detach(move || store.save_document(id, path)).py()
}

/// One open drawing, by id. Calls on a closed document raise
/// `DocumentNotFoundError`.
#[pyclass(frozen, module = "arqstudiocad._core")]
pub struct CadDocument {
    store: Arc<CoreApplication>,
    id: DocumentId,
}

impl CadDocument {
    /// Runs `f` with read access, for this call only.
    pub fn read<R>(&self, f: impl FnOnce(&CoreDocument) -> R) -> PyResult<R> {
        self.store.read(self.id, f).py()
    }

    /// Like [`read`](Self::read) but touching no Python at all, so it can
    /// run with the GIL released (inside `py.detach`).
    pub fn read_core<R>(&self, f: impl FnOnce(&CoreDocument) -> R) -> arq_core::CadResult<R> {
        self.store.read(self.id, f)
    }

    /// Runs `f` with write access, for this call only.
    pub fn edit<R>(
        &self,
        f: impl FnOnce(&mut CoreDocument) -> arq_core::CadResult<R>,
    ) -> PyResult<R> {
        self.store.edit(self.id, f).py()
    }

    /// One operation that returns only a change set.
    fn change(
        &self,
        f: impl FnOnce(&mut CoreDocument) -> arq_core::CadResult<ChangeSet>,
    ) -> PyResult<CadChangeSet> {
        self.edit(f).map(CadChangeSet)
    }

    /// One operation that creates something and returns its id too.
    fn create(
        &self,
        f: impl FnOnce(&mut CoreDocument) -> arq_core::CadResult<(u64, ChangeSet)>,
    ) -> PyResult<(u64, CadChangeSet)> {
        self.edit(f)
            .map(|(id, changes)| (id, CadChangeSet(changes)))
    }

    /// Group plumbing for `CadGroup`.
    pub fn begin(&self, name: &str) -> PyResult<()> {
        self.edit(|cad| {
            cad.begin_group(name);
            Ok(())
        })
    }

    pub fn end(&self) -> PyResult<ChangeSet> {
        self.edit(CoreDocument::end_group)
    }

    pub fn cancel(&self) -> PyResult<ChangeSet> {
        self.edit(CoreDocument::cancel_group)
    }

    pub fn group_open(&self) -> PyResult<bool> {
        self.read(CoreDocument::in_group)
    }
}

#[pymethods]
impl CadDocument {
    /// Session id of the document; never reused while the application runs.
    #[getter]
    fn id(&self) -> DocumentId {
        self.id
    }

    /// Title: the file name, or the name it was created with.
    #[getter]
    fn name(&self) -> PyResult<String> {
        self.store.document_name(self.id).py()
    }

    /// The file it was opened from or last saved to, if any.
    #[getter]
    fn path(&self) -> PyResult<Option<PathBuf>> {
        self.store.document_path(self.id).py()
    }

    /// Whether it differs from the last saved or opened state.
    #[getter]
    fn is_modified(&self) -> PyResult<bool> {
        self.read(CoreDocument::is_modified)
    }

    /// Whether it is still open.
    #[getter]
    fn is_open(&self) -> bool {
        self.store.document_ids().contains(&self.id)
    }

    /// File-format version code of the drawing, e.g. `"AC1032"` (2018).
    #[getter]
    fn version(&self) -> PyResult<String> {
        self.read(|cad| cad.document().version_code())
    }

    /// What was wrong with the damaged file this drawing was recovered
    /// from, in English as the reader reports it; empty for a drawing read
    /// intact or created here.
    #[getter]
    fn recovery_problems(&self) -> PyResult<Vec<String>> {
        self.read(|cad| {
            cad.document()
                .recovery()
                .map(|r| r.problems().to_vec())
                .unwrap_or_default()
        })
    }

    /// Objects of the file the engine could not decode and keeps verbatim,
    /// as `"ACDBASSOC2DCONSTRAINTGROUP (3E3)"`: a save in the drawing's own
    /// version writes them back unchanged.
    #[getter]
    fn undecoded_objects(&self) -> PyResult<Vec<String>> {
        self.read(|cad| cad.document().undecoded_objects().to_vec())
    }

    /// The user accepts the recovered drawing: saving may now replace the
    /// damaged file it came from (otherwise `OverwritesDamagedError`).
    fn allow_overwriting_damaged(&self) -> PyResult<()> {
        self.store.allow_overwriting_damaged(self.id).py()
    }

    /// The user agrees that saving this document loses these kinds of data
    /// (`LossyConversionError.loss_kinds`); later saves stop only for other
    /// losses, until the document is closed.
    fn accept_losses(&self, kinds: Vec<String>) -> PyResult<()> {
        self.store.accept_losses(self.id, &kinds).py()
    }

    /// The kinds of data the user agreed to lose when saving it, sorted.
    #[getter]
    fn accepted_losses(&self) -> PyResult<Vec<String>> {
        self.store.accepted_losses(self.id).py()
    }

    /// How many entities model space holds, without reading them.
    #[getter]
    fn entity_count(&self) -> PyResult<usize> {
        self.read(|cad| cad.document().model_entity_count())
    }

    // Queries (contract §4). Each copies what it needs under the lock and
    // builds the Python objects after.

    /// The entity with this id, in any space.
    fn get_entity(&self, entity_id: EntityId) -> PyResult<CadEntityInfo> {
        self.read(|cad| cad.document().entity(entity_id))?
            .map(CadEntityInfo)
            .ok_or(CadError::EntityNotFound(entity_id))
            .py()
    }

    /// The entities with these ids, in that order; without ids, every entity
    /// in model space, in drawing order. A missing id raises
    /// `EntityNotFoundError`.
    #[pyo3(signature = (entity_ids = None))]
    fn get_entities(&self, entity_ids: Option<Vec<EntityId>>) -> PyResult<Vec<CadEntityInfo>> {
        let found = self.read(|cad| {
            let doc = cad.document();
            match entity_ids {
                None => Ok(doc.entities()),
                Some(ids) => ids
                    .into_iter()
                    .map(|id| doc.entity(id).ok_or(CadError::EntityNotFound(id)))
                    .collect(),
            }
        })?;
        Ok(found.py()?.into_iter().map(CadEntityInfo).collect())
    }

    /// The box around every entity in model space; `None` when empty.
    fn get_bounds(&self) -> PyResult<Option<CadBounds2>> {
        Ok(self.read(CoreDocument::bounds)?.map(CadBounds2))
    }

    /// Ids of the model-space entities whose bounding box touches `area`, in
    /// drawing order: candidates, whatever their layer state.
    fn query_entities(&self, area: CadBounds2) -> PyResult<Vec<EntityId>> {
        self.read(|cad| cad.query_entities(area.0))
    }

    /// The entity whose drawn geometry passes closest to `point` within
    /// `aperture` (world units), the one drawn last on a tie; `None` if
    /// none. Blocks count as a whole; off and frozen layers are skipped,
    /// locked ones are not (ADR 0016 D2).
    fn pick(&self, point: PyPoint, aperture: f64) -> PyResult<Option<EntityId>> {
        self.read(|cad| cad.pick(point.0, aperture))?.py()
    }

    /// Entities wholly inside `area` or, with `crossing`, those whose drawn
    /// geometry touches it, exactly; in drawing order (ADR 0016 D2).
    #[pyo3(signature = (area, crossing = false))]
    fn select_window(&self, area: CadBounds2, crossing: bool) -> PyResult<Vec<EntityId>> {
        self.read(|cad| cad.select_window(area.0, crossing))
    }

    /// The object snap under `cursor` within `aperture` (world units) among
    /// the active `modes` (AutoCAD keys: END, MID, CEN, QUA, INT, PER, TAN,
    /// NEA); `last_point` is needed for PER and TAN (ADR 0016 D5).
    #[pyo3(signature = (cursor, aperture, modes, last_point = None))]
    fn snap(
        &self,
        cursor: PyPoint,
        aperture: f64,
        modes: Vec<String>,
        last_point: Option<PyPoint>,
    ) -> PyResult<Option<CadSnap>> {
        let modes = snap_kinds(&modes)?;
        let from = last_point.map(|p| p.0);
        let found = self
            .read(|cad| cad.snap(cursor.0, aperture, &modes, from))?
            .py()?;
        Ok(found.map(CadSnap))
    }

    /// The grips of `entity_ids` (ADR 0016 D7), entity by entity in the
    /// order given; kinds without grips yet (texts…) have none.
    fn grips(&self, entity_ids: Vec<EntityId>) -> PyResult<Vec<CadGrip>> {
        let grips = self.read(|cad| cad.grips(&entity_ids))?;
        Ok(grips.into_iter().map(CadGrip).collect())
    }

    /// Every layer: "0" first, then by name ignoring case.
    fn get_layers(&self) -> PyResult<Vec<CadLayerInfo>> {
        let layers = self.read(|cad| cad.document().layers())?;
        Ok(layers.into_iter().map(CadLayerInfo).collect())
    }

    /// The layer with this id.
    fn get_layer(&self, layer_id: LayerId) -> PyResult<CadLayerInfo> {
        self.read(|cad| cad.document().layer(layer_id))?
            .map(CadLayerInfo)
            .ok_or(CadError::LayerNotFound(layer_id))
            .py()
    }

    /// The layer with this name, ignoring case; `None` if there is none.
    fn get_layer_by_name(&self, name: &str) -> PyResult<Option<CadLayerInfo>> {
        Ok(self
            .read(|cad| cad.document().layer_by_name(name))?
            .map(CadLayerInfo))
    }

    /// The drawing's default text height (TEXTSIZE).
    #[getter]
    fn text_size(&self) -> PyResult<f64> {
        self.read(|cad| cad.document().text_size())
    }

    /// The layer new entities go to.
    #[getter]
    fn current_layer(&self) -> PyResult<CadLayerInfo> {
        Ok(CadLayerInfo(
            self.read(|cad| cad.document().current_layer())?,
        ))
    }

    // Operations (contract §4 and §6). Each is one transaction: one undo
    // step, or no change at all when it raises. Ids may be any sequence of
    // ints; repeats are edited once; an empty list changes nothing.

    /// Draws a line on the current layer; returns its id and what changed.
    fn add_line(&self, start: PyPoint, end: PyPoint) -> PyResult<(EntityId, CadChangeSet)> {
        self.create(|cad| cad.add_line(start.0, end.0))
    }

    /// Writes a single-line text, left-justified on the baseline at
    /// `anchor`, on the current layer with the current text style (TEXT,
    /// ADR 0022); `rotation` in radians.
    fn add_text(
        &self,
        content: &str,
        anchor: PyPoint,
        height: f64,
        rotation: f64,
    ) -> PyResult<(EntityId, CadChangeSet)> {
        self.create(|cad| cad.add_text(content, anchor.0, height, rotation))
    }

    /// Draws a polyline on the current layer (PLINE): `vertices` as
    /// `(point, bulge)` pairs, the bulge of the segment leaving each vertex
    /// (0 straight, positive counterclockwise); `closed` joins the last
    /// vertex to the first.
    #[pyo3(signature = (vertices, closed = false))]
    fn add_polyline(
        &self,
        vertices: Vec<(PyPoint, f64)>,
        closed: bool,
    ) -> PyResult<(EntityId, CadChangeSet)> {
        let vertices: Vec<_> = vertices.into_iter().map(|(p, b)| (p.0, b)).collect();
        self.create(|cad| cad.add_polyline(&vertices, closed))
    }

    /// Makes block `name` from `entity_ids` with its base point at `base`,
    /// and puts a reference to it in their place (BLOCK, ADR 0023); returns
    /// the reference's id.
    fn create_block(
        &self,
        name: &str,
        base: PyPoint,
        entity_ids: Vec<EntityId>,
    ) -> PyResult<(EntityId, CadChangeSet)> {
        self.create(|cad| cad.create_block(name, base.0, &entity_ids))
    }

    /// Makes block `name` from `entity_ids` as the Block Definition dialog
    /// says (BLOCK, ADR 0023 D2a): `objects` is "retain", "convert" or
    /// "delete"; the description, units (INSUNITS code) and flags go to the
    /// block's record; with `redefine` an existing block's contents are
    /// replaced. Returns the reference placed by "convert", else None.
    #[pyo3(signature = (
        name, base, entity_ids, objects = "convert", description = "", units = 0,
        scale_uniformly = false, explodable = true, redefine = false
    ))]
    #[allow(clippy::too_many_arguments)]
    fn define_block(
        &self,
        name: &str,
        base: PyPoint,
        entity_ids: Vec<EntityId>,
        objects: &str,
        description: &str,
        units: i16,
        scale_uniformly: bool,
        explodable: bool,
        redefine: bool,
    ) -> PyResult<(Option<EntityId>, CadChangeSet)> {
        let objects = match objects {
            "retain" => BlockObjects::Retain,
            "convert" => BlockObjects::Convert,
            "delete" => BlockObjects::Delete,
            other => {
                return Err(PyValueError::new_err(format!(
                    "objects must be \"retain\", \"convert\" or \"delete\", not {other:?}"
                )));
            }
        };
        let definition = BlockDefinition {
            description: description.to_owned(),
            units,
            scale_uniformly,
            explodable,
        };
        self.edit(|cad| cad.define_block(name, base.0, &entity_ids, objects, &definition, redefine))
            .map(|(id, changes)| (id, CadChangeSet(changes)))
    }

    /// Renames block `name` to `new_name`, every reference following
    /// (RENAME): one undo step.
    fn rename_block(&self, name: &str, new_name: &str) -> PyResult<CadChangeSet> {
        self.change(|cad| cad.rename_block(name, new_name))
    }

    /// Places block `name` at `point`, scaled and turned (radians), on the
    /// current layer; its attributes take their default values (INSERT).
    #[pyo3(signature = (name, point, scale = 1.0, rotation = 0.0))]
    fn insert_block(
        &self,
        name: &str,
        point: PyPoint,
        scale: f64,
        rotation: f64,
    ) -> PyResult<(EntityId, CadChangeSet)> {
        self.create(|cad| cad.insert_block(name, point.0, scale, rotation))
    }

    /// The drawing's insertion units, as DWG's INSUNITS code: a new
    /// block's units by default (ADR 0023 D2a).
    #[getter]
    fn insertion_units(&self) -> PyResult<i16> {
        self.read(|cad| cad.document().insertion_units())
    }

    /// The blocks INSERT can place, sorted: not anonymous, layouts or
    /// external references.
    #[getter]
    fn block_names(&self) -> PyResult<Vec<String>> {
        self.read(|cad| cad.document().block_names())
    }

    /// What the Blocks palette shows of block `name` (found ignoring
    /// case), or None unless INSERT can place it (ADR 0025 D1).
    fn block_info(&self, name: &str) -> PyResult<Option<CadBlockInfo>> {
        self.read(|cad| cad.document().block_info(name).map(CadBlockInfo))
    }

    /// How many references each block has anywhere in the drawing, a
    /// dynamic block's anonymous copies included; blocks without any are
    /// left out.
    #[getter]
    fn block_references(&self) -> PyResult<std::collections::HashMap<String, usize>> {
        self.read(|cad| cad.document().block_references())
    }

    /// Where block `name`'s references are (SELECTBLOCK, ADR 0025 step
    /// 8d): the ids of those in model space that can be selected, in the
    /// order drawn, then how many are on off or frozen layers, inside other
    /// blocks, and in layouts.
    fn find_block_references(&self, name: &str) -> PyResult<(Vec<EntityId>, usize, usize, usize)> {
        let found = self
            .read(|cad| {
                cad.document()
                    .find_block_references(name)
                    .map_err(arq_core::CadError::from)
            })?
            .py()?;
        Ok((found.shown, found.hidden, found.nested, found.in_layouts))
    }

    /// The blocks INSERT can place that nothing references, dynamic ones
    /// included (PURGE, ADR 0025 step 8e).
    #[getter]
    fn unused_blocks(&self) -> PyResult<Vec<String>> {
        self.read(|cad| cad.document().unused_blocks())
    }

    /// Purges unused blocks `names`, and with `nested` the blocks they held
    /// that nothing else uses: one undo step. Returns the blocks purged.
    #[pyo3(signature = (names, nested = true))]
    fn purge_blocks(
        &self,
        names: Vec<String>,
        nested: bool,
    ) -> PyResult<(Vec<String>, CadChangeSet)> {
        self.edit(|cad| cad.purge_blocks(&names, nested))
            .map(|(purged, changes)| (purged, CadChangeSet(changes)))
    }

    /// Block `name`'s preview for the Blocks palette (ADR 0025 D3): a
    /// `size` × `size` RGBA image (premultiplied, rows top to bottom), its
    /// drawing fitted in on transparency, in its own colors as seen over
    /// `background`, or all in `ink`. None unless INSERT can place it.
    #[pyo3(signature = (name, size, background, ink=None))]
    fn block_preview<'py>(
        &self,
        py: Python<'py>,
        name: &str,
        size: u32,
        background: (u8, u8, u8),
        ink: Option<(u8, u8, u8)>,
    ) -> PyResult<Option<Bound<'py, pyo3::types::PyBytes>>> {
        if !(1..=1024).contains(&size) {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "a preview is 1 to 1024 pixels wide",
            ));
        }
        let items = self.read(|cad| cad.document().block_drawing(name))?;
        Ok(items.map(|items| {
            let image = arq_render::preview(
                &items,
                size,
                [background.0, background.1, background.2],
                ink.map(|(r, g, b)| [r, g, b]),
            );
            pyo3::types::PyBytes::new(py, &image.pixels)
        }))
    }

    /// Whether block `name` is dynamic, so the block editor refuses it
    /// (ADR 0023 D3a).
    fn is_dynamic_block(&self, name: &str) -> PyResult<bool> {
        self.read(|cad| cad.document().is_dynamic_block(name))
    }

    /// Opens block `name` alone in a new document, the block editor, called
    /// `title` (BEDIT, ADR 0023 D3); it is never saved to a file.
    fn open_block_editor(&self, name: &str, title: &str) -> PyResult<CadDocument> {
        let id = self.store.open_block_editor(self.id, name, title).py()?;
        Ok(CadDocument {
            store: Arc::clone(&self.store),
            id,
        })
    }

    /// Makes what block editor `editor` holds the contents of block `name`:
    /// one undo step here; the editor is then unmodified.
    fn save_block(&self, name: &str, editor: &CadDocument) -> PyResult<CadChangeSet> {
        self.store
            .save_block(editor.id, self.id, name)
            .py()
            .map(CadChangeSet)
    }

    /// The blocks copying block `name` from `source` would bring whose
    /// names this drawing uses for other contents, `name` first if it is
    /// one: each needs a choice (ADR 0025 D6).
    fn block_clashes(&self, name: &str, source: &CadDocument) -> PyResult<Vec<String>> {
        self.store.block_clashes(source.id, self.id, name).py()
    }

    /// Copies block `name` from `source` with the blocks it nests and the
    /// layers, linetypes and text styles they use that this drawing lacks;
    /// returns its name here and the changes (ADR 0025 D6). Each clashing
    /// block is kept as this drawing has it (`keep`), redefined from
    /// `source` (`redefine`) or copied under a new name (`rename`, old to
    /// new); one left out is an error. One undo step, or part of a group.
    #[pyo3(signature = (name, source, keep = Vec::new(), redefine = Vec::new(), rename = std::collections::BTreeMap::new()))]
    fn copy_block(
        &self,
        name: &str,
        source: &CadDocument,
        keep: Vec<String>,
        redefine: Vec<String>,
        rename: std::collections::BTreeMap<String, String>,
    ) -> PyResult<(String, CadChangeSet)> {
        let mut choices = std::collections::BTreeMap::new();
        choices.extend(keep.into_iter().map(|n| (n, ClashChoice::UseDrawing)));
        choices.extend(redefine.into_iter().map(|n| (n, ClashChoice::Redefine)));
        choices.extend(
            rename
                .into_iter()
                .map(|(n, new)| (n, ClashChoice::Rename(new))),
        );
        self.store
            .copy_block(source.id, self.id, name, &choices)
            .py()
            .map(|(name, changes)| (name, CadChangeSet(changes)))
    }

    /// Writes a multiline text on the current layer with the current text
    /// style (MTEXT, ADR 0022 D6): `content` is plain text whose line breaks
    /// are paragraphs, wrapped to `width` (0: no wrapping), hanging from
    /// `anchor` as `justification` says; `rotation` in radians.
    #[pyo3(signature = (content, anchor, height, rotation, width, justification = "TopLeft"))]
    fn add_mtext(
        &self,
        content: &str,
        anchor: PyPoint,
        height: f64,
        rotation: f64,
        width: f64,
        justification: &str,
    ) -> PyResult<(EntityId, CadChangeSet)> {
        self.create(|cad| cad.add_mtext(content, anchor.0, height, rotation, width, justification))
    }

    /// Sets what is given of texts' content, height and rotation (radians),
    /// all or none (ADR 0022).
    #[pyo3(signature = (entity_ids, content = None, height = None, rotation = None))]
    fn set_text(
        &self,
        entity_ids: Vec<EntityId>,
        content: Option<String>,
        height: Option<f64>,
        rotation: Option<f64>,
    ) -> PyResult<CadChangeSet> {
        self.change(|cad| cad.set_text(&entity_ids, content.as_deref(), height, rotation))
    }

    /// Draws a circle on the current layer.
    fn add_circle(&self, center: PyPoint, radius: f64) -> PyResult<(EntityId, CadChangeSet)> {
        self.create(|cad| cad.add_circle(center.0, radius))
    }

    /// Draws an arc on the current layer, counterclockwise from
    /// `start_angle` to `end_angle` (radians).
    fn add_arc(
        &self,
        center: PyPoint,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
    ) -> PyResult<(EntityId, CadChangeSet)> {
        self.create(|cad| cad.add_arc(center.0, radius, start_angle, end_angle))
    }

    /// Draws a `CadLine`, `CadCircle` or `CadArc` on the current layer.
    fn add_geometry(&self, geometry: &Bound<'_, PyAny>) -> PyResult<(EntityId, CadChangeSet)> {
        let g = geometry_arg(geometry)?;
        self.create(|cad| cad.add_geometry(g))
    }

    /// Gives a line, circle or arc new geometry of the same kind, keeping
    /// its id, layer, color and elevation (a grip stretch, a property edit).
    fn set_entity_geometry(
        &self,
        entity_id: EntityId,
        geometry: &Bound<'_, PyAny>,
    ) -> PyResult<CadChangeSet> {
        let g = geometry_arg(geometry)?;
        self.change(|cad| cad.set_entity_geometry(entity_id, g))
    }

    /// Moves one lightweight-polyline vertex to a plan point.
    fn set_polyline_vertex(
        &self,
        entity_id: EntityId,
        vertex_index: usize,
        point: PyPoint,
    ) -> PyResult<CadChangeSet> {
        self.change(|cad| cad.set_polyline_vertex(entity_id, vertex_index, point.0))
    }

    /// Explodes block references into transformed block contents.
    fn explode_entities(
        &self,
        entity_ids: Vec<EntityId>,
    ) -> PyResult<(Vec<EntityId>, CadChangeSet)> {
        self.edit(|cad| cad.explode_entities(&entity_ids))
            .map(|(ids, changes)| (ids, CadChangeSet(changes)))
    }

    /// Erases entities of any type (ERASE).
    fn delete_entities(&self, entity_ids: Vec<EntityId>) -> PyResult<CadChangeSet> {
        self.change(|cad| cad.delete_entities(&entity_ids))
    }

    /// Copies entities, the copies moved by `offset` (COPY); returns the
    /// copies' ids, in order without repeats, and what changed. Lines,
    /// circles, arcs, blocks and polylines can be copied.
    fn copy_entities(
        &self,
        entity_ids: Vec<EntityId>,
        offset: PyVector,
    ) -> PyResult<(Vec<EntityId>, CadChangeSet)> {
        self.edit(|cad| cad.copy_entities(&entity_ids, offset.0))
            .map(|(ids, changes)| (ids, CadChangeSet(changes)))
    }

    /// Moves entities by `offset` (MOVE).
    fn move_entities(&self, entity_ids: Vec<EntityId>, offset: PyVector) -> PyResult<CadChangeSet> {
        self.change(|cad| cad.move_entities(&entity_ids, offset.0))
    }

    /// Rotates entities by `angle` radians counterclockwise around `center`
    /// (ROTATE).
    fn rotate_entities(
        &self,
        entity_ids: Vec<EntityId>,
        center: PyPoint,
        angle: f64,
    ) -> PyResult<CadChangeSet> {
        self.change(|cad| cad.rotate_entities(&entity_ids, center.0, angle))
    }

    /// Scales entities by `factor` around `center` (SCALE).
    fn scale_entities(
        &self,
        entity_ids: Vec<EntityId>,
        center: PyPoint,
        factor: f64,
    ) -> PyResult<CadChangeSet> {
        self.change(|cad| cad.scale_entities(&entity_ids, center.0, factor))
    }

    /// Mirrors entities across the line through `first` and `second`
    /// (MIRROR).
    fn mirror_entities(
        &self,
        entity_ids: Vec<EntityId>,
        first: PyPoint,
        second: PyPoint,
    ) -> PyResult<CadChangeSet> {
        let axis = Line::new(first.0, second.0).py()?;
        self.change(|cad| cad.mirror_entities(&entity_ids, axis))
    }

    /// Moves entities to another layer (CHPROP).
    fn set_entity_layer(
        &self,
        entity_ids: Vec<EntityId>,
        layer_id: LayerId,
    ) -> PyResult<CadChangeSet> {
        self.change(|cad| cad.set_entity_layer(&entity_ids, layer_id))
    }

    /// Sets the color of entities (CHPROP).
    fn set_entity_color(
        &self,
        entity_ids: Vec<EntityId>,
        color: CadColor,
    ) -> PyResult<CadChangeSet> {
        self.change(|cad| cad.set_entity_color(&entity_ids, color.0))
    }

    /// Creates a layer with AutoCAD's defaults; returns its id.
    fn create_layer(&self, name: &str) -> PyResult<(LayerId, CadChangeSet)> {
        self.create(|cad| cad.create_layer(name))
    }

    /// Renames a layer; its entities follow.
    fn rename_layer(&self, layer_id: LayerId, name: &str) -> PyResult<CadChangeSet> {
        self.change(|cad| cad.rename_layer(layer_id, name))
    }

    /// Sets a layer's color: an index or RGB color.
    fn set_layer_color(&self, layer_id: LayerId, color: CadColor) -> PyResult<CadChangeSet> {
        self.change(|cad| cad.set_layer_color(layer_id, color.0))
    }

    /// Turns a layer on or off.
    fn set_layer_visible(&self, layer_id: LayerId, visible: bool) -> PyResult<CadChangeSet> {
        self.change(|cad| cad.set_layer_visible(layer_id, visible))
    }

    /// Freezes or thaws a layer; the current layer cannot be frozen.
    fn set_layer_frozen(&self, layer_id: LayerId, frozen: bool) -> PyResult<CadChangeSet> {
        self.change(|cad| cad.set_layer_frozen(layer_id, frozen))
    }

    /// Locks or unlocks a layer.
    fn set_layer_locked(&self, layer_id: LayerId, locked: bool) -> PyResult<CadChangeSet> {
        self.change(|cad| cad.set_layer_locked(layer_id, locked))
    }

    /// Whether a layer is plotted.
    fn set_layer_plottable(&self, layer_id: LayerId, plottable: bool) -> PyResult<CadChangeSet> {
        self.change(|cad| cad.set_layer_plottable(layer_id, plottable))
    }

    /// Deletes an empty layer that is not "0", "Defpoints" or current.
    fn delete_layer(&self, layer_id: LayerId) -> PyResult<CadChangeSet> {
        self.change(|cad| cad.delete_layer(layer_id))
    }

    /// Makes a layer current; a frozen layer cannot be current.
    fn set_current_layer(&self, layer_id: LayerId) -> PyResult<CadChangeSet> {
        self.change(|cad| cad.set_current_layer(layer_id))
    }

    // History.

    /// Undoes the last step; returns what that did.
    fn undo(&self) -> PyResult<CadChangeSet> {
        self.change(CoreDocument::undo)
    }

    /// Redoes the last undone step; returns what that did.
    fn redo(&self) -> PyResult<CadChangeSet> {
        self.change(CoreDocument::redo)
    }

    /// Whether there is a step to undo.
    #[getter]
    fn can_undo(&self) -> PyResult<bool> {
        self.read(CoreDocument::can_undo)
    }

    /// Whether there is a step to redo.
    #[getter]
    fn can_redo(&self) -> PyResult<bool> {
        self.read(CoreDocument::can_redo)
    }

    /// Name of the step `undo` would revert (`"MOVE"`…), if any.
    #[getter]
    fn undo_name(&self) -> PyResult<Option<String>> {
        self.read(|cad| cad.undo_name().map(str::to_owned))
    }

    /// Name of the step `redo` would apply, if any.
    #[getter]
    fn redo_name(&self) -> PyResult<Option<String>> {
        self.read(|cad| cad.redo_name().map(str::to_owned))
    }

    /// Every step that can be undone, oldest first (the last is `undo_name`).
    #[getter]
    fn undo_history(&self) -> PyResult<Vec<String>> {
        self.read(|cad| cad.undo_names().into_iter().map(str::to_owned).collect())
    }

    /// Every step that can be redone, next first (the first is `redo_name`).
    #[getter]
    fn redo_history(&self) -> PyResult<Vec<String>> {
        self.read(|cad| cad.redo_names().into_iter().map(str::to_owned).collect())
    }

    /// A context manager making the operations inside one undo step:
    /// `with doc.group("COPY") as group: ...`.
    fn group(slf: Py<Self>, name: String) -> CadGroup {
        CadGroup::new(slf, name)
    }

    /// Starts a group by hand (for a command spread over several calls);
    /// prefer `group()`.
    fn begin_group(&self, name: &str) -> PyResult<()> {
        self.begin(name)
    }

    /// Ends the innermost group; the outermost returns what it did.
    fn end_group(&self) -> PyResult<CadChangeSet> {
        self.end().map(CadChangeSet)
    }

    /// Cancels the open group at every level, undoing it.
    fn cancel_group(&self) -> PyResult<CadChangeSet> {
        self.cancel().map(CadChangeSet)
    }

    /// Whether a group is open.
    #[getter]
    fn in_group(&self) -> PyResult<bool> {
        self.group_open()
    }

    /// Saves as DWG or DXF, by the file extension (see
    /// `CadApplication.save_document`).
    fn save(&self, py: Python<'_>, path: PathBuf) -> PyResult<()> {
        save(py, &self.store, self.id, path)
    }

    /// Closes it, discarding unsaved changes.
    fn close(&self) -> PyResult<()> {
        self.store.close_document(self.id).py()
    }

    fn __eq__(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.store, &other.store) && self.id == other.id
    }

    fn __hash__(&self) -> u64 {
        self.id
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(match self.store.document_name(self.id) {
            Ok(name) => {
                let name = pyo3::types::PyString::new(py, &name).repr()?;
                format!("CadDocument(id={}, name={name})", self.id)
            }
            Err(_) => format!("CadDocument(id={}, closed)", self.id),
        })
    }
}

/// A `CadLine`, `CadCircle` or `CadArc` passed from Python.
fn geometry_arg(geometry: &Bound<'_, PyAny>) -> PyResult<Geometry2> {
    if let Ok(l) = geometry.cast::<CadLine>() {
        Ok(l.get().0.into())
    } else if let Ok(c) = geometry.cast::<CadCircle>() {
        Ok(c.get().0.into())
    } else if let Ok(a) = geometry.cast::<CadArc>() {
        Ok(a.get().0.into())
    } else {
        Err(PyTypeError::new_err(
            "geometry must be a CadLine, CadCircle or CadArc",
        ))
    }
}

/// Adds the classes to the module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<CadApplication>()?;
    m.add_class::<CadDocument>()?;
    Ok(())
}
