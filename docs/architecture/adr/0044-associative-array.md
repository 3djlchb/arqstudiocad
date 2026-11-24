# 0044. Associative arrays, as AutoCAD keeps them

Date: 2026-11-23 · Status: Proposed · Milestone 02.14, group 3 (ARRAY)

## Context

ARRAY makes independent copies today, in three kinds:
- rectangular ([design](../../design/rectangular-array.md));
- polar ([design](../../design/polar-array.md));
- by a path ([design](../../design/path-array.md)).

AutoCAD and ProgeCAD make an **associative** array by default
(`ARRAYASSOCIATIVITY` 1; ProgeCAD's even from `-ARRAY`,
[comparison](../../testing/array-rectangular-windows.md)). Such an array
is one object: editing its counts or spacing regenerates its items.

### What such an array is in a DWG

The outline below is what the public descriptions of the format say. The
names marked *(to confirm)* wait on files ProgeCAD makes (D4).

- **The array block:**
  - an INSERT of an anonymous block (`*U…`) holds the items;
  - each item is an INSERT of a second anonymous block, the item block,
    which holds the source objects;
  - the source objects are moved into the item block, and leave model
    space.
- **The associative network:** in the named object dictionary,
  `ACAD_ASSOCNETWORK` holds an `AcDbAssocNetwork`.
- **The action:**
  - the network holds an `AcDbAssocAction`;
  - its body is an `AcDbAssocArrayActionBody`, which holds:
    - the item list, each item's place (row, column, level) and its
      transform;
    - the array's own transform;
    - its source block.
- **The parameters:**
  - an `AcDbAssocArrayRectangularParameters` holds them as named value
    parameters, such as items, item spacing, rows, row spacing, levels,
    level spacing and axes angle *(names to confirm)*;
  - an expression may give a value.
- **The dependencies:** `AcDbAssocDependency` objects link the action to
  the array's INSERT and to the item block.

**AcadRust** (our fork, ADR 0017 D2) reads and writes these objects
(`objects/associative.rs`: `AssocNetwork`, `AssocAction`,
`AssocArrayActionBody`, `AssocArrayParameters`, `AssocArrayItem`). The
compatibility matrix keeps them at 100 % in DWG and DXF
([interoperability](../../DWG_DXF_INTEROPERABILITY.md)).

**What ours does with one today:**
- it draws it, as the INSERT it is;
- it moves, turns, scales and mirrors it as a block;
- Properties calls it a block reference.

Two things are unknown:
- **Moving it:** whether AutoCAD takes an array moved this way, its
  action body's transform left as it was, or puts it back or breaks it.
- **Exploding it:** what ProgeCAD gives when one of several sources is
  exploded.

ADR 0017 D6 has three levels for dynamic content:
- **keeping** it;
- **showing** its current state;
- **recomputing** it.

An associative array has the same three. The parametric engine of
milestone 04.1 is the dependency graph that will recompute everything
associative (ADR 0014: leave room, build nothing early).

## Decision

The maintainer chose the following on 2026-11-23.

### D1 — As AutoCAD keeps it

An associative array of ours is written as AutoCAD and ProgeCAD write
one:
- the anonymous blocks and the INSERT;
- the network, the action and its body;
- the parameters and the dependencies.

So AutoCAD and ProgeCAD edit it as an array. Our own model holds it as
an array:
- its kind;
- its source objects;
- its parameters: rows, columns, levels and their spacings, axes angle.

It is read from those objects and written back to them. Nothing of the
network is invented where a sample has not shown it.

### D2 — In deliveries, reading first

1. **Read and show** the arrays others make. Rectangular, as D3 says.
   - **Recognized:** an INSERT whose action body is an array's is an
     array, its parameters read from the network.
   - **Properties:** it shows `Array (Rectangular)`, with its rows,
     columns, levels, spacings and axes angle, read only.
   - **Moved, turned, scaled or mirrored whole:** the INSERT and the
     action body's transform are kept consistent, as the samples show
     AutoCAD's are.
   - **EXPLODE:** it gives what ProgeCAD gives.
   - **Refused:** an edit that would leave the network inconsistent is
     refused, nothing changed, and the network is kept as read.
2. **Create:**
   - ARRAY makes an associative array when `ARRAYASSOCIATIVITY` is 1,
     with ARRAY's `Associative` option, as AutoCAD;
   - the graph is written as the samples have it, and checked in
     ProgeCAD.
3. **Edit:**
   - the counts, spacings and angle, from Properties and from ARRAYEDIT's
     basic options;
   - the array block regenerated from its parameters.
4. **Polar,** then 5. **path,** each read, created and edited.

### D3 — Rectangular first

Each of D2's steps is done for the rectangular array first. Polar and
path arrays follow in their own deliveries. Until each is read, they stay
blocks, as today, their network kept.

### D4 — Samples from ProgeCAD first

Nothing is read or written until ProgeCAD has made files to read.
`tools/progecad_command_compare.lsp` gets `arq:assoc-array-samples`,
for the Windows laptop ([instructions](../../testing/assoc-array-samples-windows.md)).
It makes these rectangular associative arrays, each saved as DWG 2018
and as DXF 2018:

| Case | What it makes |
|---|---|
| AA1 | one line, 3 × 4 |
| AA2 | several sources on their own layers and colors, 2 × 3 |
| AA3 | 3 × 4 with a 30° axes angle |
| AA4 | 2 × 2 × 2 levels |
| AA5 | AA1 moved, turned and scaled |
| AA6 | AA1 with its columns changed by ARRAYEDIT |
| AA7 | AA2 exploded |

The DXF shows each object's fields by name. The DWG is what ours reads
first. AA5 shows how AutoCAD keeps the INSERT and the action body
consistent; AA6 shows what changes when an array regenerates.

### D5 — Room for the parametric engine

The array regenerates itself from its parameters, a pure function:
parameters and sources in, the item transforms out. That function is
what milestone 04.1's engine will call when it recomputes what depends
on what changed. Until then nothing depends on an array but the array.

## Consequences

- ARRAY stays non-associative until delivery 2. Its prompts and
  `ARRAYASSOCIATIVITY` 0 are unchanged.
- Each delivery needs ProgeCAD on the Windows laptop: first for the
  samples, then to check what ours writes.
- An associative array read from a file and edited in a way not yet
  supported is refused, not flattened: nothing is lost (ADR 0017 D5).
- **To decide when the samples arrive:**
  - whether the sample files, synthetic and small, are kept in the
    repository as test fixtures;
  - or they stay on the machines and only what they show is recorded.
