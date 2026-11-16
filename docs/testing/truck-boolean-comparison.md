# Booleans compared with Truck

ADR 0032 D4 asks for our booleans of planar solids to be compared with
Truck's where Truck works: in general position, with no face of one solid
on a face of the other. This page records that comparison (2026-11-07,
`arq-kernel` of ADR 0032 step 3, Truck `master` at `55548a2`, the revision
reviewed in [the Truck review](../references/truck.md)).

## How

[`tools/truck-compare`](../../tools/truck-compare) is a crate with a
workspace of its own, run by hand, so Truck is never a dependency of the
product, of its checks or of its dependency policy (maintainer, 2026-11-07):

```sh
cd tools/truck-compare
cargo run --release -- 1000 1e-6   # pairs, Truck's tolerance
```

- **Inputs:** pairs of boxes, each turned about a random axis by a random
  angle and moved, built alike in both kernels from the same generator as
  the kernel's tests.
- **Operations:** each pair's union, intersection and difference.
- **Our side:** volume and bounds from `Body::mass_properties`.
- **Truck's side:** volume and bounds from its triangulation, after
  dropping the mesh positions no triangle uses (its meshes keep some, and
  they widened its bounds while its volume matched ours to the last digit).
- **Agreement:** volumes within 1e-9 of the pair's volume, and bounds
  within 1e-9 of the result's size.
- **The referee where they differ:** the volume identities
  vol(A ∪ B) = vol(A) + vol(B) − vol(A ∩ B) and
  vol(A − B) = vol(A) − vol(A ∩ B), with each box's volume exact (the
  product of its sides). A kernel that misses them is wrong for that pair.

## Results, 1000 pairs (3000 operations)

| | Truck at 1e-3 | Truck at 1e-6 |
|---|---|---|
| Both give a solid, agreeing | 1685 | 467 |
| Both give a solid, differing | 56 | 2 |
| … of these, Truck missing the identities | 56 | 2 |
| Empty in ours, the boxes apart | 423 | 423 |
| Truck gives nothing or fails, ours answers | 836 | 2108 |
| Ours fails | 0 | 0 |
| Ours missing the identities | 0 | 0 |

- **Ours:** every operation answered, and every pair's three results add
  up within 1.9e-15 of the boxes' volume.
- **Where both answer:** they agree, except where Truck's own three
  results do not add up. At its looser tolerance Truck answers more but
  misses more; at the tighter one it fails more often.
- **Truck's failures:** they include its difference of two boxes apart
  given as empty, instead of the first box.

The coincident cases Truck cannot do (faces on faces, solids touching) are
covered by the kernel's own tests: `boolean_tests.rs`, among them 300
random pairs of boxes on a coarse grid, half of them turned alike, checked
against the same identities.

## Cylinders, 2026-11-16

ADR 0033 D4 asks for the same comparison for each delivery of curved
solids. For delivery 2 the tool takes a third argument:

```sh
cd tools/truck-compare
cargo run --release -- 50 1e-3 columns
```

- **Parallel columns:** pairs of upright cylinders, apart, touching,
  crossing, one inside the other and on one axis (one pair in five), at
  random heights. Their union, intersection and difference have exact
  volumes: the lens their circles share times the height they share.
- **Columns with turned boxes:** a column and a box turned about a random
  axis. No closed form exists; our intersection is the reference, and our
  union and difference must meet the volume identities with it.
- **Truck's side:** each column a disc swept up, its volume from the
  triangulation at its tolerance, 1e-3.

### Results, 50 pairs of each (300 operations)

| | Parallel columns | Columns with turned boxes |
|---|---|---|
| Ours fails | 0 | 0 |
| Ours, worst volume error | 1.3e-11 | 8.0e-14 |
| Truck answers | 27 of 150 | 72 of 150 |
| … within 10 × its tolerance of the exact volume | 20 | 55 |
| … worst volume error | 0.81 | 0.91 |
| Truck fails | 123 | 78 |

The errors are relative to the pair's volume.

- **Ours:** every operation answered, every volume exact.
- **Truck:** most operations on cylinders fail, and some of its answers
  miss by most of the volume. Where it
  answers right, it agrees with ours within its triangulation's error.
- **Time:** 20 pairs took six minutes, nearly all of it in Truck.
