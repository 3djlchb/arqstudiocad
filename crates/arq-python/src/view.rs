//! `CadView`: a document seen through a camera, rendered on the CPU into an
//! image Qt can show (ADR 0013, D5.3), or presented by the GPU straight into
//! a native window (ADR 0034).
//!
//! The view keeps its own render scene (derived data, never the source of
//! truth) and camera behind one lock. Building the scene and rendering run
//! with the GIL released. Lock order: the view first, then the document,
//! never the other way, and neither needs the GIL while held.

use std::sync::{Mutex, MutexGuard, PoisonError};

use arq_render::{
    Camera, GpuBackend, GpuRenderer, Overlay, OverlayItem, OverlayLabel, Scene, Style, View, render,
};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::changes::CadChangeSet;
use crate::documents::CadDocument;
use crate::errors::IntoPy;
use crate::values::CadBounds2;

struct State {
    scene: Scene,
    camera: Camera,
    style: Style,
    /// The GPU presenting into a window, when attached.
    gpu: Option<GpuRenderer>,
}

/// A view of a document: what it draws, where the camera looks, and the
/// rendered image. Pixel coordinates have their origin at the top-left
/// corner and grow down; world coordinates grow up.
#[pyclass(frozen, module = "arqstudiocad._core")]
pub struct CadView {
    document: Py<CadDocument>,
    state: Mutex<State>,
}

impl CadView {
    fn state(&self) -> MutexGuard<'_, State> {
        // Nothing in the view can be left half-updated by a panic: recover.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[pymethods]
impl CadView {
    /// A `width` × `height` view of `document`, showing all of it from the
    /// view the drawing keeps (ADR 0030 D2).
    #[new]
    fn new(py: Python<'_>, document: Py<CadDocument>, width: u32, height: u32) -> PyResult<Self> {
        let doc = document.get();
        let scene = py
            .detach(|| {
                doc.read_core(|cad| {
                    let doc = cad.document();
                    Scene::build_in(doc, View::kept_in(doc))
                })
            })
            .py()?;
        let mut camera = Camera::new(width, height);
        if let Some(bounds) = scene.bounds() {
            camera.fit(bounds);
        }
        Ok(Self {
            document,
            state: Mutex::new(State {
                scene,
                camera,
                style: Style::default(),
                gpu: None,
            }),
        })
    }

    /// The document shown.
    #[getter]
    fn document(&self, py: Python<'_>) -> Py<CadDocument> {
        self.document.clone_ref(py)
    }

    /// Brings the view up to date after a change; returns whether anything
    /// it shows changed.
    fn update(&self, py: Python<'_>, changes: &CadChangeSet) -> PyResult<bool> {
        let changes = changes.0.clone();
        let doc = self.document.get();
        py.detach(|| {
            let mut state = self.state();
            doc.read_core(|cad| state.scene.update(cad.document(), &changes))
        })
        .py()
    }

    /// Builds the scene again from the whole document.
    fn rebuild(&self, py: Python<'_>) -> PyResult<()> {
        let doc = self.document.get();
        py.detach(|| {
            let mut state = self.state();
            let view = state.scene.view();
            state.scene = doc.read_core(|cad| Scene::build_in(cad.document(), view))?;
            Ok::<_, arq_core::CadError>(())
        })
        .py()
    }

    /// Which way the model is seen (ADR 0030): `"top"` (the plan),
    /// `"bottom"`, `"front"`, `"back"`, `"left"`, `"right"`, `"swiso"`,
    /// `"seiso"`, `"neiso"` or `"nwiso"`.
    #[getter]
    fn view(&self) -> &'static str {
        self.state().scene.view().name()
    }

