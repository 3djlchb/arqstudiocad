# 0002. DWG as native format, DXF as alternative

Date: 2026-10-11 · Status: Accepted · Implementation amended by
[0008](0008-opencadcodec.md) (opencadcodec replaces our own reader/writer)

## Context

Architecture and engineering files are exchanged as DWG. A CAD whose native
format is something else forces every user through an import/export step that
loses data. Commercial alternatives (BricsCAD, ZWCAD) use DWG natively.

DWG is a closed format. Reading and writing it relies on reverse-engineered
knowledge: the Open Design Alliance specification, GNU LibreDWG (GPL-3.0+), and
Open CAD Studio's Rust implementation (GPL-3.0). DXF is documented and open.

## Decision

- `.dwg` is the native file format; `.dxf` is fully supported as an alternative.
- The document model mirrors DWG from the start (phase 3):
  - `EntityId` is the DWG handle (u64), persistent and never reused;
  - layers, linetypes, text styles, dimension styles and blocks are symbol-table
    records with handles;
  - model space and paper space layouts are block records;
  - colours follow ACI / true colour; lineweights follow DWG values.
- Data DWG cannot represent goes into XDATA / XRECORD dictionaries under the
  registered application name `ARQSTUDIOCAD`, including our schema version.
- Implementation order (phase 8): DXF read/write → DWG read → DWG write.
  Until DWG writing is reliable, Save writes DXF.
- The target DWG version to write (e.g. R2018 / AC1032) is decided in phase 8.

## Consequences

- Writing DWG faithfully is the hardest part of the project; it is scheduled
  after a working DXF path so users are never stuck.
- The model cannot invent concepts DWG has no place for without an XDATA plan.
- Reusing LibreDWG or Open CAD Studio code is license-compatible (GPL-3.0) and
  must be recorded in `THIRD_PARTY.md`.
