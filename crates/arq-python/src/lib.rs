//! Python bindings for ArqStudioCAD, exposed as `arqstudiocad._core`.
//!
//! This is the only crate that knows about Python. It converts types and
//! errors; it contains no CAD logic of its own. Decisions are in ADR 0012.

use pyo3::prelude::*;

mod documents;
mod errors;
mod values;

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", arq_core::version())?;
    errors::register(m)?;
    documents::register(m)?;
    values::register(m)?;
    Ok(())
}