    /// Sees the model from another view, showing all of it, and keeps that
    /// view in the drawing, saved with it (ADR 0030 D2; not an edit). The
    /// same view keeps the zoom. An unknown name raises `ValueError`.
    #[setter]
    fn set_view(&self, py: Python<'_>, name: &str) -> PyResult<()> {
        let view = View::from_name(name)
            .ok_or_else(|| PyValueError::new_err(format!("unknown view {name:?}")))?;
        let doc = self.document.get();
        py.detach(|| {
            let mut state = self.state();
            if state.scene.view() == view {
                return Ok(());
            }
            state.scene = doc.read_core(|cad| Scene::build_in(cad.document(), view))?;
            if let Some(bounds) = state.scene.bounds() {
                state.camera.fit(bounds);
            }
            // What is shown, as AutoCAD would show it on opening.
            let camera = state.camera;
            let height = f64::from(camera.size().1) / camera.scale();
            doc.edit_core(|cad| cad.keep_view(view.direction(), camera.center(), height))
        })
        .py()
    }

    /// The image: `width * height * 4` bytes, RGBA, premultiplied alpha
    /// (`QImage.Format.Format_RGBA8888_Premultiplied`), rows top to bottom.
    fn render<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        let image = py.detach(|| {
            let state = self.state();
            render(&state.scene, &state.camera, state.style)
        });
        PyBytes::new(py, &image.pixels)
    }

    /// Presents the view with the GPU straight into a native window (ADR
    /// 0034), `QWidget.winId()` as an integer, of the window `system`:
    /// `"x11"`, with `display` Qt's `Display*`
    /// (`QGuiApplication.nativeInterface().display()`); `"win32"`, the
    /// window an `HWND`; or `"appkit"`, an `NSView*`; `display` is ignored
    /// but on X11. `antialias` asks for 4× multisampling, where a hardware
    /// adapter offers it; it doubles a frame's time on the reference
    /// machine. Returns `(backend, adapter, device_type)`. The window
    /// and the connection must outlive the attachment: call
    /// `detach_window()` before the widget goes. An unknown backend raises
    /// `ValueError`; a GPU that cannot draw there, `RuntimeError`, and the
    /// view stays as it was.
    #[pyo3(signature = (backend, display, window, antialias = false, system = "x11"))]
    fn attach_window(
        &self,
        py: Python<'_>,
        backend: &str,
        display: usize,
        window: u64,
        antialias: bool,
        system: &str,
    ) -> PyResult<(String, String, String)> {
        let backend = GpuBackend::parse(backend).map_err(PyValueError::new_err)?;
        let system = crate::gpu::WindowSystem::parse(system).map_err(PyValueError::new_err)?;
        let (width, height) = self.state().camera.size();
        let renderer = py
            .detach(|| {
                // SAFETY: the caller's contract above: Qt keeps the window
                // and its display alive until `detach_window`.
                let (instance, surface) =
                    unsafe { crate::gpu::window_surface(backend, system, display, window) }?;
                GpuRenderer::for_surface(backend, instance, surface, width, height, antialias)
            })
            .map_err(PyRuntimeError::new_err)?;
        let adapter = renderer.adapter().clone();
        self.state().gpu = Some(renderer);
        Ok((adapter.backend, adapter.adapter, adapter.device_type))
    }

    /// Stops presenting into the window; `render()` keeps working.
    fn detach_window(&self) {
        self.state().gpu = None;
    }

    /// `(backend, adapter, device_type)` of the GPU presenting into a window,
    /// or `None` when no window is attached.
    #[getter]
    fn gpu(&self) -> Option<(String, String, String)> {
        let state = self.state();
        let adapter = state.gpu.as_ref()?.adapter();
        Some((
            adapter.backend.clone(),
            adapter.adapter.clone(),
            adapter.device_type.clone(),
        ))
    }

    /// Draws the view into the attached window with the GPU, the overlays
    /// over it (ADR 0034 D3, D4), in logical pixels scaled by `ratio`:
    /// `items` in order, each `("stroke", points, (r, g, b, a), width,
    /// dashed, closed, crisp)` or `("fill", (x0, y0), (x1, y1), (r, g, b,
    /// a))`; `labels` as `(x, y, width, height, pixels)` in physical pixels,
    /// RGBA premultiplied, as Qt painted them. Panning and zooming only move
    /// what the GPU keeps. On an error the window is detached and
    /// `RuntimeError` raised, so the caller draws with `render()` instead
    /// (ADR 0034 D5); a malformed item raises `ValueError`.
    #[pyo3(signature = (items = Vec::new(), labels = Vec::new(), ratio = 1.0))]
    fn present(
        &self,
        py: Python<'_>,
        items: Vec<Bound<'_, PyAny>>,
        labels: Vec<(i32, i32, u32, u32, Vec<u8>)>,
        ratio: f32,
    ) -> PyResult<()> {
        let overlay = overlay(&items, labels, ratio)?;
        py.detach(|| {
            let mut state = self.state();
            let State {
                scene,
                camera,
                style,
                gpu,
            } = &mut *state;
            let Some(renderer) = gpu.as_mut() else {
                return Err("no window attached".to_owned());
            };
            let result = renderer.present(scene, camera, *style, &overlay);
            if result.is_err() {
                *gpu = None;
            }
            result
        })
        .map_err(PyRuntimeError::new_err)
    }

    /// A new size in pixels; the center and scale stay.
    fn resize(&self, width: u32, height: u32) {
        self.state().camera.resize(width, height);
    }

    /// Shows everything drawn (zoom extents); `False` when nothing is.
    fn zoom_extents(&self) -> bool {
        let mut state = self.state();
        match state.scene.bounds() {
            Some(bounds) => {
                state.camera.fit(bounds);
                true
            }
            None => false,
        }
    }

    /// Zooms by `factor` (above 1 closer) about the pixel `(x, y)`, which
    /// keeps showing the same world point: zooming with the wheel.
    fn zoom_at(&self, x: f64, y: f64, factor: f64) {
        self.state().camera.zoom_at(x, y, factor);
    }

    /// Moves the view by a drag of `(dx, dy)` pixels.
    fn pan(&self, dx: f64, dy: f64) {
        self.state().camera.pan(dx, dy);
    }

    /// Pixel `(x, y)` to world coordinates; outside the plan, to the
    /// view's plane (across, up).
    fn to_world(&self, x: f64, y: f64) -> (f64, f64) {
        self.state().camera.to_world(x, y)
    }

    /// World `(x, y)` to pixels.
    fn to_screen(&self, x: f64, y: f64) -> (f64, f64) {
        self.state().camera.to_screen(x, y)
    }

    /// The entity drawn under pixel `(x, y)`, within `aperture` pixels, in
    /// any view (ADR 0030); `None` if none is that close.
    fn pick(&self, x: f64, y: f64, aperture: f64) -> Option<u64> {
        let state = self.state();
        let (u, v) = state.camera.to_world(x, y);
        let at = arq_geometry::Point2::new(u, v).ok()?;
        state.scene.pick(at, aperture / state.camera.scale())
    }

    /// What a window from pixel `(x0, y0)` to `(x1, y1)` selects, or with
    /// `crossing` a crossing, in any view (ADR 0030), in drawing order.
    fn select_window(&self, x0: f64, y0: f64, x1: f64, y1: f64, crossing: bool) -> Vec<u64> {
        let state = self.state();
        let corner = |x, y| {
            let (u, v) = state.camera.to_world(x, y);
            arq_geometry::Point2::new(u, v).ok()
        };
        match (corner(x0, y0), corner(x1, y1)) {
            (Some(a), Some(b)) => state
                .scene
                .select_window(arq_geometry::Bounds2::new(a, b), crossing),
            _ => Vec::new(),
        }
    }

    /// The point of the XY plane (z = 0) under pixel `(x, y)`, where a
    /// picked point falls in any view (ADR 0030 D4); in plan, as
    /// `to_world`. `None` where the view sees the plane edge on.
    fn plane_point(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let state = self.state();
        let (u, v) = state.camera.to_world(x, y);
        state.scene.view().plane_point(u, v)
    }

    /// A point of the XY plane (z = 0) to pixels, in any view; in plan, as
    /// `to_screen`.
    fn plane_to_screen(&self, x: f64, y: f64) -> (f64, f64) {
        let state = self.state();
        let view = state.scene.view();
        let (right, up) = view.axes();
        let (u, v) = (x * right[0] + y * right[1], x * up[0] + y * up[1]);
        state.camera.to_screen(u, v)
    }

    /// The outlines of these entities in pixels, as polylines, for an
    /// overlay painted over the image (selection, highlight; ADR 0016 D4):
    /// curves fine enough for their size on screen, texts and entities not
    /// drawn yet as their boxes. Hidden or unknown entities give nothing.
    fn outlines(&self, entity_ids: Vec<u64>) -> Vec<Vec<(f64, f64)>> {
        let state = self.state();
        arq_render::outline(&state.scene, &state.camera, &entity_ids)
    }

    /// Width in pixels.
    #[getter]
    fn width(&self) -> u32 {
        self.state().camera.size().0
    }

    /// Height in pixels.
    #[getter]
    fn height(&self) -> u32 {
        self.state().camera.size().1
    }

    /// Pixels per model unit.
    #[getter]
    fn scale(&self) -> f64 {
        self.state().camera.scale()
    }

    /// The world point at the middle of the view.
    #[getter]
    fn center(&self) -> (f64, f64) {
        self.state().camera.center()
    }

    /// The box around everything drawn; `None` when nothing is.
    #[getter]
    fn bounds(&self) -> Option<CadBounds2> {
        self.state().scene.bounds().map(CadBounds2)
    }

    /// How many shapes the view draws.
    #[getter]
    fn shape_count(&self) -> usize {
        self.state().scene.shape_count()
    }

    /// Background color, `(r, g, b)`.
    #[getter]
    fn background(&self) -> (u8, u8, u8) {
        let [r, g, b] = self.state().style.background;
        (r, g, b)
    }

    #[setter]
    fn set_background(&self, rgb: (u8, u8, u8)) {
        self.state().style.background = [rgb.0, rgb.1, rgb.2];
    }

    fn __repr__(&self) -> String {
        let state = self.state();
        let (w, h) = state.camera.size();
        format!("CadView({w}x{h}, {} shapes)", state.scene.shape_count())
    }
}

