//! Geometric types and algorithms for ArqStudioCAD.
//!
//! This crate has no dependencies on the document model, Python or any GUI.
//! Conventions (ADR 0010): coordinates are `f64` model units; angles are
//! radians, 0 = East, counterclockwise positive; constructors validate their
//! input; comparisons take an explicit [`Tolerance`].
//!
//! ```
//! use arq_geometry::{Point2, Tolerance, Vector2};
//!
//! let a = Point2::new(0.0, 0.0)?;
//! let b = a + Vector2::new(3.0, 4.0)?;
//! assert_eq!(a.distance_to(b), 5.0);
//! assert!(((b - a).length() - 5.0).abs() <= Tolerance::DEFAULT.linear());
//! # Ok::<(), arq_geometry::GeometryError>(())
//! ```

pub mod angle;
mod error;
mod point;
mod tolerance;
mod vector;

pub use error::{GeometryError, GeometryResult};
pub use point::Point2;
pub use tolerance::Tolerance;
pub use vector::Vector2;
