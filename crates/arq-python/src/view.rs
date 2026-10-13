//! `CadView`: a document seen through a camera, rendered on the CPU into an
//! image Qt can show (ADR 0013, D5.3).
//!
//! The view keeps its own render scene (derived data, never the source of
//! truth) and camera behind one lock. Building the scene and rendering run
//! with the GIL released. Lock order: the view first, then the document,
//! never the other way, and neither needs the GIL while held.

use std::sync::{Mutex, MutexGuard, PoisonError};

use arq_render::{Camera, Scene, Style, render};
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
    /// A `width` × `height` view of `document`, showing all of it.
    #[new]
    fn new(py: Python<'_>, document: Py<CadDocument>, width: u32, height: u32) -> PyResult<Self> {
        let doc = document.get();
        let scene = py
            .detach(|| doc.read_core(|cad| Scene::build(cad.document())))
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
            let scene = doc.read_core(|cad| Scene::build(cad.document()))?;
            self.state().scene = scene;
            Ok::<_, arq_core::CadError>(())
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

    /// Pixel `(x, y)` to world coordinates.
    fn to_world(&self, x: f64, y: f64) -> (f64, f64) {
        self.state().camera.to_world(x, y)
    }

    /// World `(x, y)` to pixels.
    fn to_screen(&self, x: f64, y: f64) -> (f64, f64) {
        self.state().camera.to_screen(x, y)
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

/// Adds the class to the module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<CadView>()
}
