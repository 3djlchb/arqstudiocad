//! `CadEntityInfo` and `CadLayerInfo`: immutable descriptions of an entity
//! and a layer (contract §4, ADR 0012 D4.2). They are copies taken when the
//! query ran; later edits never change them.

use arq_core::{Entity, Layer};
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
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<CadEntityInfo>()?;
    m.add_class::<CadLayerInfo>()?;
    Ok(())
}
