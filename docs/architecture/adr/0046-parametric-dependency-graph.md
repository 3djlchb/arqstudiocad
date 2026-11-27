# 0046. Parametric engine foundation: dependency graph

Date: 2026-11-27 · Status: Accepted · Milestone 04.1

## Context

Milestone 04.1 calls for an owned parametric engine that will eventually
regenerate dependent geometry, solve 2D geometric and dimensional constraints,
and evaluate dynamic blocks (ADR 0017 D6 and ADR 0023 D3a). Associative arrays
also need deterministic regeneration from their parameters (ADR 0044 D5).

The repository currently preserves and displays constraint data from DWG, but
does not own an evaluator or dependency scheduler. The first delivery must be
useful groundwork without choosing a constraint solver or changing DWG data.

## Decision

### D1 — A deterministic, pure dependency graph in `arq-core`

The first 04.1 delivery is a Rust value type in `arq-core`. It stores directed
dependencies between opaque `DependencyNode` identifiers. An edge from A to B
means B must be regenerated after A changes. The graph does not depend on CAD
entity geometry, Python, Qt, or a third-party engine.

Adding an edge validates that its endpoints exist and that it will not create a
cycle. A rejected change leaves the graph unchanged. Given changed source
nodes, the graph returns affected dependents in a deterministic topological
order, with node identifiers breaking ties between otherwise-ready nodes.
Changed source nodes are not included in the returned regeneration plan.

### D2 — Planning only in this delivery

The graph identifies what is affected and in what order. It does not execute
evaluators, solve constraints, alter entities, or persist its contents. This
keeps the first delivery small and leaves the consumer contract open for the
dimension, dynamic-block, and associative-array designs.

Later 04.1 deliveries must decide how graph state becomes document-owned,
transactional, undoable, and persistent through the DWG/DXF adapter before
user-visible parametric data is introduced. They must preserve all-or-nothing
operations and the existing document ownership rules.

### D3 — No solver or external technology selected

This decision does not choose a geometric-constraint solver, add a library,
define constraint semantics, decode or write associative DWG objects, or alter
the Python API. Those choices require their own evidence and review under
master plan §16.

## Consequences

- Graph behavior can be tested entirely in Rust on Linux, including cycle
  rejection, propagation, deterministic ordering, and unchanged state after a
  rejected edge.
- Consumers can use the same scheduling primitive without duplicating graph
  traversal.
- The graph is infrastructure only until a later delivery integrates it with
  document transactions and persistence; it does not claim 04.1 is complete.
