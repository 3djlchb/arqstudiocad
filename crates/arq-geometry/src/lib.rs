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
mod arc;
mod bounds;
mod circle;
mod error;
mod geometry;
mod intersect;
mod line;
mod point;
pub mod snap;
mod tolerance;
mod transform;
mod vector;

pub use arc::Arc;
pub use bounds::Bounds2;
pub use circle::Circle;
pub use error::{GeometryError, GeometryResult};
pub use geometry::{Geometry2, GeometryKind};
pub use intersect::{Intersections, intersect};
pub use line::Line;
pub use point::Point2;
pub use tolerance::Tolerance;
pub use transform::Transform2;
pub use vector::Vector2;