/// A color as Python gives it: red, green, blue, alpha.
type Rgba = (u8, u8, u8, u8);
/// `("stroke", points, rgba, width, dashed, closed, crisp)`.
type StrokeTuple = (String, Vec<(f32, f32)>, Rgba, f32, bool, bool, bool);
/// `("fill", (x0, y0), (x1, y1), rgba)`.
type FillTuple = (String, (f32, f32), (f32, f32), Rgba);

/// The overlay the viewport described, from Python's tagged tuples.
fn overlay(
    items: &[Bound<'_, PyAny>],
    labels: Vec<(i32, i32, u32, u32, Vec<u8>)>,
    ratio: f32,
) -> PyResult<Overlay> {
    let point = |p: (f32, f32)| [p.0, p.1];
    let mut parsed = Vec::with_capacity(items.len());
    for item in items {
        let kind: String = item.get_item(0)?.extract()?;
        parsed.push(match kind.as_str() {
            "stroke" => {
                let (_, points, rgba, width, dashed, closed, crisp): StrokeTuple =
                    item.extract()?;
                OverlayItem::Stroke {
                    points: points.into_iter().map(point).collect(),
                    rgba: [rgba.0, rgba.1, rgba.2, rgba.3],
                    width,
                    dashed,
                    closed,
                    crisp,
                }
            }
            "fill" => {
                let (_, min, max, rgba): FillTuple = item.extract()?;
                OverlayItem::Fill {
                    min: point(min),
                    max: point(max),
                    rgba: [rgba.0, rgba.1, rgba.2, rgba.3],
                }
            }
            other => {
                return Err(PyValueError::new_err(format!(
                    "unknown overlay item {other:?}"
                )));
            }
        });
    }
    let mut out = Vec::with_capacity(labels.len());
    for (x, y, width, height, pixels) in labels {
        if pixels.len() != width as usize * height as usize * 4 {
            return Err(PyValueError::new_err(format!(
                "a {width}x{height} label needs {} bytes, not {}",
                width as usize * height as usize * 4,
                pixels.len()
            )));
        }
        out.push(OverlayLabel {
            at: [x, y],
            width,
            height,
            pixels,
        });
    }
    Ok(Overlay {
        items: parsed,
        labels: out,
        ratio,
    })
}

/// Adds the class to the module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<CadView>()
}
