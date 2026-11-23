//! Immutable value types (ADR 0012 D4.1, D4.2): points and vectors as
//! tuples, `CadColor`, `CadBounds2` and one class per curve.
//!
//! Every class is a frozen copy: changing the drawing never changes a value
//! already handed to Python.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use arq_core::{Color, Grip, SnapKind, SnapPoint};
use arq_geometry::{
    Arc, Bounds2, Circle, Corners, Ellipse, Geometry2, Line, Point2, Rectangle, Spline, SplineFit,
    SplineKnots, Tolerance, Vector2,
};
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;

use crate::errors::{IntoPy, InvalidOperationError};

/// A point as Python passes it: any sequence of two numbers — a tuple, a
/// list, a NumPy array of shape `(2,)`.
pub struct PyPoint(pub Point2);

/// A vector (a displacement) as Python passes it, like [`PyPoint`].
pub struct PyVector(pub Vector2);

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

impl<'a, 'py> FromPyObject<'a, 'py> for PyVector {
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        let (x, y) = pair(&obj, "vector")?;
        Vector2::new(x, y).py().map(Self)
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

    /// The `(r, g, b)` it is drawn with, the viewport's own ACI table:
    /// index colors through it, ACI 7 white or black by `background`,
    /// by-layer and by-block as ACI 7 (their real color is the layer's or
    /// the block's).
    #[pyo3(signature = (background = (33, 40, 48)))]
    fn display_rgb(&self, background: (u8, u8, u8)) -> (u8, u8, u8) {
        let bg = [background.0, background.1, background.2];
        let [r, g, b] = match self.0 {
            Color::Rgb { r, g, b } => [r, g, b],
            Color::Index(i) => arq_render::aci_rgb_on(i, bg),
            _ => arq_render::aci_rgb_on(7, bg),
        };
        (r, g, b)
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

    /// The direction from start to end, in radians from East.
    #[getter]
    fn direction(&self) -> f64 {
        arq_geometry::angle::normalize(self.0.direction().angle())
    }

    /// The line `distance` away on the side `toward` lies
    /// (OFFSET); refused with no side, a distance not greater than zero, or
    /// nothing left.
    fn offset(&self, distance: f64, toward: PyPoint) -> PyResult<Self> {
        match Geometry2::Line(self.0).offset(distance, toward.0, Tolerance::DEFAULT) {
            Ok(Geometry2::Line(g)) => Ok(Self(g)),
            Ok(_) => unreachable!("an offset keeps its kind"),
            Err(e) => Err(e).py(),
        }
    }

    /// The line offset to pass through `through` (OFFSET's
    /// Through), the distance taken square to its line or circle.
    fn offset_through(&self, through: PyPoint) -> PyResult<Self> {
        match Geometry2::Line(self.0).offset_through(through.0, Tolerance::DEFAULT) {
            Ok(Geometry2::Line(g)) => Ok(Self(g)),
            Ok(_) => unreachable!("an offset keeps its kind"),
            Err(e) => Err(e).py(),
        }
    }

    /// CHAMFER's Angle method: how far along `second` the chamfer ends when
    /// it starts `length` along `first` from where they meet and leaves it
    /// at `angle` (radians); each line taken on the side picked (`first_at`,
    /// `second_at`).
    #[staticmethod]
    fn chamfer_distance(
        first: CadLine,
        first_at: PyPoint,
        second: CadLine,
        second_at: PyPoint,
        length: f64,
        angle: f64,
    ) -> PyResult<f64> {
        arq_geometry::chamfer_second_distance(
            first.0,
            first_at.0,
            second.0,
            second_at.0,
            length,
            angle,
        )
        .py()
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

/// An ellipse or elliptical arc in the plan (ELLIPSE, ADR 0040): its
/// center, major semi-axis, minor to major ratio and parameters, as DWG
/// keeps it.
#[pyclass(frozen, eq, from_py_object, module = "arqstudiocad._core")]
#[derive(Clone, Copy, PartialEq)]
pub struct CadEllipse(pub Ellipse);

#[pymethods]
impl CadEllipse {
    /// The ellipse about `center` with major semi-axis `major_axis` (a
    /// vector), `ratio` in (0, 1], counterclockwise from parameter `start`
    /// to `end`; equal ones make the whole ellipse.
    #[new]
    #[pyo3(signature = (center, major_axis, ratio, start = 0.0, end = 0.0))]
    fn new(
        center: PyPoint,
        major_axis: PyVector,
        ratio: f64,
        start: f64,
        end: f64,
    ) -> PyResult<Self> {
        Ellipse::new(center.0, major_axis.0, ratio, start, end)
            .py()
            .map(Self)
    }

    /// The whole ellipse about `center` through `axis_end`, its other
    /// semi-axis `other` long; the longer axis is the major one.
    #[staticmethod]
    fn from_axis(center: PyPoint, axis_end: PyPoint, other: f64) -> PyResult<Self> {
        Ellipse::from_axis(center.0, axis_end.0, other)
            .py()
            .map(Self)
    }

    /// The whole ellipse about `center` through `axis_end`, a circle seen
    /// turned `rotation` radians about that axis (ELLIPSE's Rotation, up
    /// to 89.4 degrees).
    #[staticmethod]
    fn from_rotation(center: PyPoint, axis_end: PyPoint, rotation: f64) -> PyResult<Self> {
        Ellipse::from_rotation(center.0, axis_end.0, rotation)
            .py()
            .map(Self)
    }

    /// This whole ellipse cut to the arc from `start` to `end` (radians),
    /// measured counterclockwise from the first axis drawn, toward
    /// `first_axis_end`: as angles about the center, or as the ellipse's
    /// own parameters when `parameters` is true (ELLIPSE's Arc).
    #[pyo3(signature = (first_axis_end, start, end, parameters = false))]
    fn arc(
        &self,
        first_axis_end: PyPoint,
        start: f64,
        end: f64,
        parameters: bool,
    ) -> PyResult<Self> {
        let first = first_axis_end.0 - self.0.center();
        self.0
            .arc_from(first, start, end, parameters)
            .py()
            .map(Self)
    }

    /// Center.
    #[getter]
    fn center(&self) -> (f64, f64) {
        xy(self.0.center())
    }

    /// The major semi-axis, from the center, as a vector.
    #[getter]
    fn major_axis(&self) -> (f64, f64) {
        let v = self.0.major_axis();
        (v.x(), v.y())
    }

    /// Minor to major axis ratio.
    #[getter]
    fn ratio(&self) -> f64 {
        self.0.ratio()
    }

    /// Start parameter, radians in [0, 2π).
    #[getter]
    fn start_param(&self) -> f64 {
        self.0.start_param()
    }

    /// End parameter, radians in [0, 2π).
    #[getter]
    fn end_param(&self) -> f64 {
        self.0.end_param()
    }

    /// Whether it is the whole ellipse.
    #[getter]
    fn is_full(&self) -> bool {
        self.0.is_full()
    }

    /// Its length along the curve.
    #[getter]
    fn length(&self) -> f64 {
        self.0.length()
    }

    /// This ellipse with one semi-axis, the major one when `major`, set to
    /// `length` (a grip stretched): its direction and the other kept; the
    /// longer becomes the major, an arc keeping its place.
    fn with_semi_axis(&self, major: bool, length: f64) -> PyResult<Self> {
        self.0.with_semi_axis(major, length).py().map(Self)
    }

    /// This arc with its start (when `start`) or its end moved to the point
    /// of the ellipse toward `point`, seen from the center (a grip
    /// stretched); refused for the whole ellipse.
    fn with_end_at(&self, start: bool, point: PyPoint) -> PyResult<Self> {
        self.0.with_end_at(start, point.0).py().map(Self)
    }

    /// The points it is drawn by, its ends included.
    fn points(&self) -> Vec<(f64, f64)> {
        self.0.flatten().into_iter().map(xy).collect()
    }

    fn __repr__(&self) -> String {
        let (c, m) = (self.0.center(), self.0.major_axis());
        format!(
            "CadEllipse(({}, {}), ({}, {}), {}, {}, {})",
            c.x(),
            c.y(),
            m.x(),
            m.y(),
            self.0.ratio(),
            self.0.start_param(),
            self.0.end_param()
        )
    }
}

/// A spline in the plan that SPLINE makes (ADR 0042): through fit points,
/// or by control vertices.
#[pyclass(frozen, from_py_object, module = "arqstudiocad._core")]
#[derive(Clone)]
pub struct CadSpline {
    pub spline: Spline,
    /// What it was fitted through, when it was.
    pub fit: Option<SplineFit>,
}

/// The knot spacing named `knots`: `"chord"`, `"square_root"` or
/// `"uniform"`.
fn spline_knots(knots: &str) -> PyResult<SplineKnots> {
    match knots {
        "chord" => Ok(SplineKnots::Chord),
        "square_root" => Ok(SplineKnots::SquareRoot),
        "uniform" => Ok(SplineKnots::Uniform),
        _ => Err(PyValueError::new_err(
            "knots must be \"chord\", \"square_root\" or \"uniform\"",
        )),
    }
}

#[pymethods]
impl CadSpline {
    /// The cubic spline through `points` (SPLINE's Fit method): its knots
    /// spaced by `knots` (`"chord"`, `"square_root"` or `"uniform"`), along
    /// `start_tangent` and `end_tangent` (directions) when given, periodic
    /// when `closed`, which takes no tangent.
    #[staticmethod]
    #[pyo3(signature = (points, knots = "chord", start_tangent = None, end_tangent = None, closed = false))]
    fn through(
        points: Vec<PyPoint>,
        knots: &str,
        start_tangent: Option<PyVector>,
        end_tangent: Option<PyVector>,
        closed: bool,
    ) -> PyResult<Self> {
        let fit = SplineFit::new(
            points.into_iter().map(|p| p.0).collect(),
            spline_knots(knots)?,
            start_tangent.map(|v| v.0),
            end_tangent.map(|v| v.0),
            closed,
        )
        .py()?;
        Ok(Self {
            spline: fit.spline().py()?,
            fit: Some(fit),
        })
    }

    /// The spline of `degree` by control vertices `points` (SPLINE's CV
    /// method): its degree at most one fewer than the points, periodic when
    /// `closed`.
    #[staticmethod]
    #[pyo3(signature = (points, degree = 3, closed = false))]
    fn by_vertices(points: Vec<PyPoint>, degree: usize, closed: bool) -> PyResult<Self> {
        let spline =
            Spline::by_vertices(degree, points.into_iter().map(|p| p.0).collect(), closed).py()?;
        Ok(Self { spline, fit: None })
    }

    /// Its degree.
    #[getter]
    fn degree(&self) -> usize {
        self.spline.degree()
    }

    /// Its knots.
    #[getter]
    fn knots(&self) -> Vec<f64> {
        self.spline.knots().to_vec()
    }

    /// Its control points, the first ones repeated at the end when closed.
    #[getter]
    fn control_points(&self) -> Vec<(f64, f64)> {
        self.spline.points().iter().copied().map(xy).collect()
    }

    /// The points it was fitted through; empty when made by its vertices.
    #[getter]
    fn fit_points(&self) -> Vec<(f64, f64)> {
        self.fit
            .as_ref()
            .map_or_else(Vec::new, |f| f.points().iter().copied().map(xy).collect())
    }

    /// Whether it closes.
    #[getter]
    fn is_closed(&self) -> bool {
        self.spline.is_closed()
    }

    /// Its length along the curve.
    #[getter]
    fn length(&self) -> f64 {
        self.spline.length()
    }

    /// Its own control vertices: those a closed one repeats at its end left
    /// out, as its grips show them.
    #[getter]
    fn vertices(&self) -> Vec<(f64, f64)> {
        self.spline.own_vertices().iter().copied().map(xy).collect()
    }

    /// How its fit points' knots are spaced, `"chord"`, `"square_root"` or
    /// `"uniform"`; None when made by its vertices.
    #[getter]
    fn knots_method(&self) -> Option<&'static str> {
        self.fit.as_ref().map(|f| match f.knots() {
            SplineKnots::Chord => "chord",
            SplineKnots::SquareRoot => "square_root",
            SplineKnots::Uniform => "uniform",
        })
    }

    /// The points it is drawn by, its ends included.
    fn points(&self) -> Vec<(f64, f64)> {
        self.spline.flatten().iter().copied().map(xy).collect()
    }

    fn __repr__(&self) -> String {
        let how = if self.fit.is_some() {
            "fit"
        } else {
            "vertices"
        };
        format!(
            "CadSpline({how}, degree {}, {} control points{})",
            self.spline.degree(),
            self.spline.points().len(),
            if self.spline.is_closed() {
                ", closed"
            } else {
                ""
            }
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

    /// The circle with `a` and `b` at the ends of a diameter.
    #[staticmethod]
    fn from_diameter(a: PyPoint, b: PyPoint) -> PyResult<Self> {
        Circle::from_diameter(a.0, b.0).py().map(Self)
    }

    /// The circle through three points; they must not lie on one line.
    #[staticmethod]
    fn through(a: PyPoint, b: PyPoint, c: PyPoint) -> PyResult<Self> {
        Circle::from_three_points(a.0, b.0, c.0, Tolerance::DEFAULT)
            .py()
            .map(Self)
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

    /// The circle `distance` away on the side `toward` lies
    /// (OFFSET); refused with no side, a distance not greater than zero, or
    /// nothing left.
    fn offset(&self, distance: f64, toward: PyPoint) -> PyResult<Self> {
        match Geometry2::Circle(self.0).offset(distance, toward.0, Tolerance::DEFAULT) {
            Ok(Geometry2::Circle(g)) => Ok(Self(g)),
            Ok(_) => unreachable!("an offset keeps its kind"),
            Err(e) => Err(e).py(),
        }
    }

    /// The circle offset to pass through `through` (OFFSET's
    /// Through), the distance taken square to its line or circle.
    fn offset_through(&self, through: PyPoint) -> PyResult<Self> {
        match Geometry2::Circle(self.0).offset_through(through.0, Tolerance::DEFAULT) {
            Ok(Geometry2::Circle(g)) => Ok(Self(g)),
            Ok(_) => unreachable!("an offset keeps its kind"),
            Err(e) => Err(e).py(),
        }
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

    /// The arc that passes through `a`, then `b`, then `c`; they must not
    /// lie on one line.
    #[staticmethod]
    fn through(a: PyPoint, b: PyPoint, c: PyPoint) -> PyResult<Self> {
        Arc::from_three_points(a.0, b.0, c.0, Tolerance::DEFAULT)
            .py()
            .map(Self)
    }

    /// The arc around `center` from `start`, counterclockwise to the
    /// direction of `end`; the radius is the distance to `start`.
    #[staticmethod]
    fn from_start_center_end(start: PyPoint, center: PyPoint, end: PyPoint) -> PyResult<Self> {
        Arc::from_start_center_end(start.0, center.0, end.0)
            .py()
            .map(Self)
    }

    /// The arc leaving `start` along `direction` (radians) to `end`, as a
    /// polyline vertex stores it: `(bulge, end_direction)`.
    #[staticmethod]
    fn tangent_bulge(start: PyPoint, direction: f64, end: PyPoint) -> PyResult<(f64, f64)> {
        Arc::tangent_bulge(start.0, direction, end.0).py()
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

    /// The arc `distance` away on the side `toward` lies
    /// (OFFSET); refused with no side, a distance not greater than zero, or
    /// nothing left.
    fn offset(&self, distance: f64, toward: PyPoint) -> PyResult<Self> {
        match Geometry2::Arc(self.0).offset(distance, toward.0, Tolerance::DEFAULT) {
            Ok(Geometry2::Arc(g)) => Ok(Self(g)),
            Ok(_) => unreachable!("an offset keeps its kind"),
            Err(e) => Err(e).py(),
        }
    }

    /// The arc offset to pass through `through` (OFFSET's
    /// Through), the distance taken square to its line or circle.
    fn offset_through(&self, through: PyPoint) -> PyResult<Self> {
        match Geometry2::Arc(self.0).offset_through(through.0, Tolerance::DEFAULT) {
            Ok(Geometry2::Arc(g)) => Ok(Self(g)),
            Ok(_) => unreachable!("an offset keeps its kind"),
            Err(e) => Err(e).py(),
        }
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

/// A rectangle as RECTANG draws it: from a first corner to the opposite
/// one, its sides along a rotation (radians) and square to it.
#[pyclass(frozen, eq, from_py_object, module = "arqstudiocad._core")]
#[derive(Clone, Copy, PartialEq)]
pub struct CadRectangle(pub Rectangle);

#[pymethods]
impl CadRectangle {
    /// The rectangle from `first` to the opposite corner `other`, its sides
    /// along `rotation`; `other` must not lie on a side's line through
    /// `first`.
    #[new]
    #[pyo3(signature = (first, other, rotation = 0.0))]
    fn new(first: PyPoint, other: PyPoint, rotation: f64) -> PyResult<Self> {
        Rectangle::from_corners(first.0, other.0, rotation, Tolerance::DEFAULT)
            .py()
            .map(Self)
    }

    /// The rectangle `length` along `rotation` and `width` square to it,
    /// from `first` into the quarter `toward` lies in (RECTANG's
    /// Dimensions).
    #[staticmethod]
    fn sized(
        first: PyPoint,
        length: f64,
        width: f64,
        rotation: f64,
        toward: PyPoint,
    ) -> PyResult<Self> {
        Rectangle::from_size(
            first.0,
            length,
            width,
            rotation,
            toward.0,
            Tolerance::DEFAULT,
        )
        .py()
        .map(Self)
    }

    /// The rectangle of `area` from `first`, the positive way along
    /// `rotation` and square to it (RECTANG's Area): `side` is its length
    /// when `side_is_length`, else its width. The area is the one the
    /// corners leave, `fillet` or `chamfer` as in `path`.
    #[staticmethod]
    #[pyo3(
        signature = (first, area, side, side_is_length, rotation, fillet = 0.0, chamfer = (0.0, 0.0)),
        text_signature = "(first, area, side, side_is_length, rotation, fillet=0.0, chamfer=(0.0, 0.0))"
    )]
    fn of_area(
        first: PyPoint,
        area: f64,
        side: f64,
        side_is_length: bool,
        rotation: f64,
        fillet: f64,
        chamfer: (f64, f64),
    ) -> PyResult<Self> {
        Rectangle::from_area(
            first.0,
            area,
            side,
            side_is_length,
            rotation,
            corners(fillet, chamfer)?,
            Tolerance::DEFAULT,
        )
        .py()
        .map(Self)
    }

    #[classattr]
    fn __match_args__() -> (&'static str, &'static str, &'static str) {
        ("first", "other", "rotation")
    }

    /// The first corner.
    #[getter]
    fn first(&self) -> (f64, f64) {
        xy(self.0.first())
    }

    /// The corner opposite the first.
    #[getter]
    fn other(&self) -> (f64, f64) {
        xy(self.0.other())
    }

    /// The direction of the length, radians from East.
    #[getter]
    fn rotation(&self) -> f64 {
        self.0.rotation()
    }

    /// The size along the rotation.
    #[getter]
    fn length(&self) -> f64 {
        self.0.length()
    }

    /// The size square to the rotation.
    #[getter]
    fn width(&self) -> f64 {
        self.0.width()
    }

    /// Its area.
    #[getter]
    fn area(&self) -> f64 {
        self.0.area()
    }

    /// The four corners, counterclockwise from the first.
    #[getter]
    fn corners(&self) -> Vec<(f64, f64)> {
        self.0.corners().map(xy).to_vec()
    }

    /// The corners as `CadDocument.add_polyline` takes them, each with the
    /// bulge of the straight side leaving it.
    #[getter]
    fn vertices(&self) -> Vec<((f64, f64), f64)> {
        self.0.corners().map(|p| (xy(p), 0.0)).to_vec()
    }

    /// The closed polyline with its corners rounded by `fillet` or cut by
    /// `chamfer` (the first distance along the length, the second along the
    /// width), as `CadDocument.add_polyline` takes it: counterclockwise,
    /// from where the first corner's fillet or chamfer ends. Corners too
    /// large for the sides are an `InvalidGeometryError`; a zero radius or
    /// distance leaves them square.
    #[pyo3(signature = (fillet = 0.0, chamfer = (0.0, 0.0)), text_signature = "(self, /, fillet=0.0, chamfer=(0.0, 0.0))")]
    fn path(&self, fillet: f64, chamfer: (f64, f64)) -> PyResult<Vec<((f64, f64), f64)>> {
        let vertices = self
            .0
            .vertices(corners(fillet, chamfer)?, Tolerance::DEFAULT)
            .py()?;
        Ok(vertices.into_iter().map(|(p, b)| (xy(p), b)).collect())
    }

    /// Whether `fillet` or `chamfer` fit its sides.
    #[pyo3(signature = (fillet = 0.0, chamfer = (0.0, 0.0)), text_signature = "(self, /, fillet=0.0, chamfer=(0.0, 0.0))")]
    fn fits(&self, fillet: f64, chamfer: (f64, f64)) -> PyResult<bool> {
        let corners = corners(fillet, chamfer)?.checked().py()?;
        Ok(self.0.fits(corners, Tolerance::DEFAULT))
    }

    /// Its area with `fillet` or `chamfer`: what the corners leave.
    #[pyo3(signature = (fillet = 0.0, chamfer = (0.0, 0.0)), text_signature = "(self, /, fillet=0.0, chamfer=(0.0, 0.0))")]
    fn area_with(&self, fillet: f64, chamfer: (f64, f64)) -> PyResult<f64> {
        let corners = corners(fillet, chamfer)?.checked().py()?;
        Ok(self.0.area_with(corners))
    }

    fn __hash__(&self) -> u64 {
        let (a, b) = (self.0.first(), self.0.other());
        hash_floats(&[a.x(), a.y(), b.x(), b.y(), self.0.rotation()])
    }

    fn __repr__(&self) -> String {
        format!(
            "CadRectangle(first={}, other={}, rotation={})",
            fmt_xy(self.0.first()),
            fmt_xy(self.0.other()),
            fmt_f64(self.0.rotation())
        )
    }
}

/// RECTANG's corners from Python's two settings: a fillet or a chamfer,
/// never both (AutoCAD keeps one of them at a time).
fn corners(fillet: f64, chamfer: (f64, f64)) -> PyResult<Corners> {
    let chamfered = chamfer != (0.0, 0.0);
    if fillet != 0.0 && chamfered {
        return Err(PyValueError::new_err(
            "a rectangle's corners are filleted or chamfered, not both",
        ));
    }
    Ok(if chamfered {
        Corners::Chamfer(chamfer.0, chamfer.1)
    } else {
        Corners::Fillet(fillet)
    })
}

/// Adds the value classes to the module.
/// Every object snap, by AutoCAD's key.
const SNAP_KINDS: [SnapKind; 8] = [
    SnapKind::Endpoint,
    SnapKind::Midpoint,
    SnapKind::Center,
    SnapKind::Quadrant,
    SnapKind::Intersection,
    SnapKind::Perpendicular,
    SnapKind::Tangent,
    SnapKind::Nearest,
];

/// The snap kinds named by AutoCAD's keys (`"END"`, `"mid"`…, any case);
/// an unknown key is a `ValueError`.
pub fn snap_kinds(keys: &[String]) -> PyResult<Vec<SnapKind>> {
    keys.iter()
        .map(|key| {
            SNAP_KINDS
                .into_iter()
                .find(|k| k.name().eq_ignore_ascii_case(key))
                .ok_or_else(|| {
                    PyValueError::new_err(format!(
                        "unknown object snap {key:?}: use END, MID, CEN, QUA, INT, PER, TAN or NEA"
                    ))
                })
        })
        .collect()
}

/// An object snap found under the cursor (ADR 0016 D5): its kind, by
/// AutoCAD's key (`"END"`…), and its point.
#[pyclass(frozen, eq, skip_from_py_object, module = "arqstudiocad._core")]
#[derive(Clone, Copy, PartialEq)]
pub struct CadSnap(pub SnapPoint);

#[pymethods]
impl CadSnap {
    #[classattr]
    fn __match_args__() -> (&'static str, &'static str) {
        ("kind", "point")
    }

    /// The snap's AutoCAD key: END, MID, CEN, QUA, INT, PER, TAN or NEA.
    #[getter]
    fn kind(&self) -> &'static str {
        self.0.kind.name()
    }

    /// Where it snaps.
    #[getter]
    fn point(&self) -> (f64, f64) {
        xy(self.0.point)
    }

    fn __hash__(&self) -> u64 {
        let mut h = DefaultHasher::new();
        self.0.kind.hash(&mut h);
        hash_floats(&[self.0.point.x(), self.0.point.y()]).hash(&mut h);
        h.finish()
    }

    fn __repr__(&self) -> String {
        format!(
            "CadSnap(kind={:?}, point={})",
            self.0.kind.name(),
            fmt_xy(self.0.point)
        )
    }
}

/// A grip of a selected entity (ADR 0016 D7): its entity, its key
/// (`"END"`, `"MID"`, `"CEN"`, `"QUA"`, `"INS"` for a block's insertion
/// point, `"VTX"` for a polyline vertex), its point and vertex index.
#[pyclass(frozen, eq, skip_from_py_object, module = "arqstudiocad._core")]
#[derive(Clone, Copy, PartialEq)]
pub struct CadGrip(pub Grip);

#[pymethods]
impl CadGrip {
    #[classattr]
    fn __match_args__() -> (&'static str, &'static str, &'static str) {
        ("entity_id", "kind", "point")
    }

    /// The entity it edits.
    #[getter]
    fn entity_id(&self) -> u64 {
        self.0.entity
    }

    /// What it stands for, as a key.
    #[getter]
    fn kind(&self) -> &'static str {
        self.0.kind.name()
    }

    /// Where it is.
    #[getter]
    fn point(&self) -> (f64, f64) {
        xy(self.0.point)
    }

    /// Zero-based polyline vertex index, or `None` for other grips.
    #[getter]
    fn vertex_index(&self) -> Option<usize> {
        self.0.vertex_index
    }

    fn __hash__(&self) -> u64 {
        let mut h = DefaultHasher::new();
        self.0.entity.hash(&mut h);
        self.0.kind.hash(&mut h);
        self.0.vertex_index.hash(&mut h);
        hash_floats(&[self.0.point.x(), self.0.point.y()]).hash(&mut h);
        h.finish()
    }

    fn __repr__(&self) -> String {
        format!(
            "CadGrip(entity_id={}, kind={:?}, point={}, vertex_index={:?})",
            self.0.entity,
            self.0.kind.name(),
            fmt_xy(self.0.point),
            self.0.vertex_index
        )
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<CadSnap>()?;
    m.add_class::<CadGrip>()?;
    m.add_class::<CadColor>()?;
    m.add_class::<CadBounds2>()?;
    m.add_class::<CadLine>()?;
    m.add_class::<CadCircle>()?;
    m.add_class::<CadEllipse>()?;
    m.add_class::<CadSpline>()?;
    m.add_class::<CadArc>()?;
    m.add_class::<CadRectangle>()?;
    Ok(())
}
