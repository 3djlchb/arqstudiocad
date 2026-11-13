# 0038. Filled shapes in the drawing: wide polylines first

Date: 2026-11-12 · Status: Accepted (2026-11-12, PR #173)

## Context

A lightweight polyline keeps a width (a constant one, or a start and end
width per segment) and a thickness. AutoCAD draws the width as a filled
band and the thickness as walls rising from the polyline. Since RECTANG's
Line width and Thickness (#170), the application makes such polylines.
Drawings from AutoCAD hold many more, such as wide walls, borders and
title-block frames.

Until now the drawing had no fill at all (checked 2026-11-12):

- `Shape` (`arq-document`) holds curves, points and boxes only.
- The CPU rasterizer traces segments; the GPU's retained geometry is lines
  and points.
- Hatches are drawn by their boundary, their fill being later work.
- The viewport drew a wide polyline by its centerline. In the 3D views
  (ADR 0030) its thickness was left out ("Not yet", D1).

ADR 0013 D5.8 requires the CPU and the GPU to draw the same picture. ADR
0014 asks new designs to leave room for what comes later: hatches' solid
fills, the 2D SOLID entity, and shading.

## Decision

### D1 — A filled triangle is a shape

`Shape::Triangle([Point2; 3])`: a filled triangle, in world plan
coordinates, part of an area an entity covers. An entity that fills also
gives its curves as their own shapes, as before. Snaps, the highlight and
grips use the curves; the triangles are only filled.

- **Picking:** a click anywhere on the triangle picks its entity, and
  within the aperture of one of its edges.
- **Selection:** a crossing window takes it when the window and the
  triangle really meet, not merely their boxes. A window must hold the
  whole triangle, as AutoCAD's must hold a wide polyline's whole band.
- **Space (the 3D views):** triangles are not raised; the curves stand for
  them. A wide polyline is seen by its centerline in a view other than the
  plan, for now.

### D2 — A wide polyline's band, by `arq-geometry`

`arq_geometry::band` turns a path of vertices, each with its bulge and the
widths at its segment's start and end, into the triangles that cover it:

- **Straight segment:** a quadrilateral, its width varying linearly along
  it.
- **Arc:** a ring stepped 128 times per full turn, the same steps the 3D
  views use for a circle.
- **Joints:** two straight segments of one width meet in a miter, so a
  wide rectangle is a frame without notches. A miter longer than 8
  half-widths is not drawn, and the bands end at the vertex. Tangent arcs
  meet their neighbours with no gap of their own.
- **Which width:** a polyline's constant width when it has one, else each
  vertex's widths.
- **Plane:** the band follows the polyline's plane (mirrored included). On
  a tilted plane the polyline draws its curves only.

### D3 — Fills under strokes, on the CPU and the GPU alike

- **Order:** every fill is drawn before every stroke, so a line is never
  hidden by a band it crosses. This is not AutoCAD's draw order. It is the
  same rule on both paths, which keeps them drawing one picture.
- **CPU:** `trace` sends the triangles to its sink first. tiny-skia fills
  them without antialiasing, pixel centers deciding, as the GPU
  rasterizes a triangle.
- **GPU:** the retained geometry keeps a triangle list, drawn with the
  line shader before the lines.
- **Clipping:** both clip each triangle in `f64` before it becomes `f32`:
  the CPU to just beyond the view, the GPU to the retained region. A long
  band seen a million times closer keeps its precision, as lines do.

### D4 — Thickness: walls in the 3D views

In space, a polyline with thickness draws its curves again `thickness`
along its normal, and an edge up from each vertex: AutoCAD's wireframe
(ADR 0030 D1). A negative thickness, or a mirrored plane, goes down. The
plan is unchanged: thickness is seen only from another direction.

### D5 — Left for later

- Other entities' thickness (lines, circles, arcs, texts), and the band of
  a wide polyline in the 3D views.
- FILLMODE: AutoCAD can show wide polylines unfilled. Fills are always
  drawn here.
- The miter of a straight segment meeting an arc that is not tangent to
  it. Each band ends at the vertex there.
- A hatch's solid fill and the 2D SOLID entity. They become triangles of
  their own through D1 and D3, without a new primitive.

## Consequences

- RECTANG's and AutoCAD's wide polylines look as they do in AutoCAD in
  plan, and thick polylines rise in the 3D views.
- Every `match` on `Shape` handles fills. The CPU and the GPU gain a fill
  path each, checked against each other on the reference machine
  (`the_gpu_fills_what_the_cpu_fills`).
- Each wide segment adds two triangles, and each arc up to 256. The GPU
  uploads them once per retained region, as its lines.
- Hatches' solid fills, 2D solids and shading have a primitive to build on.
