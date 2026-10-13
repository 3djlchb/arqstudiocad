//! `CadChangeSet` and `CadGroup`, the `with doc.group(...)` context manager
//! (contract §6, ADR 0012 D4.5).

use std::sync::Mutex;

use arq_core::CadChangeSet as ChangeSet;
use pyo3::prelude::*;
use pyo3::types::{PyTuple, PyType};

use crate::documents::CadDocument;

/// What one change did: entities and layers created, modified and deleted,
/// and whether a document setting (such as the current layer) changed. The
/// interface refreshes only that.
#[pyclass(frozen, eq, skip_from_py_object, module = "arqstudiocad._core")]
#[derive(Clone, PartialEq)]
pub struct CadChangeSet(pub ChangeSet);

#[pymethods]
impl CadChangeSet {
    /// Entities that now exist and did not before.
    #[getter]
    fn created<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, self.0.created())
    }

    /// Entities whose geometry or properties changed.
    #[getter]
    fn modified<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, self.0.modified())
    }

    /// Entities that no longer exist.
    #[getter]
    fn deleted<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, self.0.deleted())
    }

    /// Layers created.
    #[getter]
    fn layers_created<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, self.0.layers_created())
    }

    /// Layers renamed or with changed properties.
    #[getter]
    fn layers_modified<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, self.0.layers_modified())
    }

    /// Layers deleted.
    #[getter]
    fn layers_deleted<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, self.0.layers_deleted())
    }

    /// Whether a document-level setting changed (e.g. the current layer).
    #[getter]
    fn document_changed(&self) -> bool {
        self.0.document_changed()
    }

    /// Whether nothing changed.
    #[getter]
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn __repr__(&self) -> String {
        let c = &self.0;
        let mut parts = Vec::new();
        for (name, ids) in [
            ("created", c.created()),
            ("modified", c.modified()),
            ("deleted", c.deleted()),
            ("layers_created", c.layers_created()),
            ("layers_modified", c.layers_modified()),
            ("layers_deleted", c.layers_deleted()),
        ] {
            if !ids.is_empty() {
                parts.push(format!("{name}={ids:?}"));
            }
        }
        if c.document_changed() {
            parts.push("document_changed=True".to_owned());
        }
        format!("CadChangeSet({})", parts.join(", "))
    }
}

/// `with doc.group("COPY") as group:` — the operations inside become one
/// undo step. On normal exit the group ends and `group.changes` holds what
/// it did; if an exception escapes, the group is cancelled (every nesting
/// level) and the exception goes on.
#[pyclass(frozen, module = "arqstudiocad._core")]
pub struct CadGroup {
    doc: Py<CadDocument>,
    name: String,
    changes: Mutex<Option<ChangeSet>>,
}

impl CadGroup {
    pub fn new(doc: Py<CadDocument>, name: String) -> Self {
        Self {
            doc,
            name,
            changes: Mutex::new(None),
        }
    }
}

#[pymethods]
impl CadGroup {
    /// The undo step's name.
    #[getter]
    fn name(&self) -> &str {
        &self.name
    }

    /// What the group did, once it ended normally; `None` before that or
    /// if it was cancelled.
    #[getter]
    fn changes(&self) -> Option<CadChangeSet> {
        self.lock().clone().map(CadChangeSet)
    }

    fn __enter__(slf: Bound<'_, Self>) -> PyResult<Bound<'_, Self>> {
        let me = slf.get();
        me.doc.get().begin(&me.name)?;
        Ok(slf)
    }

    fn __exit__(
        &self,
        exc_type: Option<&Bound<'_, PyType>>,
        exc_value: Option<&Bound<'_, PyAny>>,
        traceback: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        let _ = (exc_value, traceback); // only whether an exception escaped matters
        let doc = self.doc.get();
        if exc_type.is_none() {
            let changes = doc.end()?;
            *self.lock() = Some(changes);
        } else if doc.group_open()? {
            // A failed operation has already rolled the whole group back;
            // anything else that escapes cancels it here.
            doc.cancel()?;
        }
        Ok(false) // never swallow the exception
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let name = pyo3::types::PyString::new(py, &self.name).repr()?;
        Ok(format!("CadGroup(name={name})"))
    }
}

impl CadGroup {
    fn lock(&self) -> std::sync::MutexGuard<'_, Option<ChangeSet>> {
        self.changes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Adds the classes to the module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<CadChangeSet>()?;
    m.add_class::<CadGroup>()?;
    Ok(())
}
