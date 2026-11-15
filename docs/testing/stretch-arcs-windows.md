# Partial standalone STRETCH arcs: Windows comparison

Observed in ProgeCAD on Windows on 2026-11-15 (below). The reconstruction
rule they show awaits the maintainer's approval before partial standalone
arc reshaping is implemented. AutoCAD 2021 is the command baseline; its
public STRETCH help does not specify the circle reconstruction rule when
only one endpoint moves. The live-preview delivery does not invent that
rule or enable partial arcs.

Use only a newly created synthetic drawing, not a private drawing. Record
ProgeCAD version, file format/version, units and the exact commands/picks.
Keep each case separate or undo before the next one. Use Top view, world
coordinates and no snap/ortho constraint that changes the specified points.
Do not change global desktop settings through automation.

1. Create a standalone ARC with center (0,0), radius 5, start angle 0 degrees,
   end angle 90 degrees. Confirm endpoints (5,0) and (0,5).
2. STRETCH with Crossing corners (-1,4) and (1,6), selecting only the end
   near (0,5). Base (0,0), second point (3,2).
3. Record the resulting endpoints, center, radius, start/end angles,
   included angle and normal/elevation using the properties/LIST output.
   Record whether the unselected endpoint stayed at (5,0) and the selected
   endpoint moved to (3,7). Do not infer a rule from endpoints alone.
4. Undo. Repeat with only the start near (5,0) selected, using crossing
   corners (4,-1) and (6,1), with the same displacement. Record all fields.
5. Repeat for a 180-degree arc and a 270-degree arc, selecting only one
   endpoint each time. Include a nonzero elevation case. Record any refused
   or degenerate result explicitly.
6. Control: select the whole arc individually and move by (3,2). Confirm
   both endpoints and center translate, radius and included angle retained.
7. Save the synthetic before/after cases to DWG and DXF and reopen in
   ProgeCAD. Confirm the reported geometry survives. Only public synthetic
   cases may be shared through GitHub, with the maintainer's approval.

Attach the observations to the relevant PR/issue from the Windows agent.
The maintainer then approves a precise reconstruction rule and expected
coordinates for Rust tests. This comparison is behavioral evidence, not a
performance benchmark.

## Observations (2026-11-15, Windows agent)

**Environment.**
- ProgeCAD 2026 Professional ESN (`ACADVER` 19.0i) on Windows 11 Pro.
- The development laptop, with the maintainer's permission to automate ProgeCAD.
- A blank drawing from ProgeCAD's own `pcadISO (Metric).dwt`: `INSUNITS` 4 (mm), `MEASUREMENT` 1.
- World UCS, Top view, `OSMODE`, `ORTHOMODE` and `SNAPMODE` 0 for the run and restored after it.

**Method.**
- `tools/progecad_command_compare.lsp` (`arq:compare-02-14`) builds each arc with ActiveX `AddArc`.
- It runs ProgeCAD's own `STRETCH` with the guide's crossing corners, base and second point.
- It writes every field: handle, center, radius, start and end angles, included angle (end − start; ProgeCAD has no `TotalAngle`), normal, start and end points.
- One case at a time on a cleared model space.
- The tool, run a second time from the repository, gave the same results in every case.

All cases start from center (0,0), radius 5, start angle 0°, and use base (0,0) and second point (3,2) unless the table says otherwise.

