//! `Document`: one drawing, stored in the DWG/DXF engine's database (AcadRust).

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DwgWriter, DxfReader, DxfWriter};

use crate::error::{DocumentError, DocumentResult};
use crate::format::FileFormat;
use crate::reading::Recovery;

/// One drawing.
///
/// Our own model owns layers and the editable entities (ADR 0018 D2); the
/// engine's `CadDocument` holds them too, written together with the model,
/// and everything the model does not own yet (see `model.rs`). The engine's
/// document lives in a `Box` and never moves: its change recorder
/// identifies a document by its memory address during a transaction
/// (ADR 0011 D3.5).
pub struct Document {
    cad: Box<CadDocument>,
    /// The own internal model (`model.rs`).
    pub(crate) model: crate::model::OwnModel,
    /// The change being recorded, if any (see `change.rs`).
    pub(crate) recording: Option<crate::change::Recording>,
    /// Set when the drawing was recovered from a damaged file.
    recovery: Option<Recovery>,
}

impl Document {
    /// An empty drawing.
    pub fn new() -> Self {
        Self::from_cad(CadDocument::new())
    }

    fn from_cad(mut cad: CadDocument) -> Self {
        // Every table record needs a handle from the start, or its LayerId
        // would change on the first save. At rev 8189f83 the codec already
        // gives one to the records it creates and reads (layer "0" of a new
        // drawing, an R12 DXF without handles); this call keeps that true for
        // any record still without one. Pinned by
        // `layers_without_handles_get_stable_ids`.
        cad.assign_table_entry_handles();
        let mut doc = Self {
            cad: Box::new(cad),
            model: crate::model::OwnModel::default(),
            recording: None,
            recovery: None,
        };
        doc.rebuild_model();
        doc
    }

