//! Graphics for Python: the isolated GPU probe, and the one `unsafe` step of
//! the GPU viewport, a wgpu surface made from a window's native handles on
//! X11, Windows or macOS (ADR 0034 D1). It lives here, at the boundary, so that every other crate
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

/// The window system a viewport's native window belongs to, as Qt runs on
/// it: X11 (`xcb`), Windows or macOS (`cocoa`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WindowSystem {
    X11,
    Win32,
    AppKit,
}

impl WindowSystem {
    pub(crate) fn parse(name: &str) -> Result<Self, String> {
        match name {
            "x11" => Ok(Self::X11),
            "win32" => Ok(Self::Win32),
            "appkit" => Ok(Self::AppKit),
            other => Err(format!("unknown window system: {other}")),
        }
    }
}

/// The display a surface is made for: Qt's own X11 connection (wgpu's GL
/// backend picks its EGL platform when the instance is made, so the
/// instance needs it as well as the surface, and Vulkan needs the real
/// `Display*`), or the system's, which has no handle of its own.
#[derive(Debug)]
enum NativeDisplay {
    Xlib(std::ptr::NonNull<std::ffi::c_void>),
    Windows,
    AppKit,
}

// SAFETY: this type never dereferences the pointer; it only hands it to the
// graphics driver. The view that owns it is `Send` and `Sync` as a Python
// object, but the viewport presents only from Qt's GUI thread, the thread
// that owns the connection.
unsafe impl Send for NativeDisplay {}
// SAFETY: as above.
unsafe impl Sync for NativeDisplay {}

impl NativeDisplay {
    fn raw(&self) -> rwh::RawDisplayHandle {
        match self {
            Self::Xlib(display) => {
                rwh::RawDisplayHandle::Xlib(rwh::XlibDisplayHandle::new(Some(*display), 0))
            }
            Self::Windows => rwh::RawDisplayHandle::Windows(rwh::WindowsDisplayHandle::new()),
            Self::AppKit => rwh::RawDisplayHandle::AppKit(rwh::AppKitDisplayHandle::new()),
        }
    }
}

impl rwh::HasDisplayHandle for NativeDisplay {
    fn display_handle(&self) -> Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        // SAFETY: the caller of `window_surface` keeps the connection open
        // for as long as the instance and its surface live.
        Ok(unsafe { rwh::DisplayHandle::borrow_raw(self.raw()) })
    }
}

/// A surface for a native `window` of `system`, with the instance that
/// made it, on `backend`: on X11 the window's XID and `display` the
/// `Display*`; on Windows its `HWND`; on macOS its `NSView*` (wgpu gives it
/// a Metal layer). `display` is ignored but on X11.
///
/// # Safety
///
/// The handles must be live, and outlive the returned instance and surface.
/// Qt's viewport keeps them so: the view detaches the window before the
/// widget goes.
pub(crate) unsafe fn window_surface(
    backend: GpuBackend,
    system: WindowSystem,
    display: usize,
    window: u64,
) -> Result<(wgpu::Instance, wgpu::Surface<'static>), String> {
    let native = match system {
        WindowSystem::X11 => NativeDisplay::Xlib(
            std::ptr::NonNull::new(display as *mut std::ffi::c_void)
                .ok_or("no X11 display: the GPU viewport needs Qt on X11 (xcb)")?,
        ),
        WindowSystem::Win32 => NativeDisplay::Windows,
        WindowSystem::AppKit => NativeDisplay::AppKit,
    };
    let raw_window = match system {
        WindowSystem::X11 => {
            let id = std::num::NonZero::new(window).ok_or("no window")?;
            rwh::RawWindowHandle::Xlib(rwh::XlibWindowHandle::new(id.get() as _))
        }
        WindowSystem::Win32 => {
            let hwnd = std::num::NonZeroIsize::new(window as isize).ok_or("no window")?;
            rwh::RawWindowHandle::Win32(rwh::Win32WindowHandle::new(hwnd))
        }
        WindowSystem::AppKit => {
            let view = std::ptr::NonNull::new(window as usize as *mut std::ffi::c_void)
                .ok_or("no window")?;
            rwh::RawWindowHandle::AppKit(rwh::AppKitWindowHandle::new(view))
        }
    };
    let raw_display = native.raw();
    let mut descriptor = wgpu::InstanceDescriptor::new_with_display_handle(Box::new(native));
    descriptor.backends = backend.wgpu_backends();
    let instance = wgpu::Instance::new(descriptor);
    let target = wgpu::SurfaceTargetUnsafe::RawHandle {
        raw_display_handle: Some(raw_display),
        raw_window_handle: raw_window,
    };
    // SAFETY: the caller keeps the display and the window alive.
    let surface = unsafe { instance.create_surface_unsafe(target) }.map_err(|e| e.to_string())?;
    Ok((instance, surface))
}

/// Registers graphics diagnostics on the Python module.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(probe_gpu_backend, module)?)
}
