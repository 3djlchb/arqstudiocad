//! File formats the document reads and writes.

use std::path::Path;

/// A drawing file format (ADR 0002: DWG native, DXF as open alternative).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FileFormat {
    /// Native binary DWG.
    Dwg,
    /// DXF (ASCII).
    Dxf,
}

impl FileFormat {
    /// The format implied by a file's extension, case-insensitively.
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            "dwg" => Some(Self::Dwg),
            "dxf" => Some(Self::Dxf),
            _ => None,
        }
    }

    /// The usual file extension, without the dot.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Dwg => "dwg",
            Self::Dxf => "dxf",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_extension_ignoring_case() {
        assert_eq!(
            FileFormat::from_path(Path::new("plano.dwg")),
            Some(FileFormat::Dwg)
        );
        assert_eq!(
            FileFormat::from_path(Path::new("C:/x/PLANO.DXF")),
            Some(FileFormat::Dxf)
        );
        assert_eq!(FileFormat::from_path(Path::new("plano.pdf")), None);
        assert_eq!(FileFormat::from_path(Path::new("sin_extension")), None);
    }
}
