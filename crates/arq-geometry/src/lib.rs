//! Geometric types and algorithms for ArqStudioCAD.
//!
//! This crate has no dependencies on the document model, Python or any GUI.
//!
//! # Conventions ([ADR 0010])
//!
//! - Coordinates are `f64` **model units**.
//! - Angles are **radians**, 0 = East (+X), counterclockwise positive.
//!   Degrees exist only in the user interface.
//! - Constructors validate their input: an invalid geometry cannot exist.
//! - Comparisons and intersections take an explicit [`Tolerance`].
//! - Arcs follow DWG: counterclockwise from `start_angle` to `end_angle`.
//! - Algorithms compute in a local frame, so survey coordinates (~1e6) keep
//!   their precision.
//!
//! # Map
//!
//! | Need | Use |
//! |---|---|
//! | Positions and displacements | [`Point2`], [`Vector2`], [`angle`] |
//! | Bounding boxes | [`Bounds2`] |
//! | 3D positions and displacements | [`Point3`], [`Vector3`] |
//! | 3D bounding boxes and affine transforms | [`Bounds3`], [`Transform3`] |
//! | Curves | [`Line`], [`Circle`], [`Arc`], and [`Geometry2`] for any of them |
//! | Move, rotate, scale, mirror | [`Transform2`] |
//! | Where curves meet | [`intersect`] → [`Intersections`] |
//! | Object-snap points | [`snap`] |
//! | Errors | [`GeometryError`] |
//!
//! # Example
//!
//! ```
//! use std::f64::consts::FRAC_PI_2;
//! use arq_geometry::{Circle, Geometry2, Line, Point2, Tolerance, Transform2, intersect, snap};
//!
//! let tol = Tolerance::DEFAULT;
//! let wall = Line::new(Point2::new(-10.0, 3.0)?, Point2::new(10.0, 3.0)?)?;
//! let column = Circle::new(Point2::ORIGIN, 5.0)?;
//!
//! // Where the wall crosses the column: (-4, 3) and (4, 3).
//! let found = intersect(wall.into(), column.into(), tol);
//! assert_eq!(found.points().len(), 2);
//!
//! // Rotate the wall a quarter turn around the origin (ROTATE 90).
//! let turned = Transform2::rotation(Point2::ORIGIN, FRAC_PI_2)?.apply_line(wall)?;
//! assert!(turned.start().approx_eq(Point2::new(-3.0, -10.0)?, tol));
//!
//! // Tangent points from (0, 10) to the column, for a TAN snap.
//! let tangents = snap::tangent_points(column.into(), Point2::new(0.0, 10.0)?, tol);
//! assert_eq!(tangents.len(), 2);
//!
//! // Any curve through one type.
//! let shapes: [Geometry2; 2] = [wall.into(), column.into()];
//! let total: f64 = shapes.iter().map(|g| g.length()).sum();
//! assert!((total - (20.0 + 10.0 * std::f64::consts::PI)).abs() < 1e-9);
//! # Ok::<(), arq_geometry::GeometryError>(())
//! ```
//!
//! [ADR 0010]: https://github.com/3djlchb/arqstudiocad/blob/main/docs/architecture/adr/0010-geometry-conventions.md

pub mod angle;
mod arc;
mod bounds;
mod bounds3;
mod circle;
mod error;
mod geometry;
mod intersect;
mod line;
mod point;
mod point3;
pub mod snap;
mod tolerance;
mod transform;
mod transform3;
mod vector;
mod vector3;

pub use arc::Arc;
pub use bounds::Bounds2;
pub use bounds3::Bounds3;
pub use circle::Circle;
pub use error::{GeometryError, GeometryResult};
pub use geometry::{Geometry2, GeometryKind};
pub use intersect::{Intersections, intersect};
pub use line::Line;
pub use point::Point2;
pub use point3::Point3;
pub use tolerance::Tolerance;
pub use transform::Transform2;
pub use transform3::Transform3;
pub use vector::Vector2;
pub use vector3::Vector3;
