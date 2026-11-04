# 0030. Wireframe views: the model seen from AutoCAD's preset directions

Date: 2026-11-03 · Status: Proposed

## Context

The viewport shows the plan only: the camera is a center and a scale over
the XY plane (`arq-render`'s `Camera`), the scene holds each entity's plan
shapes, and picking, selection, object snaps and grips all work in plan
(ADR 0016). Solids are drawn in plan by their edges and isolines (ADR 0024
step 2), so a box looks like a rectangle and a cylinder like a circle.

ADR 0024's step 4 creates solids (primitives, extrusion, revolution). To
see what it creates, the maintainer chose to add views from other
directions first, in the present viewport and without the GPU (02.7, on
hold for the reference machine), as a step before creating
(maintainer, 2026-11-03). Shading, hidden lines and the user coordinate
systems of a later 02.8 step (ADR 0024 D5) are not part of it.

## Decision

Each choice marked *(maintainer)* was made by the maintainer on
2026-11-03.

### D1 — AutoCAD's preset views, in wireframe *(maintainer)*

- **The views:** Top (the plan), Bottom, Front, Back, Left and Right, and
  the four isometric views, SW, SE, NE and NW, each from AutoCAD's
  direction (SW Isometric looks from (-1, -1, 1)).
- **Projection:** parallel, Z up on screen in every view but Top and
  Bottom; no perspective.
- **What is drawn:** every model-space entity in 3D, as a wireframe:
  - solids by their edges and isolines (ADR 0024 step 2);
  - 2D entities in their plane, at their elevation (ADR 0011 D3.3), their
    thickness as AutoCAD draws it when it is not zero;
  - texts, blocks and everything else by the same shapes as in plan,
    placed in 3D.
- Nothing is hidden or shaded: that waits for the GPU viewport (02.7).

### D2 — The view belongs to the drawing

- Each drawing keeps its view, as its zoom.
- It is read from and written to the drawing's active viewport (the
  `*Active` VPORT's view direction and target), as AutoCAD keeps it, so a
  drawing saved in an isometric view opens in it, here and in AutoCAD.
- The plan stays the default, and plan work is unchanged.

### D3 — Rust projects, as it draws today

- The scene holds each entity's 3D wireframe (2D shapes placed at their
  elevation and normal, solids' edges and isolines). The camera gains a
  view direction, and projects in `f64` relative to the view, as now.
- **In plan the path stays as it is:** no projection, same speed.
- The spatial index (ADR 0021) stays in plan. In other views, queries use
  the projected boxes; an index per view comes only if the reference
  machine needs it.
- Zoom (wheel, window, extents) works in every view, on what the view
  shows.

### D4 — Working in a view other than the plan *(maintainer)*

- **Selection** (pick, window, crossing) works in every view, on the
  projected wireframe.
- **Points picked on screen** fall on the XY plane (Z = 0), as AutoCAD's
  with the World UCS: the line of sight through the cursor meets the
  plane. Where the plane is seen edge on (Front, Back, Left, Right), a
  picked point cannot be placed: the command asks for it to be typed and
  says so.
- **Typed points** work in every view, as in plan.
- **Object snaps, ORTHO and grips** stay plan-only in this step: in other
  views they are suspended, and the status bar says so. They follow the
  user coordinate systems, with 3D points.

### D5 — Commands and where they are

- **`-VIEW`** (Spanish `-VISTA`) takes AutoCAD's options for these views:
  Top, Bottom, Front, Back, Left, Right, SWiso, SEiso, NEiso and NWiso.
  **PLAN** (Spanish `PLANTA`) goes back to the plan. AutoCAD's VIEW, its
  View Manager with named views, comes later.
- **Ribbon:** the View tab gains a Views group, its views in a drop-down
  button.
- **Menu:** View > 3D Views lists them, as AutoCAD's.
- Changing the view is not an edit: no undo step, the drawing is not
  modified (only saved with its view, D2).

### D6 — What waits

- Shading and hidden lines: the GPU viewport (02.7).
- Orbit, perspective and named views.
- User coordinate systems, and with them snaps, ORTHO, grips and picked
  points in any view (ADR 0024 D5).
- Viewports in paper space.

## Consequences

- ADR 0024's step 4 can show what it creates, before the GPU viewport,
  on the reference machine's floor.
- The document gains a 3D drawing of every entity beside its plan one; it
  is derived data, never saved.
- 2D entities on tilted planes (read-only, ADR 0011 D3.3) are drawn where
  they lie in 3D; the plan shows them as now.
- Selection and zoom gain a projection; snaps and grips keep their plan
  code until the user coordinate systems arrive.