    /// Opens a `.dwg` or `.dxf` file; the extension decides the format.
    ///
    /// A damaged file opens only if drawing data can be recovered from it,
    /// and then [`recovery`](Self::recovery) says what was wrong; otherwise
    /// the error is [`DocumentError::Damaged`] (ADR 0017 D5.1).
    pub fn open(path: impl AsRef<Path>) -> DocumentResult<Self> {
        let path = path.as_ref();
        let format = format_of(path)?;
        if !path.is_file() {
            return Err(DocumentError::Io {
                path: path.to_path_buf(),
                message: "file not found".into(),
            });
        }
        let bytes = fs::read(path).map_err(|e| DocumentError::Io {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;
        let (cad, recovery) = crate::reading::read(bytes, format, path)?;
        let mut doc = Self::from_cad(cad);
        doc.recovery = recovery;
        Ok(doc)
    }

    /// What was wrong with the file this drawing was recovered from; `None`
    /// for a drawing read intact (or created, not read).
    pub fn recovery(&self) -> Option<&Recovery> {
        self.recovery.as_ref()
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

    /// The drawing as DWG bytes, in its own version.
    ///
    /// R12 (AC1009) exists only as DXF, so such a drawing is written as
    /// AutoCAD 2018 DWG (AC1032), as AutoCAD does; so is one of unknown
    /// version. The drawing itself keeps its version. (As DXF the codec writes
    /// an R12 drawing as R13, the oldest version whose structure matches what
    /// it emits.)
    pub fn to_dwg_bytes(&self) -> DocumentResult<Vec<u8>> {
        if matches!(self.cad.version, DxfVersion::AC1009 | DxfVersion::Unknown) {
            // A cheap copy: entities are shared `Arc`s (see `snapshot`).
            let mut modern = CadDocument::clone(&self.cad);
            modern.version = DxfVersion::AC1032;
            return Ok(DwgWriter::write_to_vec(&modern)?);
        }
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

    /// An independent copy of the drawing as it is now, for long reads
    /// (saving, autosave) away from the live document (ADR 0011, D3.10).
    ///
    /// Cheap: entities are shared `Arc`s, so the copy costs one counter bump
    /// per entity plus the tables, and later edits of either side copy only
    /// what they touch. Taken mid-change it holds the edits made so far,
    /// without the recording.
    pub fn snapshot(&self) -> Self {
        Self {
            cad: Box::new(CadDocument::clone(&self.cad)),
            model: self.model.clone(),
            recording: None,
            recovery: self.recovery.clone(),
        }
    }

    /// File-format version code of the drawing, e.g. `"AC1032"` (R2018).
    pub fn version_code(&self) -> String {
        format!("{:?}", self.cad.version)
    }

    /// Number of entities in the drawing (all spaces, all types).
    pub fn entity_count(&self) -> usize {
        self.cad.entity_count()
    }

    /// Number of entities in model space, counted without reading them.
    pub fn model_entity_count(&self) -> usize {
        self.cad.model_space_entities().count()
    }

    /// The codec document, for the other modules of this crate only.
    pub(crate) fn cad(&self) -> &CadDocument {
        &self.cad
    }

    /// Mutable codec document, for the other modules of this crate only.
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
    use crate::Layer;
    use acadrust::{EntityType, Line};

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

    /// An R12 DXF as old programs write it: layers without handles.
    const R12_WITHOUT_HANDLES: &str = "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1009\n0\nENDSEC\n\
        0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nLAYER\n70\n2\n\
        0\nLAYER\n2\n0\n70\n0\n62\n7\n6\nCONTINUOUS\n\
        0\nLAYER\n2\nMUROS\n70\n0\n62\n1\n6\nCONTINUOUS\n\
        0\nENDTAB\n0\nENDSEC\n\
        0\nSECTION\n2\nENTITIES\n0\nLINE\n8\nMUROS\n10\n0.0\n20\n0.0\n30\n0.0\n11\n5.0\n21\n0.0\n31\n0.0\n\
        0\nENDSEC\n0\nEOF\n";

    #[test]
    fn layers_without_handles_get_stable_ids() {
        let dir = tempfile::tempdir().unwrap();
        let doc = Document::from_dxf_bytes(R12_WITHOUT_HANDLES.as_bytes().to_vec()).unwrap();
        let ids: Vec<u64> = doc.layers().iter().map(Layer::id).collect();
        assert_eq!(ids.len(), 2);
        assert!(ids.iter().all(|&id| id != 0), "{ids:?}");
        assert_ne!(ids[0], ids[1]);
        let walls = doc.layer_by_name("MUROS").unwrap().id();
        assert_eq!(doc.layer(walls).unwrap().name(), "MUROS");
        for name in ["back.dxf", "back.dwg"] {
            let path = dir.path().join(name);
            doc.save(&path).unwrap();
            let back = Document::open(&path).unwrap();
            assert_eq!(back.layer_by_name("MUROS").unwrap().id(), walls, "{name}");
        }
        let new = Document::new();
        assert_ne!(new.layer_by_name("0").unwrap().id(), 0, "a new drawing");
    }

    #[test]
    fn an_r12_drawing_is_saved_as_dwg_2018() {
        let dir = tempfile::tempdir().unwrap();
        let doc = Document::from_dxf_bytes(R12_WITHOUT_HANDLES.as_bytes().to_vec()).unwrap();
        assert_eq!(doc.version_code(), "AC1009");
        let dwg = dir.path().join("casa.dwg");
        doc.save(&dwg).unwrap();
        let back = Document::open(&dwg).unwrap();
        assert_eq!(back.version_code(), "AC1032");
        assert_eq!(back.entities().len(), 1);
        assert_eq!(back.entities()[0].layer(), "MUROS");
        assert_eq!(
            back.layer_by_name("MUROS").unwrap().color(),
            crate::Color::Index(1)
        );
        // The drawing keeps its own version; as DXF the codec writes R13, the
        // oldest version matching the structure it emits (its issue #68).
        assert_eq!(doc.version_code(), "AC1009");
        let dxf = dir.path().join("casa.dxf");
        doc.save(&dxf).unwrap();
        assert_eq!(Document::open(&dxf).unwrap().version_code(), "AC1012");
    }

    #[test]
    fn model_entity_count_counts_model_space_only() {
        let doc = sample();
        assert_eq!(doc.model_entity_count(), 2);
        assert_eq!(doc.model_entity_count(), doc.entities().len());
        // A block's own entities are not in model space.
        let with_block = Document::from_dxf_bytes(
            "0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n8\n0\n2\nB\n70\n0\n10\n0\n20\n0\n30\n0\n3\nB\n\
             0\nLINE\n8\n0\n10\n0\n20\n0\n30\n0\n11\n1\n21\n0\n31\n0\n0\nENDBLK\n8\n0\n0\nENDSEC\n\
             0\nSECTION\n2\nENTITIES\n0\nINSERT\n8\n0\n2\nB\n10\n0\n20\n0\n30\n0\n0\nENDSEC\n0\nEOF\n"
                .as_bytes()
                .to_vec(),
        )
        .unwrap();
        assert_eq!(
            with_block.model_entity_count(),
            1,
            "the INSERT, not the LINE inside"
        );
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

    #[test]
    fn a_snapshot_is_independent_of_the_live_document() {
        use arq_geometry::{Line as Line2, Point2};

        let mut doc = sample();
        let first = doc.entities()[0].id();
        let snap = doc.snapshot();
        let plan = |x: f64| {
            let p = |x, y| Point2::new(x, y).unwrap();
            crate::PlanGeometry::new(Line2::new(p(x, 0.0), p(x, 1.0)).unwrap().into())
        };
        let zero = doc.layer_by_name("0").unwrap().id();

        doc.set_plan_geometry(first, plan(7.0)).unwrap();
        doc.add_entity(plan(9.0), zero).unwrap();
        doc.create_layer("NUEVA").unwrap();
        assert_eq!(snap.entities(), sample().entities(), "edits stay out");
        assert!(snap.layer_by_name("NUEVA").is_none());

        // The other way round: editing the snapshot leaves the live one alone.
        let mut snap = snap;
        let live = doc.entities();
        snap.delete_entity(first).unwrap();
        assert_eq!(doc.entities(), live);
    }

    #[test]
    fn a_snapshot_taken_mid_change_has_its_own_history() {
        let mut doc = sample();
        doc.begin_change().unwrap();
        doc.create_layer("A").unwrap();
        let mut snap = doc.snapshot();
        assert!(!snap.is_recording());
        assert!(snap.layer_by_name("A").is_some(), "edits so far");
        doc.rollback_change().unwrap();
        assert!(doc.layer_by_name("A").is_none());
        assert!(snap.layer_by_name("A").is_some());
        snap.begin_change().unwrap();
        snap.commit_change().unwrap();
    }
}
