//! Python bindings for ArqStudioCAD, exposed as `arqstudiocad._core`.
//!
//! This is the only crate that knows about Python. It converts types and
//! errors; it contains no CAD logic of its own. Decisions are in ADR 0012;
//! Python code imports from `arqstudiocad.core`, typed by `_core.pyi`.
//!
//! | Module | Python names |
//! |---|---|
//! | `errors` | `CadError` and one subclass per core error, each with a `code` |
//! | `values` | `CadColor`, `CadBounds2`, `CadLine`, `CadCircle`, `CadArc`; points as tuples |
//! | `documents` | `CadApplication`, `CadDocument` (queries, operations, history) |
//! | `info` | `CadEntityInfo`, `CadLayerInfo` |
//! | `changes` | `CadChangeSet`, `CadGroup` |
//! | `view` | `CadView`: the document rendered on the CPU for the viewport |
//!
//! Two rules keep it safe across threads: never run Python code while a
//! document is locked (build Python objects after the lock is released), and
//! release the GIL only to open and save.

use pyo3::prelude::*;

mod changes;
mod documents;
mod errors;
mod gpu;
mod info;
mod values;
mod view;

/// Names this program, `name` at `version`, in every DWG saved from now on,
/// so a drawing saved here is known as ours when read again (ADR 0045). The
/// application calls it once, with its product name.
#[pyfunction]
fn set_writer(name: &str, version: &str) {
    arq_core::set_writer(name, version);
}

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", arq_core::version())?;
    m.add_function(wrap_pyfunction!(set_writer, m)?)?;
    errors::register(m)?;
    gpu::register(m)?;
    documents::register(m)?;
    info::register(m)?;
    changes::register(m)?;
    values::register(m)?;
    view::register(m)?;
    Ok(())
}
