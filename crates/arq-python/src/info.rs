//! `CadEntityInfo`, `CadLayerInfo` and `CadBlockInfo`: immutable
//! descriptions of an entity, a layer and a block (contract §4, ADR 0012 D4.2). They are copies taken when the
//! query ran; later edits never change them.

use arq_core::{BlockInfo, Entity, Layer, TextData};
use arq_geometry::Geometry2;
use pyo3::prelude::*;
use pyo3::types::PyString;

use crate::values::{CadArc, CadBounds2, CadCircle, CadColor, CadLine};

/// One entity as it was when read.
#[pyclass(frozen, eq, skip_from_py_object, module = "arqstudiocad._core")]
#[derive(Clone, PartialEq)]
pub struct CadEntityInfo(pub Entity);

#[pymethods]
impl CadEntityInfo {
    /// Its id: the DWG handle, stable across save and open.
    #[getter]
    fn id(&self) -> u64 {
        self.0.id()
    }

    /// DXF type name: `"LINE"`, `"CIRCLE"`, `"ARC"`, `"TEXT"`, `"INSERT"`…
    #[getter]
    fn kind(&self) -> &'static str {
        self.0.kind()
    }

    /// Name of its layer.
    #[getter]
    fn layer(&self) -> &str {
        self.0.layer()
    }

    /// Its own color (often by layer).
    #[getter]
    fn color(&self) -> CadColor {
        CadColor(self.0.color())
    }

    /// Its entity linetype (`ByLayer` when inherited).
    #[getter]
    fn linetype(&self) -> &str {
        self.0.linetype()
    }

    /// Its lineweight in millimeters or a special value such as `ByLayer`.
    #[getter]
    fn lineweight(&self) -> &str {
        self.0.lineweight()
    }

    /// Read-only, type-specific details for blocks and polylines.
    #[getter]
    fn details(&self) -> Vec<(String, String)> {
        self.0.details().to_vec()
    }

    /// Plan-view bounding box, when the entity has a known extent.
    #[getter]
    fn bounds(&self) -> Option<CadBounds2> {
        self.0.bounds().map(CadBounds2)
    }

    /// Its curve, for the kinds editable in this version (LINE, CIRCLE, ARC
    /// drawn in plan); `None` for every other entity, which is kept as read.
    #[getter]
    fn geometry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.0
            .plan()
            .map(|plan| geometry_to_py(py, plan.geometry()))
            .transpose()
    }

    /// Its text, for a TEXT or MTEXT drawn in plan (ADR 0022); `None` for
    /// every other entity.
    #[getter]
    fn text(&self) -> Option<CadText> {
        self.0.text().cloned().map(CadText)
    }

    /// Height (Z) of its plan geometry; `None` when it has none.
    #[getter]
    fn elevation(&self) -> Option<f64> {
        self.0.plan().map(|plan| plan.elevation())
    }

    /// Whether its geometry can be edited in this version.
    #[getter]
    fn is_editable(&self) -> bool {
        self.0.is_editable()
    }

    /// Whether it can be moved, rotated, scaled, mirrored and copied as a
    /// whole: lines, circles, arcs, blocks and polylines drawn in plan.
    #[getter]
    fn is_transformable(&self) -> bool {
        self.0.is_transformable()
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let layer = PyString::new(py, self.0.layer()).repr()?;
        Ok(format!(
            "CadEntityInfo(id={}, kind='{}', layer={layer})",
            self.0.id(),
            self.0.kind()
        ))
    }
}

