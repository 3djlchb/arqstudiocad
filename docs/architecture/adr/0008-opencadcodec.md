# 0008. opencadcodec as the DWG/DXF engine

Date: 2026-10-11 · Status: Accepted · Amends [0002](0002-dwg-native-format.md)

## Context

ADR 0002 made DWG the native format and planned our own reader/writer
(DXF first, then DWG read, then DWG write) — the hardest part of the project.

The reference analysis (`docs/references/opencadstudio.md`) found
[opencadcodec](https://github.com/HakanSeven12/opencadcodec): a pure-Rust crate,
**MPL-2.0**, that reads and writes DXF R12–R2018+ (ASCII and binary) and
**DWG R13–R2018+**, with 48 entity types, tables, layouts, dynamic blocks,
XDATA and ACIS data. Untouched DWG records are written back verbatim, and
reads can recover with structured diagnostics. It is the engine behind Open
CAD Studio.

Both reference projects keep the file database as their in-memory model
(Open CAD Studio with opencadcodec, IngeCAD with ezdxf) so that whatever the
application does not understand survives a save.

## Decision

1. **opencadcodec is the DWG/DXF engine.** We do not write our own.
2. **`arq-document` wraps the codec document as the model.** Entities are
   addressed by DWG handle (`EntityId`), as ADR 0002 already requires.
3. **Codec types never leave `arq-document`.** `arq-core`, `arq-render` and the
   PyO3 API see only our own types; replacing or forking the codec must not
   change the public API.
4. The dependency is pinned to an exact git revision (the crate is not on
   crates.io); `deny.toml` allows that git source explicitly.
5. We keep our **own round-trip test corpus** (open → edit → save → reopen,
   DWG and DXF, every supported version) and run it on every codec upgrade.
6. Changes we make to codec files stay under MPL-2.0 and are offered upstream.

ADR 0002 still holds for everything else: DWG native, DXF alternative,
XDATA/XRECORD under `ARQSTUDIOCAD`. Its implementation order changes to:
integrate codec (phase 3) → round-trip corpus → DWG save as default once the
corpus is green; until then, DXF remains the safe default.

## Consequences

- The largest technical risk of the project becomes an integration task.
- **Dependency on a single maintainer.** Mitigations: pinned revision, our
  corpus, and MPL-2.0 allows us to fork at any time.
- `arq-document` inherits the codec's data model; design work in phase 3
  focuses on the facade, transactions and change tracking over it.
- Recorded in `THIRD_PARTY.md` when the dependency is added (phase 3).
