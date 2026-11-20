//! Shared, lazily traversed rectangular ARRAY placement.

use crate::{GeometryError, GeometryResult, Vector2};

/// A validated grid; axes rotate, source objects keep their orientation.
#[derive(Clone, Copy, Debug)]
pub struct RectangularGrid {
    rows: usize,
    columns: usize,
    row_spacing: f64,
    column_spacing: f64,
    sin: f64,
    cos: f64,
}

impl RectangularGrid {
    /// Two to 100000 cells, including the source; repeated axes need nonzero
    /// spacing. Angles are radians. Check corners so even a bounded preview
    /// rejects a grid whose unseen offsets overflow.
    pub fn new(
        rows: usize,
        columns: usize,
        row_spacing: f64,
        column_spacing: f64,
        angle: f64,
    ) -> GeometryResult<Self> {
        if rows == 0
            || columns == 0
            || rows
                .checked_mul(columns)
                .is_none_or(|n| !(2..=100_000).contains(&n))
        {
            return Err(GeometryError::OutOfRange {
                what: "rectangular array counts",
            });
        }
        if !row_spacing.is_finite()
            || !column_spacing.is_finite()
            || !angle.is_finite()
            || (rows > 1 && row_spacing == 0.0)
            || (columns > 1 && column_spacing == 0.0)
        {
            return Err(GeometryError::OutOfRange {
                what: "rectangular array spacing or angle",
            });
        }
        let (sin, cos) = angle.sin_cos();
        let grid = Self {
            rows,
            columns,
            row_spacing,
            column_spacing,
            sin,
            cos,
        };
        for (r, c) in [(0, columns - 1), (rows - 1, 0), (rows - 1, columns - 1)] {
            grid.offset(r, c)?;
        }
        Ok(grid)
    }

    /// Cell count including the unchanged original cell.
    pub fn cells(self) -> usize {
        self.rows * self.columns
    }

    fn offset(self, row: usize, column: usize) -> GeometryResult<Vector2> {
        let x = column as f64 * self.column_spacing;
        let y = row as f64 * self.row_spacing;
        Vector2::new(x * self.cos - y * self.sin, x * self.sin + y * self.cos)
    }

    /// Row, column order, excluding the origin; a preview can stop early.
    pub fn offsets(self) -> impl Iterator<Item = GeometryResult<Vector2>> {
        (0..self.rows).flat_map(move |row| {
            (0..self.columns).filter_map(move |column| {
                if row == 0 && column == 0 {
                    None
                } else {
                    Some(self.offset(row, column))
                }
            })
        })
    }
}

#[cfg(test)]
#[path = "rectangular_grid_tests.rs"]
mod tests;
