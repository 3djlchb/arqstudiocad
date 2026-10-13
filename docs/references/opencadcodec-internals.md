# opencadcodec internals (for `arq-document`)

Analyzed 2026-10-11 at commit `8189f83` (version 0.6.0, MPL-2.0, edition 2021).
Repository: <https://github.com/HakanSeven12/opencadcodec>. Decisions taken from
this analysis: [ADR 0011](../architecture/adr/0011-document-model-and-transactions.md).

## Trial build (our toolchain, Rust 1.99)

| Check | Result |
|---|---|
| Compiles as a git dependency at `8189f83` | yes, ~80 s cold |
| `CadDocument: Send + Sync` (needed by PyO3) | yes |
| Add LINE/CIRCLE/ARC at UTM coordinates | yes |
| First-touch recorder: two edits of one line → one before-image | yes |
| DXF write to memory and read back | yes, 3 entities |
| DWG write (AC1032) and read back, same handle, edit kept | yes |

## `CadDocument`: the DWG database in memory

- `header: HeaderVariables` — drawing variables (`$INSUNITS`, `$AUNITS`, …).
- Tables (`Table<T>`, keyed by normalized name, each record with a handle):
  `layers`, `line_types`, `text_styles`, `block_records`, `dim_styles`,
  `app_ids`, `views`, `vports`, `ucss`.
- Entities: a flat store of `Arc<EntityType>` with a handle → index map;
  block records keep membership (model space, paper-space layouts, blocks).
- `objects: HashMap<Handle, ObjectType>` — dictionaries, XRECORDs, layouts…
  Unknown objects are kept verbatim for DWG round trip.

Main API: `new`, `with_version`, `add_entity` (model space),
`add_paper_space_entity`, `get_entity`, `get_entity_mut` (copy-on-write),
`remove_entity`, `entities`, `entity_count`, `allocate_handle`,
`next_handle`, `rename_layer`. Fields are public: anything outside
`arq-document` must never touch them.

## Entities

`EntityType` has 48 variants (Point, Line, Circle, Arc, Ellipse, polylines,
Text, MText, Spline, Dimension, Hatch, Insert, Viewport, Solid3D, …). Each has:

- `common: EntityCommon` — `handle`, `layer` (**by name**) and `layer_handle`,
  `color`, `line_weight`, `linetype`, `linetype_scale`, `transparency`,
  `invisible`, `extended_data` (XDATA), reactors, extension dictionary,
  `owner_handle`, raw graphic data for round trip.
- Geometry in **3D**: `Vector3` points, `thickness`, and a `normal` that
  defines the entity plane (OCS). `Line { start, end }`,
  `Circle { center, radius }`, `Arc { center, radius, start_angle, end_angle }`.
  **Angles are radians** — no conversion against ADR 0010.

## Handles

`Handle(u64)`, allocated by the document, unique, preserved by DWG save/load.
They are our `EntityId` and `LayerId` (ADR 0011).

## Undo support

- **Copy-on-write entities:** `Arc<EntityType>`; `get_entity_mut` copies only
  the touched entity (`Arc::make_mut`), so a retained before-image costs
  nothing until the entity changes.
- `get_entity_arc` / `replace_entity_arc` / `remove_entity_arc` /
  `restore_entity_arc`: move shared images in and out without deep copies;
  removal keeps block-record membership so a restore needs no rescan.
- **`EntityChangeRecorder`**: `begin_entity_change_recording` →
  mutations → `end_entity_change_recording` → `take_before_images()` returns
  each touched handle once, with its image before the first touch (`None` for
  entities created in the transaction).
- **Structure snapshots**: `snapshot_structure` / `swap_structure` save and
  exchange everything except the entity store (tables, objects, header,
  handle counter) without walking entities.
- **Caveat:** the active recorder is associated with the document's memory
  address (thread-local map). The document must not move while recording:
  keep it in a `Box`.

## I/O

DXF R12–R2018+ (ASCII and binary) and DWG R13–R2018+, read and write.
Readers are failsafe and report diagnostics; `DxfReader::from_reader`,
`DxfReader::from_file`, `DwgReader::from_file`, `DxfWriter::write_to_vec`,
`DwgWriter::write_to_file`. Default DWG output observed: AC1032 (R2018).

## Dependencies

thiserror, anyhow, nom 7, byteorder, flate2, indexmap, foldhash, encoding_rs,
bitflags, once_cell, web-time, itoa, ryu, memmap2, rayon — MIT/Apache/BSD,
compatible with GPL-3.0-or-later. `deny.toml` must allow the git source.

## Risks

Single maintainer; no tags; public mutable fields; large compile time
(~212k lines); recorder tied to the document address. Mitigations: pinned
commit, round-trip corpus, the `arq-document` boundary, `Box`ed document.

## Known issues found while using it

- **`EntityType::explode` gives LINEs in OCS coordinates** (rev 8189f83).
  Exploding an LWPOLYLINE, a 2D POLYLINE, a HATCH or a SOLID copies the
  vertices' OCS coordinates into LINE entities, whose coordinates are world
  ones by definition. With a normal of (0,0,1) nothing shows; with (0,0,-1) —
  what MIRROR often leaves — the lines land mirrored. `arq-document`'s drawing
  primitives map those LINEs through the parent's OCS
  (`a_mirrored_polyline_is_drawn_where_it_is`). Worth reporting upstream.
- **Dimensions:** `explode` approximates them; the anonymous block AutoCAD
  stored (`DimensionBase::block_name`) is what was drawn, so we draw that when
  it exists.
