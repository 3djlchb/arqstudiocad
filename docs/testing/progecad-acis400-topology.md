# Voids and second lumps in DWG 2000 (SAT 400)

Found by ProgeCAD 2026's check of the solids UNION, SUBTRACT and INTERSECT
make (ADR 0032 D4), 2026-11-07.

## What failed

A drawing holding seven combined solids, saved in DWG 2018, in DXF and in
DWG 2000 (on LibreDWG's native `2000/Constraints.dwg`), was opened in
ProgeCAD 2026 Professional, each solid measured and the drawing audited, all
by script on copies:

| Drawing | Solids | Worst volume difference from ours | AUDIT |
|---|---|---|---|
| DWG 2018 | 7 | 1.1e-15 | 0 errors |
| DXF | 7 | 1.1e-15 | 0 errors |
| DWG 2000 | 7 | **3.7e-2** | 0 errors |

In DWG 2000 one solid differed: a 3 × 3 × 3 m box with a 1 m cubic void
measured 27 m³ in ProgeCAD, 26 m³ in ours. ProgeCAD did not see the void.

## Why

DWG 2000 keeps a solid's ACIS as SAT 400, which lacks the pointer ACIS 7.0
added to every topology record after its attribute and id. The codec writes
ACIS 7.0 and drops that pointer when it saves in an older version; reading
SAT 400, it puts a placeholder back.

- **The fault:** for lumps it dropped token 1 instead of token 0, and for
  shells token 2 instead of 0.
- **Lumps:** they lost their next pointer, so a body's second lump was
  unreachable.
- **Shells:** the void that follows the outside shell moved into the
  subshell's place.
- **Reading:** SAT 400 was read with the same mistake, so the codec read its
  own output back unharmed. That is why no round trip of ours had found it.

The outer shell of the hollow box, written and saved:

```text
SAT 700 written:  shell $-1 -1 $-1 $166 $-1 $153 $-1 $167 #   next $166 (the void)
SAT 400 saved:    shell $-1 $-1 $166 $153 $-1 $167 #          $166 where the subshell goes
SAT 400 fixed:    shell $-1 $166 $-1 $153 $-1 $167 #
```

## The correction

- **Fork `f10cc74`** (red test `8823d84`, immutable tag
  `arqstudiocad/on-ea66abe-acis400-topology`, stacked on `6e03fb1`): every
  topology record drops the added pointer, and regains it, at token 0,
  lumps and shells included, as the codec's own `SatLump` and `SatShell`
  read them.
- **Fork tests:** `tests/acis_v400_topology.rs` reads a body of two lumps,
  the first with a void, in SAT 700 and in SAT 400, and writes it down to
  SAT 400. It checks every next pointer and the written records themselves.
  The full default suite passes.
- **Application test:** `a_void_and_a_second_lump_survive_dwg_2000` saves a
  hollow box and a body of two lumps in DWG 2000 and checks the SAT 400
  records as written. It fails with the previous pin.

## Checked in ProgeCAD

The hollow box and the two-lump solid, saved in DWG 2000 on
`2000/Constraints.dwg` with the fix:

| Solid | ProgeCAD's volume | Exact | AUDIT |
|---|---|---|---|
| Hollow box | 26 000 000 000 | 26 000 000 000 | 0 errors |
| Two boxes touching along an edge | 2 000 000 000 | 2 000 000 000 | |

Both the void and the second lump are now there in DWG 2000.

## Limits

- No AutoCAD-written SAT 400 body with several lumps or a void was found, in
  the codec samples or in the maintainer's local drawings. The layout rests
  on the codec's own ACIS 7.0 accessors, on ProgeCAD reading ACIS 7.0 (SAB)
  voids as they are, and on ProgeCAD reading the corrected SAT 400.
- Single-lump solids without voids were unaffected: the misplaced pointers
  were all null.
