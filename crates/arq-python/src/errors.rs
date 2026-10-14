//! Core errors as Python exceptions (contract §4, ADR 0012 D4.4).
//!
//! `CadError` is the base; each variant of `arq_core::CadError` is a
//! subclass with a stable `code` class attribute and an English technical
//! message. The not-found errors also carry the id that was not found.

use arq_core::CadError as CoreError;
use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;

create_exception!(
    arqstudiocad._core,
    CadError,
    PyException,
    "Base of every error raised by the CAD core."
);
create_exception!(
    arqstudiocad._core,
    InvalidGeometryError,
    CadError,
    "A geometric value was invalid: NaN, zero radius, zero-length segment…"
);
create_exception!(
    arqstudiocad._core,
    EntityNotFoundError,
    CadError,
    "No entity has this id."
);
create_exception!(
    arqstudiocad._core,
    LayerNotFoundError,
    CadError,
    "No layer has this id."
);
create_exception!(
    arqstudiocad._core,
    DocumentNotFoundError,
    CadError,
    "No open document has this id."
);
create_exception!(
    arqstudiocad._core,
    InvalidOperationError,
    CadError,
    "The operation is not allowed now: locked layer, read-only entity…"
);
create_exception!(
    arqstudiocad._core,
    CadIoError,
    CadError,
    "Reading or writing a file failed."
);
create_exception!(
    arqstudiocad._core,
    DamagedFileError,
    CadIoError,
    "The file is damaged and no drawing could be recovered from it."
);
create_exception!(
    arqstudiocad._core,
    SaveNotVerifiedError,
    CadIoError,
    "Not saved: the written file did not read back as the drawing."
);
create_exception!(
    arqstudiocad._core,
    LossyConversionError,
    CadIoError,
    "Not saved: the file would lose data its format or version cannot keep."
);
create_exception!(
    arqstudiocad._core,
    OverwritesDamagedError,
    CadIoError,
    "Saving would replace the damaged file the drawing was recovered from."
);
create_exception!(
    arqstudiocad._core,
    UnsupportedFormatError,
    CadError,
    "The file format is not supported."
);

/// Each exception class and its stable code, for registration.
fn classes(py: Python<'_>) -> [(Bound<'_, pyo3::types::PyType>, &'static str); 12] {
    [
        (py.get_type::<CadError>(), "cad_error"),
        (py.get_type::<InvalidGeometryError>(), "invalid_geometry"),
        (py.get_type::<EntityNotFoundError>(), "entity_not_found"),
        (py.get_type::<LayerNotFoundError>(), "layer_not_found"),
        (py.get_type::<DocumentNotFoundError>(), "document_not_found"),
        (py.get_type::<InvalidOperationError>(), "invalid_operation"),
        (py.get_type::<CadIoError>(), "io_error"),
        (py.get_type::<DamagedFileError>(), "damaged_file"),
        (py.get_type::<LossyConversionError>(), "lossy_conversion"),
        (py.get_type::<SaveNotVerifiedError>(), "save_not_verified"),
        (
            py.get_type::<OverwritesDamagedError>(),
            "overwrites_damaged",
        ),
        (
            py.get_type::<UnsupportedFormatError>(),
            "unsupported_format",
        ),
    ]
}

/// Adds the exception classes, with their codes, to the module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    for (class, code) in classes(m.py()) {
        class.setattr("code", code)?;
        m.add(class.name()?, class)?;
    }
    Ok(())
}

/// The Python exception for a core error.
pub fn to_py(error: CoreError) -> PyErr {
    let message = error.to_string();
    let (err, id) = match error {
        CoreError::InvalidGeometry(_) => (InvalidGeometryError::new_err(message), None),
        CoreError::EntityNotFound(id) => (
            EntityNotFoundError::new_err(message),
            Some(("entity_id", id)),
        ),
        CoreError::LayerNotFound(id) => {
            (LayerNotFoundError::new_err(message), Some(("layer_id", id)))
        }
        CoreError::DocumentNotFound(id) => (
            DocumentNotFoundError::new_err(message),
            Some(("document_id", id)),
        ),
        CoreError::InvalidOperation(_) => (InvalidOperationError::new_err(message), None),
        CoreError::Io(_) => (CadIoError::new_err(message), None),
        CoreError::DamagedFile(_) => (DamagedFileError::new_err(message), None),
        CoreError::NotVerified(_) => (SaveNotVerifiedError::new_err(message), None),
        CoreError::Lossy { kinds, losses, .. } => {
            let err = LossyConversionError::new_err(message);
            Python::attach(|py| {
                // As for the ids below: cannot fail in practice.
                let _ = err.value(py).setattr("loss_kinds", kinds);
                let _ = err.value(py).setattr("losses", losses);
            });
            return err;
        }
        CoreError::OverwritesDamaged(_) => (OverwritesDamagedError::new_err(message), None),
        CoreError::UnsupportedFormat(_) => (UnsupportedFormatError::new_err(message), None),
    };
    if let Some((name, id)) = id {
        Python::attach(|py| {
            // Setting an attribute on a fresh exception object cannot fail in
            // practice; if it did, the exception is still the right class.
            let _ = err.value(py).setattr(name, id);
        });
    }
    err
}

/// `?`-friendly conversion: `result.map_err(to_py)` without naming it.
pub trait IntoPy<T> {
    /// The value, or the core error as a Python exception.
    fn py(self) -> PyResult<T>;
}

impl<T, E: Into<CoreError>> IntoPy<T> for Result<T, E> {
    fn py(self) -> PyResult<T> {
        self.map_err(|e| to_py(e.into()))
    }
}
