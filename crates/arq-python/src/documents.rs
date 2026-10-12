//! `CadApplication` and the `CadDocument` handle (ADR 0012 D4.3, D4.6).
//!
//! A `CadDocument` is the shared store plus a document id: it holds no
//! reference into the drawing, and every call locks the document for that
//! call only. Opening and saving release the GIL while the file is read or
//! written.
//!
//! Rule for every call: never run Python code while a document is locked.
//! A thread that released the GIL may hold that lock, and it must be able to
//! finish without needing the GIL back.

use std::path::PathBuf;
use std::sync::Arc;

use arq_core::{CadApplication as CoreApplication, CadDocument as CoreDocument, DocumentId};
use pyo3::prelude::*;

use crate::errors::IntoPy;

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
    #[allow(dead_code, reason = "operations arrive in step 4.4")]
    pub fn edit<R>(
        &self,
        f: impl FnOnce(&mut CoreDocument) -> arq_core::CadResult<R>,
    ) -> PyResult<R> {
        self.store.edit(self.id, f).py()
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
