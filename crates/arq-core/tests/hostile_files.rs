//! Hostile drawings (ADR 0037 D2), kept in `tests/hostile/`:
//! - files made by hand to break the reader (`tools/make_hostile_drawings.py`);
//! - inputs the fuzzers found breaking it from our own seeds. A finding
//!   derived from a third-party sample becomes a generated test instead
//!   (`fuzz/README.md`).
//!
//! Each must open as the application opens it, or be refused with an
//! error, within a time limit: never a crash, a hang or a panic. Runs with
//! every `cargo test`; an empty folder is a failure (contract §12).

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use arq_core::{CadDocument, Document};

/// Longer than any of these files needs on the floor machine, by far.
const LIMIT: Duration = Duration::from_secs(60);

fn hostile_files() -> Vec<PathBuf> {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/hostile");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&folder)
        .unwrap_or_else(|e| panic!("{}: {e}", folder.display()))
        .map(|entry| entry.expect("entry").path())
        .filter(|path| {
            matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("dwg" | "dxf")
            )
        })
        .collect();
    files.sort();
    files
}

/// What the application does with a file it opens: read it, index it,
/// measure it and draw everything in it.
fn open_as_the_application_does(path: &Path) {
    let Ok(doc) = CadDocument::open(path) else {
        return; // refused with an error: fine
    };
    let _ = doc.bounds();
    let document: &Document = doc.document();
    let _ = document.drawing_all();
}

#[test]
fn hostile_files_open_or_are_refused_never_crash_or_hang() {
    let files = hostile_files();
    assert!(
        !files.is_empty(),
        "no hostile files: this test would check nothing"
    );
    let mut failures = Vec::new();
    for path in files {
        let (sent, received) = mpsc::channel();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let worker = path.clone();
        thread::Builder::new()
            .name(name.clone())
            .stack_size(8 << 20) // the main thread's, on Linux
            .spawn(move || {
                let outcome = std::panic::catch_unwind(|| open_as_the_application_does(&worker));
                let _ = sent.send(outcome.is_ok());
            })
            .expect("thread");
        match received.recv_timeout(LIMIT) {
            Ok(true) => {}
            Ok(false) => failures.push(format!("{name}: panicked")),
            Err(_) => failures.push(format!("{name}: still running after {LIMIT:?}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
