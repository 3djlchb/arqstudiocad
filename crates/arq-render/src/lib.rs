//! Render scene for ArqStudioCAD.
//!
//! Holds graphics data derived from the document (never the source of truth)
//! and is independent of Qt. Geometry is rendered relative to the camera
//! origin so large (e.g. georeferenced) coordinates survive `f32` conversion.
