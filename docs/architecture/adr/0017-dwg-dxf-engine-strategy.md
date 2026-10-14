# 0017. DWG/DXF engine: AcadRust as the base, behind a contract of ours

Date: 2026-10-13 · Status: Accepted (interoperability validation pending) ·
Amends [0008](0008-opencadcodec.md) · Evidence:
[DWG/DXF library comparison](../../references/dwg-library-comparison.md)

## Context

ADR 0008 made opencadcodec the DWG/DXF engine. The comparison of
2026-10-13 (220 drawings, 6,600 operations, each library in its own
process) found that opencadcodec is a fork of **AcadRust** (crates.io,
MPL-2.0), close to AcadRust 0.6.1, while AcadRust has moved on to 0.6.4;
each line has work the other lacks. GNU LibreDWG, measured on the same
files, read no drawing the codecs could not, writes weak DXF and marks most
dynamic-block and associative classes unstable.

The comparison also found two ways to lose data silently: converting a DWG
to another version drops the custom objects the codec cannot re-encode
(opencadcodec; AcadRust refuses instead), and a truncated DWG opens as an
empty drawing with no error (both codecs).

## Decision

### D1 — AcadRust is the production base *(maintainer)*

The DWG/DXF adapter is built on **AcadRust from crates.io**, pinned to an
exact version (`=0.6.4`, the version measured). Upgrades are deliberate:
each runs the round-trip corpus and the comparison bench
(`tools/dwg-bench`) before it is accepted, and the version, its results
and the date are recorded in this ADR's log below.

### D2 — opencadcodec's improvements are recovered one by one

No wholesale merge of the two code bases. The concrete differences are
listed (the comparison's annex and section 12) and each valuable one is
brought in as a small change with a regression drawing that proves it:
first the **geometric-constraint groups** (`ACDBASSOC2DCONSTRAINTGROUP`),
then entity-fidelity fixes and diagnostics. Our changes live in **our own
fork** of AcadRust ([3djlchb/acadrust](https://github.com/3djlchb/acadrust)),
used through `[patch.crates-io]` pinned to a commit, with its origin and
license recorded in `THIRD_PARTY.md`. *(Amended 2026-10-13, maintainer:)*
they are **not sent upstream**; the fork is maintained by us. Each AcadRust
upgrade re-applies our changes on the new version and runs their
regression drawings, the round-trip corpus and the comparison bench before
it is accepted (D1).

### D3 — LibreDWG stays out of the product

LibreDWG is not linked into the application. It stays an optional
reference in `tools/dwg-bench`, and is reconsidered only if drawings appear
that AcadRust cannot process and LibreDWG can.

### D4 — The model is ours; the codec is an adapter

The rule of ADR 0008 (codec types never leave `arq-document`) becomes an
explicit contract of `arq-document`, so the engine can be replaced without
touching the core, BIM data or the interface:

| Operation | Contract |
|---|---|
| `read_document` | read a file; return the document **and its diagnostics** (records skipped or recovered, whether it is partial) |
| `write_document` | write with an **explicit target version** |
| `inspect_capabilities` | which entities and operations the engine supports, as the core uses them |
| `validate_roundtrip` | what survives writing and reading again |

These are our contract's names, not the library's.

### D5 — Four mandatory protections

1. **Damaged files are reported.** A file read partially, or one that
   reads as empty although it has content, yields an explicit diagnostic;
   it is never taken for a valid empty drawing, and it cannot be saved over
   the original without the user's confirmation.
2. **Safe save.** Write to a temporary file, read it back, validate it
   against the document; only then replace the original. On failure the
   original stays untouched.
3. **Unknown objects are preserved.** Their original data, source version
   and identity are kept. If a version conversion cannot keep them, the
   user is warned and can cancel; a loss is never silent. Some proprietary
   objects cannot survive a version change at all: then the conversion is
   stopped, not promised.
4. **Regression drawings.** Every improvement brought in from opencadcodec,
   and every protection, has a drawing that shows it working and that the
   rest still round-trips.

### D6 — Dynamic blocks and constraints: three levels, our engine

The adapter reads and writes parameters, actions, references and
associative data; the Rust document model represents and keeps them; an
ArqStudioCAD **parametric engine** evaluates constraints, visibility,
stretches and dependencies; the interface edits parameters. Three levels of
compatibility are distinct: **keeping** a dynamic block's definition,
**showing** its current state (the evaluated block stored in the file) and
**recomputing** its geometry when a parameter changes. Open CAD Studio
(GPL-3.0) is a technical reference for the third level: we learn from its
design; we do not copy modules without reviewing their dependencies and
obligations.

### D7 — Licenses

The product stays GPL-3.0-or-later. AcadRust's files keep MPL-2.0, and so do
our changes to them (MPL-2.0 file-level obligations); every dependency and
every change brought in is recorded in `THIRD_PARTY.md`, after checking the
copyright notices of the specific files.

## Pending

- **Interoperability:** the comparison's output files, opened in an
  independent CAD application, must confirm the measurements before this
  decision's validation is closed.
- **Xrefs and BIM drawings** (AutoCAD Architecture, Civil 3D, Revit and
  ArchiCAD exports) are not yet in the corpus.

## Consequences

- One Rust engine, published and maintained upstream, with our own fixes
  kept in our fork; upgrades cost re-applying them.
- The editing work of milestone 2.5 is re-validated on AcadRust before it
  goes on.
- Saving costs a re-read; for the drawings measured it is milliseconds.

## Log

| Date | Engine | Corpus | Notes |
|---|---|---|---|
| 2026-10-13 | AcadRust 0.6.4 (measured) | 220 drawings, all three libraries | comparison; decision taken |
| 2026-10-13 | AcadRust 0.6.4 + our patch `b2358ab` ([fork](https://github.com/3djlchb/acadrust/tree/arqstudiocad/preserve-undecoded)) | round-trip corpus; LibreDWG `Constraints.dwg` R14–2018 | 03.2: the fork's constraint-group reading turned out to be garbage for R14–2010 (old layout); instead, class objects that fail to decode are kept verbatim, so those drawings open intact and keep their constraints byte for byte in a same-version save. Decoding the old layout is pending. Our changes stay in our fork, not sent upstream (D2 amended). |
