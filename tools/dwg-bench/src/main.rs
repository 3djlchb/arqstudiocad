//! dwg-bench: reads, writes and re-reads a corpus of DWG/DXF drawings with
//! AcadRust, opencadcodec and LibreDWG, each operation in a process of its
//! own (a crash or a hang is a result, not the end of the run).
//!
//! ```text
//! dwg-bench job <library> read <input>
//! dwg-bench job <library> write <input> <output.dwg|output.dxf>
//! dwg-bench run <corpus-dir> <work-dir> [timeout-seconds]
//! ```
//!
//! `run` writes `<work-dir>/results.jsonl`, one job per line; `report.py`
//! turns it into the comparison.

mod codecs;
#[cfg(feature = "libredwg")]
mod libredwg;
mod summary;

use std::fs;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use summary::{JobResult, Summary};

fn libraries() -> Vec<&'static str> {
    let mut all = vec!["acadrust", "opencadcodec"];
    if cfg!(feature = "libredwg") {
        all.push("libredwg");
    }
    all
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("job") if args.len() >= 4 => {
            let output = args.get(4).map(PathBuf::from);
            let result = job(&args[1], &args[2], Path::new(&args[3]), output.as_deref());
            println!("{}", serde_json::to_string(&result).expect("serializable"));
        }
        Some("run") if args.len() >= 3 => {
            let timeout = args.get(3).and_then(|t| t.parse().ok()).unwrap_or(120);
            run(
                Path::new(&args[1]),
                Path::new(&args[2]),
                Duration::from_secs(timeout),
            );
        }
        _ => {
            eprintln!("usage: dwg-bench job <library> read|write <input> [output]");
            eprintln!("       dwg-bench run <corpus-dir> <work-dir> [timeout-seconds]");
            std::process::exit(2);
        }
    }
}

/// Peak resident memory of this process (Linux), KiB.
fn peak_kib() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

fn job(library: &str, operation: &str, input: &Path, output: Option<&Path>) -> JobResult {
    let mut result = JobResult {
        library: library.to_owned(),
        operation: operation.to_owned(),
        input: input.display().to_string(),
        output: output.map(|o| o.display().to_string()),
        ..JobResult::default()
    };
    let start = Instant::now();
    let outcome: Result<Option<Summary>, String> = match (library, operation, output) {
        ("acadrust", "read", _) => codecs::acadrust_lib::read(input)
            .map(|(doc, rec)| Some(codecs::acadrust_lib::summarize(&doc, rec))),
        ("opencadcodec", "read", _) => codecs::opencadcodec_lib::read(input)
            .map(|(doc, rec)| Some(codecs::opencadcodec_lib::summarize(&doc, rec))),
        ("acadrust", "write", Some(out)) => {
            let (doc, _) = match codecs::acadrust_lib::read(input) {
                Ok(read) => read,
                Err(e) => return failed(result, format!("read: {e}")),
            };
            let start = Instant::now();
            let written = codecs::acadrust_lib::write(&doc, out).map(|()| None);
            result.millis = start.elapsed().as_secs_f64() * 1e3;
            return finish(result, written);
        }
        ("opencadcodec", "write", Some(out)) => {
            let (doc, _) = match codecs::opencadcodec_lib::read(input) {
                Ok(read) => read,
                Err(e) => return failed(result, format!("read: {e}")),
            };
            let start = Instant::now();
            let written = codecs::opencadcodec_lib::write(&doc, out).map(|()| None);
            result.millis = start.elapsed().as_secs_f64() * 1e3;
            return finish(result, written);
        }
        #[cfg(feature = "libredwg")]
        ("libredwg", "read", _) => libredwg::read(input).map(Some),
        #[cfg(feature = "libredwg")]
        ("libredwg", "write", Some(out)) => {
            let outcome = libredwg::write(input, out);
            result.millis = outcome.as_ref().map_or(0.0, |ms| *ms);
            return finish(result, outcome.map(|_| None));
        }
        _ => Err(format!("unknown job: {library} {operation}")),
    };
    result.millis = start.elapsed().as_secs_f64() * 1e3;
    finish(result, outcome)
}

fn failed(mut result: JobResult, error: String) -> JobResult {
    result.error = Some(error);
    result.peak_kib = peak_kib();
    result
}

fn finish(mut result: JobResult, outcome: Result<Option<Summary>, String>) -> JobResult {
    match outcome {
        Ok(summary) => {
            result.ok = true;
            result.summary = summary;
        }
        Err(e) => result.error = Some(e),
    }
    result.peak_kib = peak_kib();
    result
}

