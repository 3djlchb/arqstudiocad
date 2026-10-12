//! Immutable value types (ADR 0012 D4.1, D4.2): points and vectors as
//! tuples, `CadColor`, `CadBounds2` and one class per curve.
//!
//! Every class is a frozen copy: changing the drawing never changes a value
//! already handed to Python.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use arq_core::Color;
use arq_geometry::{Arc, Bounds2, Circle, Line, Point2};
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;

use crate::errors::{IntoPy, InvalidOperationError};

/// A point as Python passes it: any sequence of two numbers — a tuple, a
/// list, a NumPy array of shape `(2,)`.
pub struct PyPoint(pub Point2);

/// The two numbers of a point-like Python object.
fn pair(obj: &Bound<'_, PyAny>, what: &str) -> PyResult<(f64, f64)> {
    let shape_error = || PyTypeError::new_err(format!("a {what} is two numbers (x, y)"));
    // Text and bytes are sequences too: b"ab" would read as (97, 98).
    let text = obj.is_instance_of::<pyo3::types::PyString>()
        || obj.is_instance_of::<pyo3::types::PyBytes>()
        || obj.is_instance_of::<pyo3::types::PyByteArray>();
    if text || obj.len().ok() != Some(2) {
        return Err(shape_error());
    }
    let x = obj
        .get_item(0)?
        .extract::<f64>()
        .map_err(|_| shape_error())?;
    let y = obj
        .get_item(1)?
        .extract::<f64>()
        .map_err(|_| shape_error())?;
    Ok((x, y))
}

impl<'a, 'py> FromPyObject<'a, 'py> for PyPoint {
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        let (x, y) = pair(&obj, "point")?;
        Point2::new(x, y).py().map(Self)
    }
}

/// A point as Python receives it.
pub fn xy(p: Point2) -> (f64, f64) {
    (p.x(), p.y())
}

/// Hash of floats consistent with `==`: `-0.0` and `0.0` hash alike.
fn hash_floats(values: &[f64]) -> u64 {
    let mut h = DefaultHasher::new();
    for v in values {
        (v + 0.0).to_bits().hash(&mut h);
    }
    h.finish()
}

fn fmt_xy(p: Point2) -> String {
    format!("({}, {})", fmt_f64(p.x()), fmt_f64(p.y()))
}

/// A float written as Python writes it for the common cases (`1.0`, `0.5`).
fn fmt_f64(v: f64) -> String {
    format!("{v:?}")
}

/// An entity or layer color (ADR 0012 D4.2). Build it with the named
/// constructors; `kind` says which one it is.
#[pyclass(frozen, eq, hash, from_py_object, module = "arqstudiocad._core")]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct CadColor(pub Color);

#[pymethods]
impl CadColor {
    /// The color of the entity's layer.
    #[staticmethod]
    fn by_layer() -> Self {
        Self(Color::ByLayer)
    }

    /// The color of the block the entity is inserted in.
    #[staticmethod]
    fn by_block() -> Self {
        Self(Color::ByBlock)
    }

    /// An AutoCAD Color Index, 1 to 255.
    #[staticmethod]
    fn from_index(index: i64) -> PyResult<Self> {
        match u8::try_from(index) {
            Ok(i) if i >= 1 => Ok(Self(Color::Index(i))),
            _ => Err(InvalidOperationError::new_err(format!(
                "color index {index} is not 1 to 255 (0 means ByBlock)"
            ))),
        }
    }

    /// A true color.
    #[staticmethod]
    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self(Color::Rgb { r, g, b })
    }

    /// `"bylayer"`, `"byblock"`, `"index"`, `"rgb"` or `"none"`.
    #[getter]
    fn kind(&self) -> &'static str {
        match self.0 {
            Color::ByLayer => "bylayer",
            Color::ByBlock => "byblock",
            Color::Index(_) => "index",
            Color::Rgb { .. } => "rgb",
            Color::None => "none",
        }
    }

    /// The ACI index, for an index color.
    #[getter]
    fn index(&self) -> Option<u8> {
        match self.0 {
            Color::Index(i) => Some(i),
            _ => None,
        }
    }

    /// `(r, g, b)`, for a true color.
    #[getter]
    fn rgb(&self) -> Option<(u8, u8, u8)> {
        match self.0 {
            Color::Rgb { r, g, b } => Some((r, g, b)),
            _ => None,
        }
    }

    fn __repr__(&self) -> String {
        match self.0 {
            Color::ByLayer => "CadColor.by_layer()".to_owned(),
            Color::ByBlock => "CadColor.by_block()".to_owned(),
            Color::Index(i) => format!("CadColor.from_index({i})"),
            Color::Rgb { r, g, b } => format!("CadColor.from_rgb({r}, {g}, {b})"),
            Color::None => "CadColor(kind='none')".to_owned(),
        }
    }
}

/// An axis-aligned box: `min` is the lower-left corner, `max` the upper-right.
#[pyclass(frozen, eq, from_py_object, module = "arqstudiocad._core")]
#[derive(Clone, Copy, PartialEq)]
pub struct CadBounds2(pub Bounds2);

#[pymethods]
impl CadBounds2 {
    /// The box with corners `a` and `b`, in any order.
    #[new]
    fn new(a: PyPoint, b: PyPoint) -> Self {
        Self(Bounds2::new(a.0, b.0))
    }

    #[classattr]
    fn __match_args__() -> (&'static str, &'static str) {
        ("min", "max")
    }

    /// Lower-left corner.
    #[getter]
    fn min(&self) -> (f64, f64) {
        xy(self.0.min())
    }

    /// Upper-right corner.
    #[getter]
    fn max(&self) -> (f64, f64) {
        xy(self.0.max())
    }

