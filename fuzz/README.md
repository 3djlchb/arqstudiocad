# Fuzzing the DWG/DXF readers

A drawing from someone else is untrusted input ([SECURITY.md](../SECURITY.md)).
These fuzzers feed the readers millions of broken drawings and stop at
anything the application could not survive:
- a crash;
- a panic that escapes the reader;
- a hang (20 s);
- memory past 2 GB.

The decisions are in [ADR 0037](../docs/architecture/adr/0037-security-baseline.md) D2.

| Target | Opens the input as |
|---|---|
| `read_dwg` | a `.dwg` file |
| `read_dxf` | a `.dxf` file |

Each target opens its input as the application does, through
`Document::read`, the same path as opening a file. It then indexes,
measures and draws the drawing. A drawing refused with an error is fine.

## Running

This crate is a workspace of its own on the nightly pinned in
`rust-toolchain.toml`. The application never builds with nightly.

**Platform:** Linux, macOS or WSL. libFuzzer does not link with MSVC on
Windows.

```sh
cargo install cargo-fuzz --version 0.13.2 --locked
cd fuzz
cargo fuzz run -O read_dxf corpus/read_dxf ../crates/arq-core/tests/hostile -- \
    -max_total_time=600 -timeout=20 -rss_limit_mb=2048
```

**Seeds:**
- the hand-made hostile drawings in `crates/arq-core/tests/hostile/`;
- the codec's public samples, fetched with `uv run python tools/fetch_codec_samples.py` into `target/corpus/`.

**In CI:** `.github/workflows/fuzz.yml` runs each target for half an hour
every week, by hand for as long as asked, and for a minute on pull requests
that touch the readers or this crate.

## A finding

1. **Reproduce it:** `cargo fuzz run -O <target> <file>`.
2. **Find the cause:** the stable test `cargo test -p arq-core --test hostile_files` shows the panic's message when the file sits in `crates/arq-core/tests/hostile/`.
3. **Fix it** in our code, or in our AcadRust fork (ADR 0017 D2).
4. **Keep it as a regression test.** A finding from our own seeds goes into `crates/arq-core/tests/hostile/` under a name that says what it breaks. A finding derived from a third-party sample is not committed, for its license; instead a test builds the same damage from a drawing written by the test.

   The first finding (2026-11-11) was a DWG without layer "0", which made opening panic outside the reader. Its regression test is `a_drawing_without_layer_zero_opens_recovered_with_it_added`.
