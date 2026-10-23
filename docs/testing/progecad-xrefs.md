# ProgeCAD Xref validation exercise

Status: ready to execute; **no external validation result recorded yet**.
Milestones 03.4/03.5. The maintainer confirmed ProgeCAD 2026 on 2026-10-22,
but does not yet have an authored Xref set. Record the edition, exact build,
language and DWG save version before starting; these remain unknown.
The application version does not establish the DWG save version.

This exercise creates original drawings in ProgeCAD. Do not substitute the
synthetic DXFs in `tests/fixtures/xrefs/`: those test adapter metadata, not
independently authored DWG files or external application behavior.

## 1. Author the source set

Suggested folder: `D:\PROGRAMACION\EJERCICIOS DWG\Xrefs-ProgeCAD\source`.
Create it yourself in Explorer. Keep all six drawings together. Use the same
units in every drawing, world UCS, model space, base point (0,0,0), and the
same DWG save version. Record INSUNITS and any automatic insertion-unit scaling.

1. **child.dwg:** draw a line from (0,0,0) to (3,4,0) and a circle centered at
   (6,0,0), radius 1. Save and close it.
2. **attachment.dwg:** draw a local circle centered at (-5,-5,0), radius 1,
   as a marker that is not part of the reference. Save the drawing first.
   Open `XREF`, choose Attach, select `child.dwg`, and choose Attachment.
   Enter insertion (10,20,0), scale (2,2,1), and rotation 30 degrees; disable
   on-screen entry for these values. Use a filename-only path if available;
   the manual describes clearing Retain Path for files in the same folder.
   Confirm the stored path in the manager and record what this version uses.
3. **overlay.dwg:** create the same local marker and attach the same child
   with the same transform, choosing Overlay instead. Both host drawings
   should show the child's geometry when opened directly.
4. **master.dwg:** attach `attachment.dwg` at (0,0,0) and `overlay.dwg` at
   (100,0,0), both as Attachment, scale 1, rotation 0. Record the nested
   reference tree. The attachment branch should include its child; the
   overlay branch should retain its local marker without propagating its
   overlaid child. Record any discrepancy rather than adjusting the drawing
   to conceal it.
5. **missing.dwg:** Save As a copy of `attachment.dwg`, then change only its
   stored reference path to `absent-child.dwg` (a file that does not exist).
   Record the warning/status and save. Do not delete or rename the real child.
6. **unloaded.dwg:** Save As another copy of `attachment.dwg`, unload its
   reference in the manager, and save. Record that the reference remains
   listed despite not being displayed. Keep this separate from the missing case.

Use Zoom Extents and inspect the Xref manager after reopening each file.
For the direct attachment/overlay, the transformed line should run from
(10,20) to approximately (11.196152423,29.928203230); its length is 10.
The child's circle radius becomes 2. Confirm these measurements in ProgeCAD,
not by appearance alone. The host's local circle stays radius 1.

The command and dialog workflow follows the official [attachment manual](https://www.progesoft.com/products/progecad-professional/manual?mp=attach-external-reference-dialog-box).
The [Xref Manager reference](https://www.progesoft.com/products/progecad-professional/manual?mp=command-reference%2Falphabetical-list-of-commands%2Fxref-manager)
describes unload and reload; [path management](https://www.progesoft.com/products/progecad-professional/manual?mp=working-with-other-files-in-your-drawings%2Fworking-with-external-references%2Fchanging-the-path-for-external-references)
describes missing-file handling. Labels can differ by installed version/language.
Do not use Bind or Explode in this exercise: they change what is being tested.

## 2. Freeze the baseline

Once all drawings are closed, retain the source directory unchanged. Record:

- ProgeCAD edition/build, OS, units and save version;
- filenames, SHA-256 values and which application authored each file;
- each stored path, mode and loaded/unloaded state;
- screenshots of the reference trees, warnings and measured geometry;
- any unexpected result, including whether the application repaired a file.

Keep private drawings and raw screenshots outside tracked repository files.
Only add drawings to the repository later with explicit sharing permission.
The small fixture set should contain no real client data.

## 3. Measure in ArqStudioCAD

From the repository root, after the files exist:

```powershell
uv run python tools/compatibility_matrix.py "D:\PROGRAMACION\EJERCICIOS DWG\Xrefs-ProgeCAD\source" --format json --output target/progecad-xrefs-matrix.json
uv run python tools/compatibility_matrix.py "D:\PROGRAMACION\EJERCICIOS DWG\Xrefs-ProgeCAD\source" --output target/progecad-xrefs-matrix.md
```

This tool measures reading and safe saving; it cannot certify Xref resolution.
Its temporary outputs are deleted, so they cannot be used for the next step.
A blocked save is a result to record, never an instruction to accept losses.
The current application need not display the external geometry.

## 4. Retain output copies and reopen them in ProgeCAD

For each host and each output format, create a separate working folder under
ignored `target/progecad-xrefs-review/`, copying the complete source set into
it. Example: `attachment-dxf/` contains the six original DWGs as dependencies.
Open that folder's `attachment.dwg` in ArqStudioCAD and Save As
`attachment-output.dxf` in the **same folder**. Repeat for a DWG output and
the other hosts. Always use a new output filename, never the original source.

Keep stored reference filenames unchanged, and leave their copied DWG
dependencies available beside the new host. A converted DXF host may still
reference DWG children: this is a per-host preservation test, not a conversion
of the entire reference graph. Do not repath or manually repair the output
before observing it. If the baseline uses absolute paths, record that the
output still targets the baseline; do not claim package portability.

Close the source host and open each output in ProgeCAD. Compare against the
baseline: reference count/tree, stored paths and modes, loaded state, host
marker, insertion transform, measured child geometry and warning text. Record
repair prompts before responding. Never report an unreopened output as passed.
Capture the source and output file hashes with the observations.

For reload behavior, use only a disposable copied package. Change its child's
line endpoint to (6,8,0), save and close the child, then reload the parent in
ProgeCAD. The transformed line length should become 20. This characterizes
the external application's behavior; it does not prove ArqStudioCAD reload.

## 5. Results and exit criteria

Use one row per host/format; duplicate this table into a local report:

| Host / output | Source hash | Output hash | Read / save | Tree / path / mode | Transform / geometry | Warning / repair | Verdict / evidence |
|---|---|---|---|---|---|---|---|
| attachment / DWG | pending | pending | pending | pending | pending | pending | NOT RUN |
| attachment / DXF | pending | pending | pending | pending | pending | pending | NOT RUN |
| overlay / DWG, DXF (separate rows) | pending | pending | pending | pending | pending | pending | NOT RUN |
| master / DWG, DXF (separate rows) | pending | pending | pending | pending | pending | pending | NOT RUN |
| missing / DWG, DXF (separate rows) | pending | pending | pending | pending | pending | pending | NOT RUN |
| unloaded / DWG, DXF (separate rows) | pending | pending | pending | pending | pending | pending | NOT RUN |

Allowed verdicts: PASS (all stated comparisons made), FAIL (difference),
BLOCKED (save refused), NOT RUN. An expected unresolved reference is not
automatically a failure; losing its metadata or misreporting it is.
Record timings and source changes separately from fidelity results.
Add a portability case by copying the completed package to another directory;
absolute-path behavior and foreign-platform paths require separate cases.

03.5 remains open until real outputs have been independently opened and their
observations recorded. This exercise does not cover every DWG version, BIM
semantics, clipped references, cyclic graphs, or all visual styling.