| Case | Selected end, crossing corners | After: center | radius | start° | end° | included° | start point | end point |
|---|---|---|---|---|---|---|---|---|
| A1 90° | end (0,5): (-1,4)–(1,6) | (0.3542858741, 2.4583673926) | 5.2560660172 | 332.1135992961 | 59.7771925057 | 87.6635932096 | (5,0) | (3,7) |
| A2 90° | start (5,0): (4,-1)–(6,1) | (2.0692751777, -1.6485995261) | 6.9631727984 | 31.5999528450 | 107.2879567159 | 75.6880038709 | (8,2) | (0,5) |
| A3 180° | end (-5,0): (-6,-1)–(-4,1) | (1.8227973253, 2.1297906385) | 3.825 | 326.1646634679 | 181.9445447302 | 215.7798812623 | (5,0) | (-2,2) |
| A3b 180° | start (5,0): (4,-1)–(6,1) | (1.7775043613, -0.8037783482) | 6.825 | 24.2557288003 | 173.2365957248 | 148.9808669245 | (8,2) | (-5,0) |
| A4 270° | end (0,-5): (-1,-6)–(1,-4) | (0.6074094784, 0.7617270144) | 4.4581475452 | 350.1620716040 | 302.4577933440 | 312.2957217401 | (5,0) | (3,-3) |
| A4b 270° | start (5,0): (4,-1)–(6,1) | (2.2793799644, 0.4664228978) | 5.9226136393 | 15.0069296776 | 247.3649206538 | 232.3579909763 | (8,2) | (0,-5) |
| A5 90°, elevation 2 | end (0,5): (-1,4)–(1,6) | (0.3542858741, 2.4583673926, 2) | 5.2560660172 | 332.1135992961 | 59.7771925057 | 87.6635932096 | (5,0,2) | (3,7,2) |
| A6 control, arc picked | whole arc | (3, 2) | 5 | 0 | 90 | 90 | (8,2) | (3,7) |
| A7 control, both ends in | (-1,-1)–(6,6) | (3, 2) | 5 | 0 | 90 | 90 | (8,2) | (3,7) |
| A8 end onto start | end (0,5), displacement (5,-5) | (0, 0) | 5 | 0 | 90 | 90 | (5,0) | (0,5) |

In every case the arc kept its handle and its normal (0,0,1), and no other entity was created.

**What these show:**
- **The endpoints.** The unselected endpoint stays where it was, and the selected one moves by the displacement. The arc stays an ARC with the same handle, normal and elevation (A5 keeps Z = 2). It still runs counterclockwise from its start to its end.
- **What changes.** The radius, the center and the included angle all change, so none of them is what ProgeCAD keeps.
- **What is kept: the sagitta.** The sagitta is the distance from the chord's midpoint to the arc's midpoint, h = r(1 − cos(θ/2)) for an included angle θ. Before and after, in every case, it agrees to 5·10⁻¹¹, the precision of the log:
  - 1.4644660941 for the 90° arcs;
  - 5 for the 180° arcs;
  - 8.5355339059 for the 270° arcs.
- **Controls (A6, A7).** The arc moved whole: center (3,2), radius and angles unchanged.
- **Degenerate case (A8).** Moving the end onto the start leaves no chord: ProgeCAD left the arc unchanged and reported no error.

### Rule proposed for approval

For a standalone ARC with exactly one endpoint selected by STRETCH's crossing selection:

1. Move that endpoint by the displacement; keep the other, the normal, the elevation, the direction (counterclockwise from start to end in the arc's own coordinate system) and the entity.
2. Keep the sagitta. With the old chord length c and included angle θ, h = (c/2)·tan(θ/4). With the new chord c′, the new bulge is b′ = 2h/c′, the included angle θ′ = 4·atan(b′), and the radius r′ = c′ / (2 sin(θ′/2)). The center lies on the chord's perpendicular bisector, on the side that keeps the arc counterclockwise.
3. If the new chord is zero, leave the arc unchanged, as ProgeCAD does. Refusing the stretch with a message is the alternative for the maintainer to choose.
4. Both endpoints selected, or the arc picked as an object: translate it whole.

Rule 2 reproduces A1–A4b to 5·10⁻¹¹, the precision of the log. The table's rows are the expected coordinates for the Rust tests.

### Reopening

**How.** Every case of this page and of [the BREAK comparison](break-closed-polylines-windows.md) was saved by ProgeCAD before and after, as DWG 2018 (AC1032) and as DXF 2018: 84 files. Each was then reopened by a separate ProgeCAD from its command line and described again with the same fields. Opening with `vla-Open` from a script hangs ProgeCAD.

**Result.** All 84 opened.
- 83 match the description written when the case was run, field for field and handle for handle. That includes every STRETCH file.
- **The exception is a ProgeCAD defect in its DXF writer:** B7 after BREAK, saved as DXF, lost its widths. Its DWG kept them. See [the BREAK comparison](break-closed-polylines-windows.md#a-progecad-dxf-defect).

**The drawings themselves are not committed.** They are synthetic and made from ProgeCAD's own template. The tool reproduces them.
