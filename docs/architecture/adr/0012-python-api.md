# 0012. Python API of the core

Date: 2026-10-12 · Status: Accepted · Builds on [0010](0010-geometry-conventions.md),
[0011](0011-document-model-and-transactions.md)

## Context

Phase 4 exposes `arq-core` to Python through `crates/arq-python` (PyO3,
abi3 for Python 3.12+), as contract §4 requires: a small, stable set of
classes, typed with a `.pyi` stub, no Python object holding a reference into
Rust memory across calls, and every error a specific Python exception. The
commands, scripts and the interface (phases 5 and 6) are written against this
API, so its shape is decided before writing it.

## Decision

### D4.1 — Points and vectors are tuples

A single point or vector is returned as a `tuple[float, float]`. On entry
any sequence of two numbers is accepted — a tuple, a list or a NumPy array of
shape `(2,)` — so NumPy users pass their arrays directly. Rust validates every
value; NaN or infinity raises `InvalidGeometryError`. No point class: commands
and scripts write `(10.0, 5.0)` without imports.

Bulk coordinates (polyline vertices, exports, render data; later phases) are
returned as `(N, 2)` `float64` arrays through Python's buffer protocol, which
`numpy.asarray` reads without a copy. NumPy stays optional: the core never
requires it, and the installers do not grow by its 20–30 MB.

### D4.2 — Values are immutable classes

`CadColor`, `CadBounds2`, `CadLine`, `CadCircle`, `CadArc`, `CadEntityInfo`,
`CadLayerInfo` and `CadChangeSet` are frozen: read-only attributes, equality,
`repr`, and hashing where it makes sense. They are copies; changing the
drawing never changes one already returned.

- `CadColor` has named constructors (`by_layer()`, `by_block()`,
  `from_index(1..=255)`, `from_rgb(r, g, b)`) and the attributes `kind`,
  `index` and `rgb`; index 7 and RGB (7, 7, 7) cannot be confused.
- Entity geometry is one class per curve — `CadLine`, `CadCircle`, `CadArc`
  — with `CadEntityInfo.geometry` set to `None` for read-only kinds (TEXT,
  INSERT…). Each class supports `match`/`case` by keyword.
- Angles are radians (ADR 0010); degrees exist only in the interface.

### D4.3 — Documents are handles

`CadApplication` owns the open documents (the Rust store, one lock each).
`CadDocument` in Python is a handle: the shared store plus a `DocumentId`.
Every call locks the document for that call only. After the document is
closed, any call raises `DocumentNotFoundError`.

### D4.4 — Errors

`CadError` (a subclass of `Exception`) is the base; each `CadError` variant
of contract §4 is a subclass: `InvalidGeometryError`, `EntityNotFoundError`,
`LayerNotFoundError`, `DocumentNotFoundError`, `InvalidOperationError`,
`CadIoError` (and, since milestone 03.3, its subclasses `DamagedFileError`,
`OverwritesDamagedError`, `SaveNotVerifiedError` and `LossyConversionError`), `UnsupportedFormatError`. Each carries a stable `code`
(`"invalid_geometry"`, `"entity_not_found"`…) and an English technical
message; the not-found errors also carry the id. Translated text is produced
in Python from the code (contract §13).

### D4.5 — Groups are context managers

`with doc.group("COPY") as group:` begins a group and, on normal exit, ends
it; `group.changes` then holds the merged change set. On an exception the
group is cancelled (if the core has not already rolled it back) and the
exception propagates. `begin_group`/`end_group`/`cancel_group` stay available
for commands that span several calls.

### D4.6 — The GIL

Released only while **opening and saving** a file (reading or writing it and
the codec's work: up to seconds), so the interface and other Python threads
keep running. Every other call takes micro- to milliseconds and keeps it.
Queries are reconsidered with the spatial index (phase 7).

The binding never runs Python code while a document is locked: a thread that
released the GIL may hold that lock, and it must be able to finish without
the GIL, or the two threads would wait for each other forever. A test checks
that other Python threads really run during an open and a save.

### D4.7 — Collections

Operations take ids as any sequence of `int`; queries return `list[int]`;
`entities()` and `layers()` return lists of info objects. Change sets expose
tuples.

### D4.8 — Public module and stubs

The classes live in the native module `arqstudiocad._core`; application code
imports them from `arqstudiocad.core`, which re-exports them with `__all__`.
The stub `_core.pyi` is written by hand and checked against the built module
by mypy's `stubtest`, one of the required checks: a stub that drifts from the
module fails.

## Consequences

- The binding converts types and errors only; every rule (transactions, locks,
  read-only kinds, locked layers) stays in `arq-core`.
- Python code is fully typed; mypy catches a wrong argument or an unhandled
  `None` geometry before running.
- Tuples keep scripts short, at the price of no methods on points; geometric
  helpers needed by commands come from the core, not from Python math.