/// Every `.dwg` and `.dxf` under `dir`, sorted.
fn drawings(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(d) = pending.pop() {
        let Ok(entries) = fs::read_dir(&d) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("dwg") || e.eq_ignore_ascii_case("dxf"))
            {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// Runs one job in a child process, with a time limit.
fn spawn(args: &[String], timeout: Duration) -> Result<String, String> {
    let exe = std::env::current_exe().expect("own path");
    let mut child = Command::new(exe)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn: {e}"))?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut out = String::new();
                if let Some(mut stdout) = child.stdout.take() {
                    use std::io::Read;
                    let _ = stdout.read_to_string(&mut out);
                }
                return match out.lines().last() {
                    Some(line) if line.starts_with('{') => Ok(line.to_owned()),
                    _ => Err(describe(status)),
                };
            }
            Ok(None) if start.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("timeout after {} s", timeout.as_secs()));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(5)),
            Err(e) => return Err(format!("wait: {e}")),
        }
    }
}

fn describe(status: ExitStatus) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return format!("crashed (signal {signal})");
        }
    }
    match status.code() {
        Some(101) => "panicked".to_owned(),
        Some(code) => format!("exited with code {code} without a result"),
        None => "ended without a result".to_owned(),
    }
}

fn record(
    out: &mut fs::File,
    library: &str,
    operation: &str,
    input: &Path,
    output: Option<&Path>,
    timeout: Duration,
) -> bool {
    let mut args = vec![
        "job".to_owned(),
        library.to_owned(),
        operation.to_owned(),
        input.display().to_string(),
    ];
    if let Some(o) = output {
        args.push(o.display().to_string());
    }
    let line = spawn(&args, timeout).unwrap_or_else(|error| {
        serde_json::to_string(&JobResult {
            library: library.to_owned(),
            operation: operation.to_owned(),
            input: input.display().to_string(),
            output: output.map(|o| o.display().to_string()),
            error: Some(error),
            ..JobResult::default()
        })
        .expect("serializable")
    });
    writeln!(out, "{line}").expect("results file");
    serde_json::from_str::<JobResult>(&line).is_ok_and(|r| r.ok)
}

fn run(corpus: &Path, work: &Path, timeout: Duration) {
    fs::create_dir_all(work).expect("work dir");
    let results_path = work.join("results.jsonl");
    let mut done = std::collections::HashSet::new();
    if let Ok(existing) = fs::File::open(&results_path) {
        // Resume: jobs already recorded are not run again.
        for line in std::io::BufReader::new(existing)
            .lines()
            .map_while(Result::ok)
        {
            if let Ok(r) = serde_json::from_str::<JobResult>(&line) {
                done.insert((r.library, r.operation, r.input, r.output));
            }
        }
    }
    let mut out = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&results_path)
        .expect("results file");
    let files = drawings(corpus);
    let libs = libraries();
    for (n, input) in files.iter().enumerate() {
        let rel = input.strip_prefix(corpus).unwrap_or(input);
        eprintln!("[{}/{}] {}", n + 1, files.len(), rel.display());
        for writer in &libs {
            let key = |op: &str, o: Option<&Path>| {
                (
                    writer.to_string(),
                    op.to_owned(),
                    input.display().to_string(),
                    o.map(|p| p.display().to_string()),
                )
            };
            if !done.contains(&key("read", None)) {
                record(&mut out, writer, "read", input, None, timeout);
            }
            // LibreDWG also tries DWG 2018, to measure where its writer stops.
            let exts: &[&str] = if *writer == "libredwg" {
                &["dwg", "dxf", "r2018.dwg"]
            } else {
                &["dwg", "dxf"]
            };
            for ext in exts {
                let target = work
                    .join("out")
                    .join(writer)
                    .join(rel)
                    .with_extension(format!(
                        "{}.{ext}",
                        rel.extension().and_then(|e| e.to_str()).unwrap_or("x")
                    ));
                fs::create_dir_all(target.parent().expect("parent")).expect("out dir");
                if !done.contains(&key("write", Some(&target))) {
                    record(&mut out, writer, "write", input, Some(&target), timeout);
                }
                if target.is_file() {
                    for reader in &libs {
                        let reread = (
                            reader.to_string(),
                            "read".to_owned(),
                            target.display().to_string(),
                            None,
                        );
                        if !done.contains(&reread) {
                            record(&mut out, reader, "read", &target, None, timeout);
                        }
                    }
                }
            }
        }
    }
    eprintln!("results: {}", results_path.display());
}
