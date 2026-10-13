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
    CadApplication as CoreApplication, CadChangeSet as ChangeSet, CadDocument as CoreDocument,
    CadError, DocumentId, EntityId, LayerId,
};
use arq_geometry::{Geometry2, Line};
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;

use crate::changes::{CadChangeSet, CadGroup};
use crate::errors::IntoPy;
use crate::info::{CadEntityInfo, CadLayerInfo};
use crate::values::{CadArc, CadBounds2, CadCircle, CadColor, CadLine, PyPoint, PyVector};

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
        let g: Geometry2 = if let Ok(l) = geometry.cast::<CadLine>() {
            l.get().0.into()
        } else if let Ok(c) = geometry.cast::<CadCircle>() {
            c.get().0.into()
        } else if let Ok(a) = geometry.cast::<CadArc>() {
            a.get().0.into()
        } else {
            return Err(PyTypeError::new_err(
                "geometry must be a CadLine, CadCircle or CadArc",
            ));
        };
        self.create(|cad| cad.add_geometry(g))
    }

    /// Erases entities of any type (ERASE).
    fn delete_entities(&self, entity_ids: Vec<EntityId>) -> PyResult<CadChangeSet> {
        self.change(|cad| cad.delete_entities(&entity_ids))
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

/// Adds the classes to the module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<CadApplication>()?;
    m.add_class::<CadDocument>()?;
    Ok(())
}
