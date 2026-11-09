# 0021. Spatial index for model-space queries

Date: 2026-10-17 · Status: Accepted

## Context

ADR 0011 D3.8 deferred spatial indexing until phase 02.7 and recorded a
17 ms scan for a 100,000-entity query. The current benchmark on the development
machine measured 18.0 ms for a 1% window query and 17.5 ms for the full drawing.
The public query contract already returns bounding-box candidates in drawing
order; exact geometry and layer filtering remain the caller's responsibility.

The index must work with model-space bounds for both owned and codec-backed
entities, remain private to the Rust core, and stay synchronized with
transactions, rollback, undo and redo. It is derived data and is rebuilt when a
drawing is opened; it is never persisted.

## Decision

- Use `rstar` 0.13.0, an actively maintained Rust R*-tree with MIT OR
  Apache-2.0 licensing, inside `arq-core` only.
- Index finite entity bounding boxes and use the tree to find candidates for
  `query_entities`. Apply the existing tolerance check and return results in
  drawing order. Keep the public API unchanged.
- Build the index from a loaded document. Update it only after a transaction
  commits and after undo/redo has restored the document. Failed transactions
  leave the index untouched.
- Keep the index as a broad phase. Exact picking, crossing/window selection,
  object snaps, visibility filtering and renderer work keep their existing
  semantics; they may adopt indexed candidates in later reviewed steps.
- Compare the existing 100,000-entity benchmark before and after the change.
  Full-drawing queries are expected to remain O(n), since they return all
  entities; small-window queries should avoid scanning unrelated bounds.

On the same development machine, `query 1% window` measured 18.017 ms before
and 44.451 µs after this change (Criterion 20-sample estimate, 2026-10-17).
The full-drawing query, picking, exact selection and snaps are not accelerated
by this first slice.

## Consequences

- Entity bounds are stored twice: once in the document/codec and once in this
  rebuildable index. The R*-tree does not affect persistence or the public API.
- Drawing order is tracked for live entries and retained as a small tombstone
  mapping so undo restores query ordering. IDs are never reused in a document.
- This decision covers the first spatial-index slice of milestone 02.7. The
  wgpu renderer, runtime backend probing and broader viewport acceleration
  remain open under ADRs 0007 and 0013.

## Implementation follow-up (2026-10-17)

The next 02.7 slice uses R*-tree candidates for `pick`, window/crossing
selection and object snaps. `arq-document` still applies the existing exact
drawn-geometry checks and visibility rules to those candidates; the index
does not change query results or drawing order. GPU viewport rendering remains
open. An isolated wgpu adapter/pipeline/offscreen readback probe is now
implemented under ADR 0007; it has passed locally on Windows with DX12, while
Linux reference-machine validation remains open.

## Upgrade to rstar 0.14.0 (2026-11-08)

The reference machine's report (`tools/reference_machine_report.sh`) found
a panic, "This is a bug in rstar", when 1,000 entities were moved in an
opened 100,000-line drawing. rstar 0.13.0's `bulk_load` built some trees
with leaves on different levels, and a later insertion into one panicked
(rstar issue #256, a regression in 0.13.0, fixed in 0.14.0). Opening a
drawing bulk-loads the index, so any edit after opening could hit it. A
standalone reproduction outside this repository showed the fault in rstar
alone. `arq-core` now pins `rstar` 0.14.0 (MSRV 1.90, below ours); its
breaking change to `RTreeNum` does not affect `f64`.
`a_large_opened_drawing_takes_new_entities` fails on 0.13.0 and passes on
0.14.0.

On the reference machine (i7-2600, Criterion 20-sample estimate), 0.13.0
and then 0.14.0: `query 1% window` 37.2 µs and 42.2 µs, `query everything`
7.24 ms and 7.34 ms.