    /// Size along X.
    #[getter]
    fn width(&self) -> f64 {
        self.0.width()
    }

    /// Size along Y.
    #[getter]
    fn height(&self) -> f64 {
        self.0.height()
    }

    /// Middle of the box.
    #[getter]
    fn center(&self) -> (f64, f64) {
        xy(self.0.center())
    }

    fn __hash__(&self) -> u64 {
        let (a, b) = (self.0.min(), self.0.max());
        hash_floats(&[a.x(), a.y(), b.x(), b.y()])
    }

    fn __repr__(&self) -> String {
        format!(
            "CadBounds2(min={}, max={})",
            fmt_xy(self.0.min()),
            fmt_xy(self.0.max())
        )
    }
}

/// A straight segment.
#[pyclass(frozen, eq, from_py_object, module = "arqstudiocad._core")]
#[derive(Clone, Copy, PartialEq)]
pub struct CadLine(pub Line);

#[pymethods]
impl CadLine {
    /// A segment from `start` to `end`; they must differ.
    #[new]
    fn new(start: PyPoint, end: PyPoint) -> PyResult<Self> {
        Line::new(start.0, end.0).py().map(Self)
    }

    #[classattr]
    fn __match_args__() -> (&'static str, &'static str) {
        ("start", "end")
    }

    /// First point.
    #[getter]
    fn start(&self) -> (f64, f64) {
        xy(self.0.start())
    }

    /// Last point.
    #[getter]
    fn end(&self) -> (f64, f64) {
        xy(self.0.end())
    }

    /// Length.
    #[getter]
    fn length(&self) -> f64 {
        self.0.length()
    }

    fn __hash__(&self) -> u64 {
        let (a, b) = (self.0.start(), self.0.end());
        hash_floats(&[a.x(), a.y(), b.x(), b.y()])
    }

    fn __repr__(&self) -> String {
        format!(
            "CadLine(start={}, end={})",
            fmt_xy(self.0.start()),
            fmt_xy(self.0.end())
        )
    }
}

/// A full circle.
#[pyclass(frozen, eq, from_py_object, module = "arqstudiocad._core")]
#[derive(Clone, Copy, PartialEq)]
pub struct CadCircle(pub Circle);

#[pymethods]
impl CadCircle {
    /// A circle around `center`; the radius must be positive.
    #[new]
    fn new(center: PyPoint, radius: f64) -> PyResult<Self> {
        Circle::new(center.0, radius).py().map(Self)
    }

    #[classattr]
    fn __match_args__() -> (&'static str, &'static str) {
        ("center", "radius")
    }

    /// Center.
    #[getter]
    fn center(&self) -> (f64, f64) {
        xy(self.0.center())
    }

    /// Radius.
    #[getter]
    fn radius(&self) -> f64 {
        self.0.radius()
    }

    /// Circumference.
    #[getter]
    fn length(&self) -> f64 {
        self.0.length()
    }

    fn __hash__(&self) -> u64 {
        let c = self.0.center();
        hash_floats(&[c.x(), c.y(), self.0.radius()])
    }

    fn __repr__(&self) -> String {
        format!(
            "CadCircle(center={}, radius={})",
            fmt_xy(self.0.center()),
            fmt_f64(self.0.radius())
        )
    }
}

/// A circular arc, counterclockwise from `start_angle` to `end_angle`
/// (radians, normalized to `[0, 2π)`).
#[pyclass(frozen, eq, from_py_object, module = "arqstudiocad._core")]
#[derive(Clone, Copy, PartialEq)]
pub struct CadArc(pub Arc);

#[pymethods]
impl CadArc {
    /// An arc around `center`, counterclockwise from `start_angle` to
    /// `end_angle` (radians).
    #[new]
    fn new(center: PyPoint, radius: f64, start_angle: f64, end_angle: f64) -> PyResult<Self> {
        Arc::new(center.0, radius, start_angle, end_angle)
            .py()
            .map(Self)
    }

    #[classattr]
    fn __match_args__() -> (&'static str, &'static str, &'static str, &'static str) {
        ("center", "radius", "start_angle", "end_angle")
    }

    /// Center.
    #[getter]
    fn center(&self) -> (f64, f64) {
        xy(self.0.center())
    }

    /// Radius.
    #[getter]
    fn radius(&self) -> f64 {
        self.0.radius()
    }

    /// Start angle, radians.
    #[getter]
    fn start_angle(&self) -> f64 {
        self.0.start_angle()
    }

    /// End angle, radians.
    #[getter]
    fn end_angle(&self) -> f64 {
        self.0.end_angle()
    }

    /// The point at `start_angle`.
    #[getter]
    fn start_point(&self) -> (f64, f64) {
        xy(self.0.start_point())
    }

    /// The point at `end_angle`.
    #[getter]
    fn end_point(&self) -> (f64, f64) {
        xy(self.0.end_point())
    }

    /// Length along the arc.
    #[getter]
    fn length(&self) -> f64 {
        self.0.length()
    }

    fn __hash__(&self) -> u64 {
        let c = self.0.center();
        hash_floats(&[
            c.x(),
            c.y(),
            self.0.radius(),
            self.0.start_angle(),
            self.0.end_angle(),
        ])
    }

    fn __repr__(&self) -> String {
        format!(
            "CadArc(center={}, radius={}, start_angle={}, end_angle={})",
            fmt_xy(self.0.center()),
            fmt_f64(self.0.radius()),
            fmt_f64(self.0.start_angle()),
            fmt_f64(self.0.end_angle())
        )
    }
}

/// Adds the value classes to the module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<CadColor>()?;
    m.add_class::<CadBounds2>()?;
    m.add_class::<CadLine>()?;
    m.add_class::<CadCircle>()?;
    m.add_class::<CadArc>()?;
    Ok(())
}
