//! Graphics diagnostics exposed for the isolated GPU probe process.

use arq_render::{GpuBackend, probe_gpu_backend as run_probe};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

#[pyfunction]
fn probe_gpu_backend(backend: &str) -> PyResult<(String, String, String)> {
    let backend = GpuBackend::parse(backend).map_err(PyValueError::new_err)?;
    let result = Python::attach(|py| py.detach(move || run_probe(backend)))
        .map_err(PyRuntimeError::new_err)?;
    Ok((result.backend, result.adapter, result.device_type))
}

/// Registers graphics diagnostics on the Python module.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(probe_gpu_backend, module)?)
}
