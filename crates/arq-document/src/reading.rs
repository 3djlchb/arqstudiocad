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

/// Reads a drawing from a file's bytes: the engine's document, and the
/// recovery report if the file is damaged but drawing data could be read.
pub(crate) fn read(
    bytes: Vec<u8>,
    format: FileFormat,
    path: &Path,
) -> DocumentResult<(CadDocument, Option<Recovery>)> {
    let mut problems = match attempt(bytes.clone(), format, false) {
        Ok(outcome) => {
            let problems = damage(&outcome.stats, format);
            if problems.is_empty() {
                return Ok((outcome.document, None));
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
            Ok((
                outcome.document,
                Some(Recovery {
                    problems: capped(problems),
                }),
            ))
        }
        Ok(_) => Err(damaged(path, problems)),
        Err(problem) => {
            problems.push(problem);
            Err(damaged(path, problems))
        }
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
    if stats.recovered_errors > 0 && problems.is_empty() {
        problems.push(format!(
            "{} errors were recovered from",
            stats.recovered_errors
        ));
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
