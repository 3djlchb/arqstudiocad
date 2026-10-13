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
//!
//! Two rules keep it safe across threads: never run Python code while a
//! document is locked (build Python objects after the lock is released), and
//! release the GIL only to open and save.

use pyo3::prelude::*;

mod changes;
mod documents;
mod errors;
mod info;
mod values;

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", arq_core::version())?;
    errors::register(m)?;
    documents::register(m)?;
    info::register(m)?;
    changes::register(m)?;
    values::register(m)?;
    Ok(())
}
