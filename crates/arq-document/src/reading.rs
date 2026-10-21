//! Reading drawings, and telling damaged files apart (ADR 0017 D5.1).
//!
//! A file is read strictly first. Some damage stops that read with an
//! error; some does not: a truncated R13–2000 DWG reads as an empty
//! drawing, and a corrupt one can make the engine panic. The engine's read
//! statistics reveal the first case, and a panic is caught. Either way the
//! file is read again in the engine's recovery (failsafe) mode: if that
//! yields drawing data, the document opens marked [`Recovery`], with what
//! was wrong; if not, opening fails with [`DocumentError::Damaged`]. A
//! damaged file is never taken for a valid drawing.

use std::io::Cursor;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

use acadrust::io::dwg::DwgReadOptions;
use acadrust::io::dxf::DxfReaderConfiguration;
use acadrust::io::read::{ReadOutcome, ReadStats};
use acadrust::{CadDocument, DwgReader, DxfReader};

use crate::error::{DocumentError, DocumentResult};
use crate::format::FileFormat;

/// Problems listed at most; the rest are counted.
const MAX_PROBLEMS: usize = 20;

/// What was wrong with a drawing opened by recovery: its file is damaged,
/// and part of it may be missing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recovery {
    problems: Vec<String>,
}

impl Recovery {
    /// What the reader found wrong, in English as the engine reports it;
    /// at least one line.
    pub fn problems(&self) -> &[String] {
        &self.problems
    }
}

/// The engine's codes for an object it could not decode and kept verbatim:
/// any class object (our AcadRust patch, ADR 0017 D2) or one of the
/// auxiliary classes AcadRust preserves itself. Its bytes survive a
/// same-version save, so it is a note, not damage.
const KEPT_UNDECODED: [&str; 2] = ["record-kept-undecoded", "auxiliary-object-preserved"];

/// A drawing read from a file.
pub(crate) struct Read {
    /// The engine's document.
    pub(crate) cad: CadDocument,
    /// Set if the file is damaged and the drawing was recovered.
    pub(crate) recovery: Option<Recovery>,
    /// Objects kept undecoded, as `"ACDBASSOC2DCONSTRAINTGROUP (3E3)"`.
    pub(crate) undecoded: Vec<String>,
}

/// Reads a drawing from a file's bytes: the engine's document, the
/// recovery report if the file is damaged but drawing data could be read,
/// and the objects kept undecoded.
pub(crate) fn read(bytes: Vec<u8>, format: FileFormat, path: &Path) -> DocumentResult<Read> {
    let mut problems = match attempt(bytes.clone(), format, false) {
        Ok(outcome) => {
            let problems = damage(&outcome.stats, format);
            if problems.is_empty() {
                return Ok(finish(outcome, None));
            }
            problems
        }
        Err(problem) => vec![problem],
    };
    match attempt(bytes, format, true) {
        Ok(outcome) if usable(&outcome.stats, format) => {
            for problem in damage(&outcome.stats, format) {
                if !problems.contains(&problem) {
                    problems.push(problem);
                }
            }
            let recovery = Recovery {
                problems: capped(problems),
            };
            Ok(finish(outcome, Some(recovery)))
        }
        Ok(_) => Err(damaged(path, problems)),
        Err(problem) => {
            problems.push(problem);
            Err(damaged(path, problems))
        }
    }
}

fn finish(outcome: ReadOutcome, recovery: Option<Recovery>) -> Read {
    let undecoded = outcome
        .stats
        .diagnostics
        .iter()
        .filter(|d| KEPT_UNDECODED.contains(&d.code.as_str()))
        .filter_map(|d| d.record_handle)
        .map(|handle| {
            let name = match outcome.document.objects.get(&acadrust::Handle::new(handle)) {
                Some(acadrust::objects::ObjectType::Unknown { type_name, .. }) => {
                    type_name.as_str()
                }
                _ => "object",
            };
            format!("{name} ({handle:X})")
        })
        .collect();
    Read {
        cad: outcome.document,
        recovery,
        undecoded,
    }
}

fn damaged(path: &Path, problems: Vec<String>) -> DocumentError {
    DocumentError::Damaged {
        path: path.to_path_buf(),
        problems: capped(problems),
    }
}

/// One read; an engine panic becomes an error like any other.
fn attempt(bytes: Vec<u8>, format: FileFormat, failsafe: bool) -> Result<ReadOutcome, String> {
    let read = move || match format {
        FileFormat::Dwg => {
            let options = if failsafe {
                DwgReadOptions::failsafe()
            } else {
                DwgReadOptions::default()
            };
            DwgReader::from_stream_with_options(Cursor::new(bytes), options).read_with_stats()
        }
        FileFormat::Dxf => DxfReader::from_reader(Cursor::new(bytes))?
            .with_configuration(DxfReaderConfiguration {
                failsafe,
                ..DxfReaderConfiguration::default()
            })
            .read_with_stats(),
    };
    match panic::catch_unwind(AssertUnwindSafe(read)) {
        Ok(Ok(outcome)) => Ok(outcome),
        Ok(Err(e)) => Err(e.to_string()),
        Err(payload) => {
            let message = payload
                .downcast_ref::<&str>()
                .map(|s| (*s).to_owned())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_default();
            Err(format!("the reader stopped unexpectedly: {message}"))
        }
    }
}

/// Whether a read found drawing data. An empty DWG still has its table
/// records; a DXF may hold nothing at all and be valid.
fn usable(stats: &ReadStats, format: FileFormat) -> bool {
    match format {
        FileFormat::Dwg => stats.has_usable_drawing_data(),
        FileFormat::Dxf => stats.decoded_source_records > 0,
    }
}

/// What a read that returned a drawing says went wrong: nothing for an
/// intact file.
fn damage(stats: &ReadStats, format: FileFormat) -> Vec<String> {
    let mut problems: Vec<String> = stats
        .diagnostics
        .iter()
        .filter(|d| !KEPT_UNDECODED.contains(&d.code.as_str()))
        .map(|d| d.message.clone())
        .collect();
    if format == FileFormat::Dwg && !stats.has_usable_drawing_data() {
        problems.push("no drawing data could be read".to_owned());
    }
    if !stats.stream_completed {
        problems.push("the drawing ends before its end marker".to_owned());
    }
    if stats.skipped_source_records > 0 && stats.diagnostics.is_empty() {
        problems.push(format!(
            "{} records could not be read",
            stats.skipped_source_records
        ));
    }
    // AcadRust counts the objects it preserves itself as recovered errors.
    let kept = stats
        .diagnostics
        .iter()
        .filter(|d| KEPT_UNDECODED.contains(&d.code.as_str()))
        .count();
    let recovered = stats.recovered_errors.saturating_sub(kept);
    if recovered > 0 && problems.is_empty() {
        problems.push(format!("{recovered} errors were recovered from"));
    }
    problems
}

fn capped(mut problems: Vec<String>) -> Vec<String> {
    if problems.len() > MAX_PROBLEMS {
        let more = problems.len() - MAX_PROBLEMS;
        problems.truncate(MAX_PROBLEMS);
        problems.push(format!("and {more} more"));
    }
    problems
}

#[cfg(test)]
#[path = "reading_tests.rs"]
mod tests;
