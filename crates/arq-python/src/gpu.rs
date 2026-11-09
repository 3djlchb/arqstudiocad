//! Graphics for Python: the isolated GPU probe, and the one `unsafe` step of
//! the GPU viewport, a wgpu surface made from a window's native handles
//! (ADR 0034 D1). It lives here, at the boundary, so that every other crate
//! keeps `unsafe` forbidden.

use arq_render::wgpu;
use arq_render::wgpu::rwh;
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

/// An X11 connection, Qt's own: wgpu's GL backend picks its EGL platform
/// when the instance is made, so the instance needs it as well as the
/// surface, and Vulkan needs the real `Display*`.
#[derive(Debug)]
struct XlibDisplay(std::ptr::NonNull<std::ffi::c_void>);

// SAFETY: this type never dereferences the pointer; it only hands it to the
// graphics driver. The view that owns it is `Send` and `Sync` as a Python
// object, but the viewport presents only from Qt's GUI thread, the thread
// that owns the connection.
unsafe impl Send for XlibDisplay {}
// SAFETY: as above.
unsafe impl Sync for XlibDisplay {}

impl XlibDisplay {
    fn raw(&self) -> rwh::RawDisplayHandle {
        rwh::RawDisplayHandle::Xlib(rwh::XlibDisplayHandle::new(Some(self.0), 0))
    }
}

impl rwh::HasDisplayHandle for XlibDisplay {
    fn display_handle(&self) -> Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        // SAFETY: the caller of `window_surface` keeps the connection open
        // for as long as the instance and its surface live.
        Ok(unsafe { rwh::DisplayHandle::borrow_raw(self.raw()) })
    }
}

/// A surface for the X11 `window` of the `display` connection, with the
/// instance that made it, on `backend`.
///
/// # Safety
///
/// `display` must be a live `Display*` and `window` a live window of it,
/// both outliving the returned instance and surface. Qt's viewport keeps
/// them so: the view detaches the window before the widget goes.
pub(crate) unsafe fn window_surface(
    backend: GpuBackend,
    display: usize,
    window: u64,
) -> Result<(wgpu::Instance, wgpu::Surface<'static>), String> {
    let pointer = std::ptr::NonNull::new(display as *mut std::ffi::c_void)
        .ok_or("no X11 display: the GPU viewport needs Qt on X11 (xcb)")?;
    let display = XlibDisplay(pointer);
    let raw_display = display.raw();
    let mut descriptor = wgpu::InstanceDescriptor::new_with_display_handle(Box::new(display));
    descriptor.backends = backend.wgpu_backends();
    let instance = wgpu::Instance::new(descriptor);
    let window = std::num::NonZero::new(window).ok_or("no window")?;
    let target = wgpu::SurfaceTargetUnsafe::RawHandle {
        raw_display_handle: Some(raw_display),
        raw_window_handle: rwh::RawWindowHandle::Xlib(
            rwh::XlibWindowHandle::new(window.get() as _),
        ),
    };
    // SAFETY: the caller keeps the display and the window alive.
    let surface = unsafe { instance.create_surface_unsafe(target) }.map_err(|e| e.to_string())?;
    Ok((instance, surface))
}

/// Registers graphics diagnostics on the Python module.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(probe_gpu_backend, module)?)
}
