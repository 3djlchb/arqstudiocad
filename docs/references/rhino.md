# Reference analysis: Rhino 3D geometry and its open parts

Analyzed 2026-10-12, for the future 3D phase. Not a dependency today.

## Rhino's geometry model

NURBS-based: almost everything is representable as NURBS, a circle exactly
as a rational NURBS curve.

| Type | Role |
|---|---|
| Curves | line, arc, polyline, NURBS; PolyCurve chains them |
| NURBS surfaces | free-form surfaces |
| Brep | solids and polysurfaces: surface faces joined by edges and vertices |
| Extrusion | lightweight Brep for simple extrusions (walls, columns) |
| SubD (Rhino 7+) | subdivision surfaces, convertible to NURBS |
| Mesh | triangles/quads for display and fabrication |
| Document tolerances | absolute and angular per document (same idea as our `Tolerance`, ADR 0010) |

## What is open and what is not

| Component | Does | License | Usable by ArqStudioCAD |
|---|---|---|---|
| openNURBS (C++), `mcneel/opennurbs` | read/write `.3dm`, geometry classes and evaluation, SubD, meshes | McNeel states MIT on its developer site; the repository's `LICENSE` file only says "All Rights Reserved" plus a warranty disclaimer | yes, but confirm the grant in writing with McNeel before porting code |
| rhino3dm (Python/JS/.NET), `mcneel/rhino3dm` | openNURBS with a RhinoCommon-style API | MIT (GitHub) | yes: `.3dm` import/export |
| RhinoCommon / C++ SDK | the real kernel: booleans, surface–surface intersection, fillets, offsets, meshing | proprietary, needs licensed Rhino | no |
| Rhino.Compute | REST server exposing RhinoCommon | runs licensed Rhino | no |
| Rhino.Inside, Grasshopper | Rhino inside other apps; visual programming | need licensed Rhino / proprietary | no |

McNeel: the open toolkit is for reading and writing 3DM files; the full
platform is the Rhino SDK. The `opennurbs.dll` shipped inside Rhino may not be
redistributed (one built from the open source may).

No Rust bindings for openNURBS were found (crates.io, 2026-10-12).

## Relevance

- **Interoperability:** a future `.3dm` import/export, most practically with
  rhino3dm from the Python layer (e.g. a plugin).
- **Ideas only:** NURBS as universal representation, Brep topology, a
  lightweight Extrusion object for walls/columns, per-document tolerances.
- **3D solids in DWG are ACIS, not Rhino NURBS**, so for a DWG-native CAD the
  opencadcodec + opencadkernel (ACIS round trip) path matters more.

## Open Rust alternatives for the 3D phase

| Project | Offers | License |
|---|---|---|
| truck (`ricosjp/truck`) | full CAD kernel: B-rep + NURBS + booleans, WebGPU rendering | Apache-2.0 |
| curvo (`mattatz/curvo`) | NURBS curves/surfaces: extrude, loft, sweep, revolve, closest point, curve intersection | MIT |
| opencadkernel | B-rep with ACIS round trip alongside opencadcodec | MPL-2.0 |

Decision deferred to an ADR at the 3D phase.
