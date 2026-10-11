//! Core facade of ArqStudioCAD: operations, transactions, undo/redo and queries.
//!
//! Must build and pass its tests without Python, Qt or a graphics system.

/// Version of the CAD core, taken from the workspace version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_matches_workspace() {
        assert_eq!(version(), "0.1.0");
    }
}
