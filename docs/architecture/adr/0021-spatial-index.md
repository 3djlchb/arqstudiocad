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
