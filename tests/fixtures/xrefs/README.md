# Synthetic Xref corpus

Original, minimal ASCII DXF fixtures authored for this repository (GPL-3.0-or-later).
No third-party drawings or private project data are included. The files use
AC1009 group-code syntax; this is a controlled metadata corpus, not independent
CAD-application evidence.

| Host | Reference | Mode | Purpose |
|---|---|---|---|
| `attachment.dxf` | `child.dxf` | Attachment | Existing relative path |
| `overlay.dxf` | `child.dxf` | Overlay | Distinguish overlay from attachment |
| `missing.dxf` | `absent.dwg` | Attachment | Preserve an unresolved path; deliberately absent |
| `nested.dxf` | `attachment.dxf` | Attachment | Two-level file graph ending at `child.dxf` |

Each host has one INSERT at (10, 20, 0), scale (2, 2, 1), rotation 30 degrees.
The child contains one line from (0, 0, 0) to (3, 4, 0). Host blocks have no
cached external geometry. A reader that only preserves the reference cannot
therefore be mistaken for a reader that displays the child.

`cargo test -p arq-document xref_fixture_metadata_roundtrips` checks the source
reference path and flags, exclusion from insertable local blocks, INSERT
transformation, and preservation through both safe-save formats and reopening.
It inspects the engine metadata inside the adapter's tests, without exposing
codec types in the public API. Copies are temporary; source drawings are read only.

The existing report tool accepts this folder without modification:

```sh
uv run python tools/compatibility_matrix.py tests/fixtures/xrefs --output target/xref-compatibility-matrix.md
```

Its five clean reads, if successful, are file-level results, not five resolved
Xrefs: four files are hosts and one is a child. Only the Rust assertions check
reference metadata. Neither test proves path resolution, nested loading,
overlay propagation, missing-file diagnostics, visual fidelity, cyclic graph
handling, or compatibility with an independent CAD application. DWG outputs
are generated from these DXF inputs; externally authored DWG hosts remain needed.
