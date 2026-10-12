//! `CadApplication`: the open documents (contract §4 and §9, ADR 0011 D3.10).
//!
//! Each document sits behind its own lock, so there is one mutation stream
//! per document and a call never holds a document beyond its own duration.
//! Saving copies the drawing under the lock ([`CadDocument::snapshot`]) and
//! writes the file without it, so drawing goes on while a file is written.

use std::collections::BTreeMap;
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
        })
    }

    /// Opens a DWG or DXF file, named after it.
    pub fn open_document(&self, path: impl AsRef<Path>) -> CadResult<DocumentId> {
        let path = path.as_ref();
        let cad = CadDocument::open(path)?;
        Ok(self.insert(OpenDocument {
            cad,
            name: file_name(path),
            path: Some(path.to_path_buf()),
        }))
    }

    /// Saves a document as DWG or DXF, by the file extension; it then takes
    /// the file's name and path and is unmodified (unless it changed while
    /// the file was being written).
    pub fn save_document(&self, id: DocumentId, path: impl AsRef<Path>) -> CadResult<()> {
        let path = path.as_ref();
        let shared = self.shared(id)?;
        let snapshot = lock(id, &shared)?.cad.snapshot();
        snapshot.save(path)?;
        let mut open = lock(id, &shared)?;
        open.cad.mark_saved(&snapshot);
        open.name = file_name(path);
        open.path = Some(path.to_path_buf());
        Ok(())
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

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
