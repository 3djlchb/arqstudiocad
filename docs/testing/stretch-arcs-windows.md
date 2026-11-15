# Partial standalone STRETCH arcs: Windows comparison

Pending maintainer decision (2026-11-15): confirm behavior in ProgeCAD on
Windows before implementing partial standalone arc reshaping on Linux.
AutoCAD 2021 is the command baseline; its public STRETCH help does not
specify the circle reconstruction rule when only one endpoint moves.
The live-preview delivery does not invent that rule or enable partial arcs.

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
performance benchmark. No independent Windows validation is claimed until
those observations have actually been supplied.
