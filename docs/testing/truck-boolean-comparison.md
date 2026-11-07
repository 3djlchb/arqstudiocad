# Planar booleans compared with Truck

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
