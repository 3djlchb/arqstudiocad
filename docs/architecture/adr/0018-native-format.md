# 0018. Working format: DWG with an own internal model; native `.ascad` reserved

Date: 2026-10-13 · Status: Accepted · Keeps [0002](0002-dwg-native-format.md)
(DWG as the working format) · Builds on [0011](0011-document-model-and-transactions.md),
[0017](0017-dwg-dxf-engine-strategy.md) · Master plan §2, §5, §6, §15

## Context

The master plan 2.0 asked for a native format of our own. Before imposing
one, the maintainer took BricsCAD as the reference (2026-10-13): a CAD with
advanced BIM that keeps **DWG as its main working format**, classifies DWG
entities as building elements with BIM data embedded in the drawing, shares
project-wide data through an external project database (`.bsyslib`) when a
project has several drawings, and exchanges BIM through IFC (IFC2x3, IFC4,
property sets included). It shows that an advanced CAD/BIM does not need a
drawing extension of its own. ArqStudioCAD differs in its aim: one system
for architecture, structures, topography, building services, analysis and
visualization, which may need to keep information that fits neither DWG
nor IFC. That has to be proven, not assumed.

## Decision

### D1 — DWG is the main working format *(maintainer)*

Users work in DWG, as in BricsCAD: a drawing opened from DWG is saved to
DWG, in its own version (ADR 0017 D5 protections apply). DXF is the open
alternative. ADR 0002 stands.

### D2 — An own internal model from the start *(maintainer)*

Needed whatever the file format. ArqStudioCAD's document model is ours; the
DWG engine (AcadRust) is an adapter that reads into it and writes from it
(ADR 0017 D4). The own model takes over the content in stages; what it does
not own yet stays in the engine's database and is preserved:

| Stage | The own model owns |
|---|---|
| 1 | layers, current layer; LINE, CIRCLE, ARC (the editable kinds) |
| 2 | LWPOLYLINE, INSERT and block definitions |
| 3 | everything the platform edits; preserved data as per-object payloads |

Entity and layer ids stay stable across saves and reopenings (ADR 0011
D3.6).

### D3 — IFC for BIM exchange

IFC (through IfcOpenShell, decoupled) is the BIM exchange format, not a
substitute for the internal database.

### D4 — A real multidisciplinary test decides the native format *(maintainer)*

A real multidisciplinary project is modeled to find out what DWG can keep
(XDATA, XRECORD, dictionaries, our own objects) and what needs storage
beside it (a project database, as BricsCAD's `.bsyslib`). **If DWG plus a
project database keep everything without unacceptable losses, no native
extension is created. If the limits become important, the reserved
format of D5 is implemented.** The test is planned with phase 05 (BIM),
when there is multidisciplinary content to keep.

### D5 — The native format `.ascad` is reserved *(maintainer)*

The design below is kept so it can be implemented the day it is needed. The
extension `.ascad` was checked on 2026-10-13: no registered use (fileinfo.com
lists none; searches only return the different `.asc` and `.scad`).

```
drawing.ascad (ZIP, deflate)
├── manifest.json            format, format version (major.minor), application
│                            version, dates, source (original file, format, DWG
│                            version), every entry with its SHA-256
├── model/document.json      the own model: layers, current layer, entities it owns
│                            (stable ids, layer, color, geometry, elevation)
└── preserved/source.dwg     what the own model does not own, kept by the engine
                             in the drawing's own DWG version
```

- **Source of truth:** the own model for the kinds it owns; the preserved
  DWG for the rest; a disagreement on opening is a diagnostic, never resolved
  silently.
- **Versions:** a newer major is refused with a clear message; a newer minor
  is read, ignoring what it adds; older versions are migrated, each with a
  fixture in the tests.
- **Safety:** temporary file, read back, validate, replace (ADR 0017 D5);
  a missing or corrupt entry or a checksum mismatch fails opening with a
  diagnostic.
- **In the application:** new drawings would be saved as `.ascad`; drawings
  opened from DWG/DXF keep their format; Save As offers all three.
- Later entries (history, views, project references, BIM data, resources)
  and a project container with several models are added by later decisions.

## Consequences

- No file format of our own to maintain while DWG suffices; users keep their
  DWG workflow.
- The real work now is the **own internal model** (milestone 02.6), which
  every future format, including `.ascad`, will serialize.
- The technical MVP's "save and recover the native document" is met by
  saving and recovering the working document (DWG) **from the own model**.
- `.ascad` can be implemented quickly when D4 asks for it: its design, its
  rules and its place in the code are set.
