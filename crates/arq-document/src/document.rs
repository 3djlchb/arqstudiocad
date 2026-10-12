//! `Document`: one drawing, stored in opencadcodec's DWG database.

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use opencadcodec::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};

use crate::error::{DocumentError, DocumentResult};
use crate::format::FileFormat;

/// One drawing.
///
/// The codec's `CadDocument` is the only store (ADR 0011, D3.2). It lives in a
/// `Box` and never moves: the codec's change recorder identifies a document by
/// its memory address during a transaction (D3.5).
pub struct Document {
    cad: Box<CadDocument>,
}

impl Document {
    /// An empty drawing.
    pub fn new() -> Self {
        Self::from_cad(CadDocument::new())
    }

    fn from_cad(cad: CadDocument) -> Self {
        Self { cad: Box::new(cad) }
    }

    /// Opens a `.dwg` or `.dxf` file; the extension decides the format.
    pub fn open(path: impl AsRef<Path>) -> DocumentResult<Self> {
        let path = path.as_ref();
        let format = format_of(path)?;
        if !path.is_file() {
            return Err(DocumentError::Io {
                path: path.to_path_buf(),
                message: "file not found".into(),
            });
        }
        let cad = match format {
            FileFormat::Dwg => DwgReader::from_file(path)?.read()?,
            FileFormat::Dxf => DxfReader::from_file(path)?.read()?,
        };
        Ok(Self::from_cad(cad))
    }

    /// Reads a DXF drawing from memory.
    pub fn from_dxf_bytes(bytes: Vec<u8>) -> DocumentResult<Self> {
        Ok(Self::from_cad(
            DxfReader::from_reader(Cursor::new(bytes))?.read()?,
        ))
    }

    /// The drawing as DXF bytes.
    pub fn to_dxf_bytes(&self) -> DocumentResult<Vec<u8>> {
        Ok(DxfWriter::new(&self.cad).write_to_vec()?)
    }

    /// The drawing as DWG bytes.
    pub fn to_dwg_bytes(&self) -> DocumentResult<Vec<u8>> {
        Ok(DwgWriter::write_to_vec(&self.cad)?)
    }

    /// Saves to `path` in the format its extension names.
    ///
    /// Safe save: the whole file is produced in memory, written next to the
    /// target under a temporary name, then renamed over it. If anything fails,
    /// an existing file at `path` is left untouched.
    pub fn save(&self, path: impl AsRef<Path>) -> DocumentResult<()> {
        let path = path.as_ref();
        let bytes = match format_of(path)? {
            FileFormat::Dwg => self.to_dwg_bytes()?,
            FileFormat::Dxf => self.to_dxf_bytes()?,
        };
        let temp = temp_path(path);
        let io_error = |e: std::io::Error| DocumentError::Io {
            path: path.to_path_buf(),
            message: e.to_string(),
        };
        fs::write(&temp, bytes).map_err(io_error)?;
        fs::rename(&temp, path).map_err(|e| {
            let _ = fs::remove_file(&temp);
            io_error(e)
        })
    }

    /// File-format version code of the drawing, e.g. `"AC1032"` (R2018).
    pub fn version_code(&self) -> String {
        format!("{:?}", self.cad.version)
    }

    /// Number of entities in the drawing (all spaces, all types).
    pub fn entity_count(&self) -> usize {
        self.cad.entity_count()
    }

    /// The codec document, for the other modules of this crate only.
    #[allow(dead_code)] // used by the entity and layer views (steps 3.2–3.3)
    pub(crate) fn cad(&self) -> &CadDocument {
        &self.cad
    }

    /// Mutable codec document, for the other modules of this crate only.
    #[allow(dead_code)] // used by the entity and layer views (steps 3.2–3.3)
    pub(crate) fn cad_mut(&mut self) -> &mut CadDocument {
        &mut self.cad
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

fn format_of(path: &Path) -> DocumentResult<FileFormat> {
    FileFormat::from_path(path).ok_or_else(|| DocumentError::UnsupportedFormat {
        path: path.to_path_buf(),
    })
}

/// `plano.dwg` → `plano.dwg.arqstudiocad-tmp`, in the same folder so the final
/// rename stays on one file system.
fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".arqstudiocad-tmp");
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencadcodec::{EntityType, Line};

    /// A drawing with two lines, built through the codec until the entity
    /// API exists (step 3.2).
    fn sample() -> Document {
        let mut doc = Document::new();
        for (x0, y0, x1, y1) in [
            (0.0, 0.0, 10.0, 0.0),
            (500_000.0, 9_977_000.0, 500_010.0, 9_977_005.0),
        ] {
            doc.cad_mut()
                .add_entity(EntityType::Line(Line::from_coords(
                    x0, y0, 0.0, x1, y1, 0.0,
                )))
                .unwrap();
        }
        doc
    }

    #[test]
    fn new_document_is_empty() {
        let doc = Document::new();
        assert_eq!(doc.entity_count(), 0);
        assert!(doc.version_code().starts_with("AC"));
    }

    #[test]
    fn dxf_round_trip_in_memory() {
        let bytes = sample().to_dxf_bytes().unwrap();
        let back = Document::from_dxf_bytes(bytes).unwrap();
        assert_eq!(back.entity_count(), 2);
    }

    #[test]
    fn save_and_open_both_formats() {
        let dir = tempfile::tempdir().unwrap();
        let doc = sample();
        for name in ["plano.dwg", "plano.dxf", "PLANO2.DWG"] {
            let path = dir.path().join(name);
            doc.save(&path).unwrap();
            let back = Document::open(&path).unwrap();
            assert_eq!(back.entity_count(), 2, "{name}");
            assert_eq!(back.version_code(), doc.version_code(), "{name}");
        }
        // Safe save leaves no temporary file behind.
        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().ends_with("-tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn save_overwrites_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plano.dxf");
        Document::new().save(&path).unwrap();
        sample().save(&path).unwrap();
        assert_eq!(Document::open(&path).unwrap().entity_count(), 2);
    }

    #[test]
    fn unsupported_missing_and_corrupt_files_are_errors() {
        let dir = tempfile::tempdir().unwrap();
        let pdf = dir.path().join("plano.pdf");
        assert!(matches!(
            Document::open(&pdf),
            Err(DocumentError::UnsupportedFormat { .. })
        ));
        assert!(matches!(
            Document::new().save(&pdf),
            Err(DocumentError::UnsupportedFormat { .. })
        ));
        let missing = dir.path().join("no-existe.dwg");
        assert!(matches!(
            Document::open(&missing),
            Err(DocumentError::Io { .. })
        ));
        for name in ["roto.dwg", "roto.dxf"] {
            let path = dir.path().join(name);
            fs::write(&path, b"this is not a drawing").unwrap();
            assert!(
                Document::open(&path).is_err(),
                "{name} must be rejected, not panic"
            );
        }
    }
}
