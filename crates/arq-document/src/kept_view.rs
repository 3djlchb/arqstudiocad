//! The view a drawing keeps (ADR 0030 D2): its `*Active` viewport's
//! direction, center and height, as AutoCAD keeps them, so a drawing saved
//! in an isometric view opens in it, here and in AutoCAD.
//!
//! Keeping a view is not an edit: no change is recorded, so it makes no
//! undo step and leaves the drawing unmodified. A view never chosen here
//! leaves the viewport as the file had it.

use arq_geometry::{GeometryError, Vector3};

use crate::document::Document;
use crate::error::DocumentResult;

/// The name of the model's current viewport in the VPORT table.
const ACTIVE: &str = "*Active";

/// How the drawing is seen, as its `*Active` viewport keeps it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeptView {
    /// From the target toward the viewer (AutoCAD's VIEWDIR), not
    /// normalized.
    pub direction: [f64; 3],
    /// The point looked at; the center is measured from it.
    pub target: [f64; 3],
    /// The view's turn about its direction, in radians (VIEWTWIST).
    pub twist: f64,
    /// The view's middle on its plane (across and up, from the target).
    pub center: (f64, f64),
    /// How many model units the view shows, top to bottom.
    pub height: f64,
}

impl Document {
    /// The view the drawing keeps; `None` when it has no `*Active`
    /// viewport.
    pub fn kept_view(&self) -> Option<KeptView> {
        let vport = self.cad().vports.get(ACTIVE)?;
        let (d, t) = (vport.view_direction, vport.view_target);
        Some(KeptView {
            direction: [d.x, d.y, d.z],
            target: [t.x, t.y, t.z],
            twist: vport.view_twist,
            center: (vport.view_center.x, vport.view_center.y),
            height: vport.view_height,
        })
    }

    /// Keeps a parallel view in the drawing: seen from `direction`, without
    /// a turn, its middle at `center` on the view's plane (across and up,
    /// from the origin) and `height` model units high. Not an edit (see the
    /// module). A drawing without a `*Active` viewport keeps nothing.
    pub fn keep_view(
        &mut self,
        direction: [f64; 3],
        center: (f64, f64),
        height: f64,
    ) -> DocumentResult<()> {
        let [x, y, z] = direction;
        Vector3::new(x, y, z)?.normalized()?;
        if !(center.0.is_finite() && center.1.is_finite()) {
            return Err(GeometryError::NonFinite {
                what: "view center",
            }
            .into());
        }
        if !(height.is_finite() && height > 0.0) {
            return Err(GeometryError::NegativeOrNonFinite {
                what: "view height",
            }
            .into());
        }
        if let Some(vport) = self.cad_mut().vports.get_mut(ACTIVE) {
            vport.view_direction = acadrust::types::Vector3::new(x, y, z);
            // The view's plane through the origin: its center is then the
            // view's own across and up.
            vport.view_target = acadrust::types::Vector3::new(0.0, 0.0, 0.0);
            vport.view_twist = 0.0;
            vport.view_center = acadrust::types::Vector2::new(center.0, center.1);
            vport.view_height = height;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "kept_view_tests.rs"]
mod tests;