/// The Python object for a curve: `CadLine`, `CadCircle` or `CadArc`.
fn geometry_to_py(py: Python<'_>, g: Geometry2) -> PyResult<Bound<'_, PyAny>> {
    Ok(match g {
        Geometry2::Line(l) => Bound::new(py, CadLine(l))?.into_any(),
        Geometry2::Circle(c) => Bound::new(py, CadCircle(c))?.into_any(),
        Geometry2::Arc(a) => Bound::new(py, CadArc(a))?.into_any(),
    })
}

/// A TEXT or MTEXT as it was when read (ADR 0022).
#[pyclass(frozen, eq, skip_from_py_object, module = "arqstudiocad._core")]
#[derive(Clone, PartialEq)]
pub struct CadText(pub TextData);

#[pymethods]
impl CadText {
    /// What it says, as stored (MTEXT keeps its formatting codes).
    #[getter]
    fn content(&self) -> &str {
        self.0.content()
    }

    /// The point it hangs from: the insertion point of a left, baseline
    /// text, else its alignment point; an MTEXT's insertion point.
    #[getter]
    fn anchor(&self) -> (f64, f64) {
        (self.0.anchor().x(), self.0.anchor().y())
    }

    /// Character height.
    #[getter]
    fn height(&self) -> f64 {
        self.0.height()
    }

    /// Reading direction, radians counterclockwise from East.
    #[getter]
    fn rotation(&self) -> f64 {
        self.0.rotation()
    }

    /// Text style name.
    #[getter]
    fn style(&self) -> &str {
        self.0.style()
    }

    /// Justification as AutoCAD names it (`"Left"`, `"MiddleCenter"`…).
    #[getter]
    fn justification(&self) -> &'static str {
        self.0.justification()
    }

    /// Width factor.
    #[getter]
    fn width_factor(&self) -> f64 {
        self.0.width_factor()
    }

    /// Oblique angle, radians.
    #[getter]
    fn oblique(&self) -> f64 {
        self.0.oblique()
    }

    /// An MTEXT's wrapping width (0: no wrapping); 0 for a TEXT.
    #[getter]
    fn width(&self) -> f64 {
        self.0.width()
    }

    /// Whether it is an MTEXT.
    #[getter]
    fn is_multiline(&self) -> bool {
        self.0.is_multiline()
    }

    /// Whether its height and rotation can be set (not for Aligned or Fit).
    #[getter]
    fn sizes_editable(&self) -> bool {
        self.0.sizes_editable()
    }

    /// Whether its content can be set (not for an MTEXT with formatting).
    #[getter]
    fn content_editable(&self) -> bool {
        self.0.content_editable()
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let content = PyString::new(py, self.0.content()).repr()?;
        Ok(format!(
            "CadText(content={content}, justification='{}')",
            self.0.justification()
        ))
    }
}

/// One layer as it was when read.
#[pyclass(frozen, eq, skip_from_py_object, module = "arqstudiocad._core")]
#[derive(Clone, PartialEq)]
pub struct CadLayerInfo(pub Layer);

#[pymethods]
impl CadLayerInfo {
    /// Its id: the handle of its LAYER table record.
    #[getter]
    fn id(&self) -> u64 {
        self.0.id()
    }

    /// Its name; names are compared ignoring case.
    #[getter]
    fn name(&self) -> &str {
        self.0.name()
    }

    /// Its color: an index or RGB color.
    #[getter]
    fn color(&self) -> CadColor {
        CadColor(self.0.color())
    }

    /// Name of its linetype.
    #[getter]
    fn linetype(&self) -> &str {
        self.0.linetype()
    }

    /// On (shown) or off.
    #[getter]
    fn is_visible(&self) -> bool {
        self.0.is_visible()
    }

    /// Frozen: hidden and not regenerated.
    #[getter]
    fn is_frozen(&self) -> bool {
        self.0.is_frozen()
    }

    /// Locked: its entities cannot be edited or erased.
    #[getter]
    fn is_locked(&self) -> bool {
        self.0.is_locked()
    }

    /// Whether it is plotted.
    #[getter]
    fn is_plottable(&self) -> bool {
        self.0.is_plottable()
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let name = PyString::new(py, self.0.name()).repr()?;
        Ok(format!("CadLayerInfo(id={}, name={name})", self.0.id()))
    }
}

/// Adds the classes to the module.
/// A block as the Blocks palette shows it (ADR 0025 D1).
#[pyclass(frozen, eq, skip_from_py_object, module = "arqstudiocad._core")]
#[derive(Clone, PartialEq)]
pub struct CadBlockInfo(pub BlockInfo);

#[pymethods]
impl CadBlockInfo {
    /// Its name, spelled as the block table spells it.
    #[getter]
    fn name(&self) -> &str {
        &self.0.name
    }

    /// Its description; empty when it has none.
    #[getter]
    fn description(&self) -> &str {
        &self.0.description
    }

    /// Its insertion units, as DWG's INSUNITS code (0 unitless, 4
    /// millimeters, 5 centimeters, 6 meters…).
    #[getter]
    fn units(&self) -> i16 {
        self.0.units
    }

    /// The attribute definitions it holds, constant ones included.
    #[getter]
    fn attributes(&self) -> usize {
        self.0.attributes
    }

    /// Whether it is a dynamic block.
    #[getter]
    fn is_dynamic(&self) -> bool {
        self.0.dynamic
    }

    /// A dynamic block's parameters the user sees, by name; empty for a
    /// static block or when they cannot be read.
    #[getter]
    fn parameters(&self) -> Vec<String> {
        self.0.parameters.clone()
    }

    fn __repr__(&self) -> String {
        format!("CadBlockInfo(name={:?})", self.0.name)
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<CadEntityInfo>()?;
    m.add_class::<CadBlockInfo>()?;
    m.add_class::<CadLayerInfo>()?;
    m.add_class::<CadText>()?;
    Ok(())
}
