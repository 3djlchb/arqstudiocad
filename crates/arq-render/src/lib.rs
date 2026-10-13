//! Render scene for ArqStudioCAD (contract §8, ADR 0013).
//!
//! Holds graphics data derived from the document (never the source of truth)
//! and is independent of Qt. Geometry is rendered relative to the camera, so
//! large (e.g. georeferenced) coordinates survive the conversion to `f32`
//! pixels.
//!
//! | Need | Use |
//! |---|---|
//! | What to draw, kept up to date from change sets | [`Scene`] |
//! | Which part of the plan is shown | [`Camera`] |
//! | Paint it into an image on the CPU | [`render`] → [`Image`], with a [`Style`] |
//! | ACI colors | [`aci_rgb`], [`aci_rgb_on`] |

mod aci;
mod camera;
mod raster;
mod scene;

pub use aci::{aci_rgb, aci_rgb_on, luminance};
pub use camera::Camera;
pub use raster::{Image, Style, render};
pub use scene::Scene;
