//! `CadApplication`: the open documents (contract §4 and §9, ADR 0011 D3.10).
//!
//! Each document sits behind its own lock, so there is one mutation stream
//! per document and a call never holds a document beyond its own duration.
//! Saving copies the drawing under the lock ([`CadDocument::snapshot`]) and
//! writes the file without it, so drawing goes on while a file is written.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::error::{CadError, CadResult};
use crate::history::{CadDocument, Snapshot};

/// Identity of an open document. Never reused while the application runs.
pub type DocumentId = u64;

/// One open document and what the application knows about it.
struct OpenDocument {
    cad: CadDocument,
    name: String,
    path: Option<PathBuf>,
    /// The damaged file the drawing was recovered from, while replacing it
    /// still needs the user's confirmation (ADR 0017 D5.1).
    damaged: Option<PathBuf>,
    /// Kinds of data the user agreed to lose when saving it (ADR 0017
    /// D5.3), asked once per document.
    accepted_losses: BTreeSet<String>,
}

type Shared = Arc<Mutex<OpenDocument>>;

#[derive(Default)]
struct Store {
    /// Open documents, in opening order (ids only grow).
    documents: BTreeMap<DocumentId, Shared>,
    last_id: DocumentId,
}

/// The open documents of one application.
#[derive(Default)]
pub struct CadApplication {
    store: Mutex<Store>,
}

impl CadApplication {
    /// An application with no documents open.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an empty drawing called `name` (the title until it is saved).
    pub fn create_document(&self, name: &str) -> DocumentId {
        self.insert(OpenDocument {
            cad: CadDocument::new(),
            name: name.to_owned(),
            path: None,
            damaged: None,
            accepted_losses: BTreeSet::new(),
        })
    }

    /// Opens a DWG or DXF file, named after it. A damaged file opens only
    /// if a drawing can be recovered from it (`CadError::DamagedFile`
    /// otherwise); then the document's `recovery()` says what was wrong, and
    /// saving over that file needs [`allow_overwriting_damaged`](Self::allow_overwriting_damaged).
    pub fn open_document(&self, path: impl AsRef<Path>) -> CadResult<DocumentId> {
        let path = path.as_ref();
        let cad = CadDocument::open(path)?;
        let damaged = cad.document().recovery().map(|_| path.to_path_buf());
        Ok(self.insert(OpenDocument {
            cad,
            name: file_name(path),
            path: Some(path.to_path_buf()),
            damaged,
            accepted_losses: BTreeSet::new(),
        }))
    }

    /// Saves a document as DWG or DXF, by the file extension (refusing to
    /// replace a damaged original unasked, and stopping before data would be
    /// lost unless accepted, [`CadError::Lossy`]); it then takes
    /// the file's name and path and is unmodified (unless it changed while
    /// the file was being written).
    pub fn save_document(&self, id: DocumentId, path: impl AsRef<Path>) -> CadResult<()> {
        let path = path.as_ref();
        let shared = self.shared(id)?;
        let (snapshot, accepted) = {
            let open = lock(id, &shared)?;
            if open.damaged.as_deref().is_some_and(|d| same_file(d, path)) {
                return Err(CadError::OverwritesDamaged(path.display().to_string()));
            }
            (open.cad.snapshot(), open.accepted_losses.clone())
        };
        snapshot.save_accepting(path, |loss| accepted.contains(loss.kind()))?;
        let mut open = lock(id, &shared)?;
        open.cad.mark_saved(&snapshot);
        open.name = file_name(path);
        open.path = Some(path.to_path_buf());
        Ok(())
    }

    /// The user accepts the recovered drawing: it may now be saved over the
    /// damaged file it came from. Does nothing for an intact drawing.
    pub fn allow_overwriting_damaged(&self, id: DocumentId) -> CadResult<()> {
        lock(id, &self.shared(id)?)?.damaged = None;
        Ok(())
    }

    /// The user agrees that saving this document loses these kinds of data
    /// (the `kinds` of a [`CadError::Lossy`]); later saves stop only for
    /// other losses. Kept until the document is closed.
    pub fn accept_losses(&self, id: DocumentId, kinds: &[String]) -> CadResult<()> {
        lock(id, &self.shared(id)?)?
            .accepted_losses
            .extend(kinds.iter().cloned());
        Ok(())
    }

    /// The kinds of data the user agreed to lose when saving this document.
    pub fn accepted_losses(&self, id: DocumentId) -> CadResult<Vec<String>> {
        Ok(lock(id, &self.shared(id)?)?
            .accepted_losses
            .iter()
            .cloned()
            .collect())
    }

    /// Closes a document, discarding unsaved changes; asking the user first
    /// is the caller's job ([`CadDocument::is_modified`]).
    pub fn close_document(&self, id: DocumentId) -> CadResult<()> {
        self.store()
            .documents
            .remove(&id)
            .map(|_| ())
            .ok_or(CadError::DocumentNotFound(id))
    }

    /// The open documents, in opening order.
    pub fn document_ids(&self) -> Vec<DocumentId> {
        self.store().documents.keys().copied().collect()
    }

    /// A document's title: its file name, or the name it was created with.
    pub fn document_name(&self, id: DocumentId) -> CadResult<String> {
        Ok(lock(id, &self.shared(id)?)?.name.clone())
    }

    /// The file a document was opened from or last saved to.
    pub fn document_path(&self, id: DocumentId) -> CadResult<Option<PathBuf>> {
        Ok(lock(id, &self.shared(id)?)?.path.clone())
    }

    /// Runs `f` with read access to a document, holding its lock only for
    /// the call.
    pub fn read<R>(&self, id: DocumentId, f: impl FnOnce(&CadDocument) -> R) -> CadResult<R> {
        Ok(f(&lock(id, &self.shared(id)?)?.cad))
    }

    /// Runs `f` with write access to a document, holding its lock only for
    /// the call. Changes still go through the document's operations, so
    /// each is a transaction.
    pub fn edit<R>(
        &self,
        id: DocumentId,
        f: impl FnOnce(&mut CadDocument) -> CadResult<R>,
    ) -> CadResult<R> {
        f(&mut lock(id, &self.shared(id)?)?.cad)
    }

    /// A copy of a document as it is now, to read or save from any thread.
    pub fn snapshot(&self, id: DocumentId) -> CadResult<Snapshot> {
        Ok(lock(id, &self.shared(id)?)?.cad.snapshot())
    }

    fn insert(&self, open: OpenDocument) -> DocumentId {
        let mut store = self.store();
        store.last_id += 1;
        let id = store.last_id;
        store.documents.insert(id, Arc::new(Mutex::new(open)));
        id
    }

    /// The document's slot; the store's lock is released on return, so a
    /// slow call on one document never blocks the others.
    fn shared(&self, id: DocumentId) -> CadResult<Shared> {
        self.store()
            .documents
            .get(&id)
            .cloned()
            .ok_or(CadError::DocumentNotFound(id))
    }

    fn store(&self) -> MutexGuard<'_, Store> {
        // Only the short calls above hold the store, and they cannot panic
        // while holding it; recover rather than fail.
        self.store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Locks a document. A panic while it was held (a bug) may have left it
/// half-changed, so it is refused from then on; it can still be closed.
fn lock(id: DocumentId, shared: &Shared) -> CadResult<MutexGuard<'_, OpenDocument>> {
    shared.lock().map_err(|_| {
        CadError::InvalidOperation(format!(
            "document {id} is unusable after an internal error; close it"
        ))
    })
}

/// Whether two paths name the same file (when both exist, as the file
/// system resolves them; otherwise as written).
fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
