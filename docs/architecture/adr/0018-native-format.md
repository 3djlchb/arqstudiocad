# 0018. Native format: an own, versioned document format

Date: 2026-10-13 · Status: Accepted · Supersedes [0002](0002-dwg-native-format.md)
(DWG as the native format) · Builds on [0011](0011-document-model-and-transactions.md),
[0017](0017-dwg-dxf-engine-strategy.md) · Master plan §2, §5, §6, §15

## Context

ADR 0002 made DWG the native format, and since ADR 0008 `arq-document` keeps
the drawing in the DWG engine's database. The master plan 2.0 reverses this:
the native format is **our own, versioned and extensible**, independent of
DWG/DXF, and an exchange file is never the project's only source of truth.
DWG cannot hold what the platform will need (history, associative views,
BIM semantics, project references) without bending a format we do not
control, and the comparison of 2026-10-13 showed how easily an engine loses
data it does not understand. "Save and recover the native document" is the
one missing task of the technical MVP. The maintainer chose to do it now
(2026-10-13).

## Decision

### D1 — An `.arqcad` file is a versioned ZIP container

```
drawing.arqcad (ZIP, deflate)
├── manifest.json            format, format version, application version, dates,
│                            source (original file, format, DWG version),
│                            the list of entries with their SHA-256
├── model/document.json      our own model: layers, current layer, entities we own
│                            (stable ids, layer, color, geometry, elevation)
└── preserved/source.dwg     everything our model does not own yet, kept by the
                             engine in the drawing's own DWG version
```

ZIP because every platform and language reads it, entries can be added
without rewriting the format, and a damaged entry is detected by its
checksum. JSON for the model in version 1: readable, diffable, easy to
migrate; a binary encoding may be added later as another entry kind under
the same manifest, after measuring. Further entries (history, views,
project references, BIM data, resources) are added by later decisions.

### D2 — Who is the source of truth

For the entity kinds the own model owns, **`model/document.json` is
authoritative**; for everything else, the preserved DWG is. On opening, the
preserved DWG is loaded into the engine as the backing store and the own
model is applied over it by entity id; a disagreement between them is
reported as a diagnostic, never resolved silently. On saving, both are
written from the same document state.

The own model grows in stages:

| Stage | Owns | Milestone |
|---|---|---|
| 1 | layers, current layer; LINE, CIRCLE, ARC (the editable kinds) | 02.6 (closes the MVP) |
| 2 | LWPOLYLINE, INSERT and block definitions | after 03.3 |
| 3 | everything the platform edits; preserved data becomes per-object payloads instead of a whole DWG | 02.8 and later |
| 4 | the project container: several models, drawings, views, sheets, resources | phase 04 |

### D3 — Identity

Entity and layer ids are stable across saves and reopenings, as they are
today (ADR 0011 D3.6: the DWG handle). The own model stores them; DWG export
keeps them as handles where the engine allows it.

### D4 — Versions

`manifest.json` carries `format_version` as `major.minor`. A reader refuses
a newer **major** version with a clear message, and reads a newer **minor**
version, ignoring entries and fields it does not know (minor versions only
add). Every older version is read by explicit migrations, each with a
fixture file in the tests. Version 1.0 is the one this ADR defines.

### D5 — Safe save and checks

Saving `.arqcad` follows ADR 0017 D5: write to a temporary file, read it
back, validate it against the document (entities, layers, checksums), then
replace the original; on failure the original is untouched. A missing or
corrupt entry, a checksum mismatch or an unknown major version makes
opening fail with a diagnostic; a partial file is never opened as a valid
empty drawing.

### D6 — Formats in the application

- A **new** drawing is saved as `.arqcad`.
- A drawing **opened from DWG or DXF** keeps saving to its own format with
  Save, as today; Save As offers `.arqcad`, DWG and DXF.
- Opening an `.arqcad` file restores the drawing exactly as it was saved:
  entities, ids, layers, current layer and the modified flag cleared.

## Consequences

- The MVP's last task is implementable now without rewriting the core: in
  stage 1 the engine's database stays the backing store, and the own model
  takes authority over the kinds we edit.
- DWG/DXF become exchange formats; their losses are an adapter's problem,
  reported by ADR 0017's protections, not the project's.
- Stage 3 is a real change of the document model; it gets its own ADR when
  it starts.
- The extension `.arqcad` and its MIME type are registered by the
  packaging (phase 11).
