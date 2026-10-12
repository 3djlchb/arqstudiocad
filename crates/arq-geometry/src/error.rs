//! Errors raised when a geometric value cannot be built or computed.

use std::fmt;

/// Why a geometric value was rejected.
///
/// Mapped to `CadError::InvalidGeometry` by the core (contract §4).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum GeometryError {
    /// A coordinate, length or angle was NaN or infinite.
    NonFinite {
        /// What was being built, e.g. `"Point2"`.
        what: &'static str,
    },
    /// A direction was requested from a vector of zero length.
    ZeroVector,
    /// A tolerance must be finite and strictly positive.
    InvalidTolerance,
    /// A margin, distance or size must be finite and not negative.
    NegativeOrNonFinite {
        /// Which value, e.g. `"margin"`.
        what: &'static str,
    },
    /// A line segment whose start and end coincide.
    ZeroLength,
    /// A circle or arc radius must be finite and strictly positive.
    InvalidRadius,
    /// An arc whose start and end angles coincide (use a circle instead).
    ZeroSweep,
    /// Three points on one line define no circle or arc.
    Collinear,
    /// A distance or parameter falls outside the curve.
    OutOfRange {
        /// Which value, e.g. `"distance"`.
        what: &'static str,
    },
}

impl fmt::Display for GeometryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite { what } => write!(f, "{what}: values must be finite numbers"),
            Self::ZeroVector => write!(f, "a zero-length vector has no direction"),
            Self::InvalidTolerance => write!(f, "tolerances must be finite and positive"),
            Self::NegativeOrNonFinite { what } => {
                write!(f, "{what} must be a finite number, zero or greater")
            }
            Self::ZeroLength => write!(f, "a line needs two different points"),
            Self::InvalidRadius => {
                write!(f, "the radius must be a finite number greater than zero")
            }
            Self::ZeroSweep => write!(f, "an arc needs different start and end angles"),
            Self::Collinear => write!(f, "three points on one line define no arc or circle"),
            Self::OutOfRange { what } => write!(f, "{what} is outside the curve"),
        }
    }
}

impl std::error::Error for GeometryError {}

/// Shorthand for results of geometric constructors and operations.
pub type GeometryResult<T> = Result<T, GeometryError>;
