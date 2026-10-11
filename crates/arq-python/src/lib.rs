//! Python bindings for ArqStudioCAD, exposed as `arqstudiocad._core`.
//!
//! This is the only crate that knows about Python. It converts types and
//! errors; it contains no CAD logic of its own.

use pyo3::prelude::*;

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", arq_core::version())?;
    Ok(())
}
