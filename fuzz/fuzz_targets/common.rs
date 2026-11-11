//! What both fuzzers do with an input: what the application does with a
//! file it opens (ADR 0037 D2).

use std::path::Path;
use std::sync::Once;

use arq_core::{CadDocument, Document, FileFormat};

/// Reads `data` as a drawing in `format`, then indexes, measures and draws
/// it. Refusing it with an error is fine; a crash, a hang, a memory blowup
/// or a panic that escapes is a finding.
pub fn open_as_the_application_does(data: &[u8], format: FileFormat) {
    quiet_caught_panics();
    let name = Path::new(match format {
        FileFormat::Dwg => "fuzz.dwg",
        FileFormat::Dxf => "fuzz.dxf",
    });
    let Ok(document) = Document::read(data.to_vec(), format, name) else {
        return;
    };
    let doc = CadDocument::from_document(document);
    let _ = doc.bounds();
    let _ = doc.document().drawing_all();
}

/// The reader catches the engine's panics and refuses the file, as the
/// application does (ADR 0017 D5.1). libFuzzer's hook would abort on them
/// even so; this one stays quiet, so only a panic that escapes, which
/// libfuzzer-sys still turns into an abort, is a finding.
fn quiet_caught_panics() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| std::panic::set_hook(Box::new(|_| {})));
}
