//! The editing operations of a [`CadDocument`]. Each is one transaction:
//! one undo step, or nothing at all when it fails (contract §6).
//!
//! Operations taking ids remove duplicates; an empty list changes nothing.
//! Entities on a locked layer cannot be modified or erased; new entities may
//! still be drawn on a locked current layer, as in AutoCAD.

use std::collections::BTreeMap;

use arq_document::{
    BlockData, BlockDefinition, BlockObjects, ChangeSet, ClashChoice, Color, Combined, Currencies,
    Document, DocumentError, Entity, EntityId, Extruded, LayerId, MatchSettings, PlanGeometry,
    PolylineStyle, Revolved,
};
use arq_geometry::{
    Arc, Chain, Circle, CrossingArea, Ellipse, Geometry2, GeometryError, JoinPath, Line, Point2,
    Point3, SelectionPolygon, Spline, SplineFit, Tolerance, Transform2, Vector2, break_curve,
    chain_paths, cutting_points, extend, join_arcs, join_elliptical_arcs, join_groups, join_lines,
    trim,
};

use crate::error::{CadError, CadResult};
use crate::history::CadDocument;

impl CadDocument {
    /// Draws a line on the current layer.
    pub fn add_line(&mut self, start: Point2, end: Point2) -> CadResult<(EntityId, ChangeSet)> {
        self.add_geometry(Line::new(start, end)?.into())
    }

    /// Makes a box between two opposite corners, its sides along the axes,
    /// on the current layer (BOX, ADR 0024 step 4a).
    pub fn add_box(
        &mut self,
        corner: Point3,
        opposite: Point3,
    ) -> CadResult<(EntityId, ChangeSet)> {
        self.transact("BOX", |doc| Ok(doc.add_box(corner, opposite)?))
    }

    /// Makes a cylinder standing on its base circle at `center`, `height`
    /// along Z (negative: downward), on the current layer (CYLINDER, ADR
    /// 0024 step 4b).
    pub fn add_cylinder(
        &mut self,
        center: Point3,
        radius: f64,
        height: f64,
    ) -> CadResult<(EntityId, ChangeSet)> {
        self.transact("CYLINDER", |doc| {
            Ok(doc.add_cone(center, radius, radius, height)?)
        })
    }

    /// Makes a cone standing on its base circle at `center`, its top of
    /// radius `top` (0: a point), on the current layer (CONE, ADR 0024
    /// step 4b).
    pub fn add_cone(
        &mut self,
        center: Point3,
        base: f64,
        top: f64,
        height: f64,
    ) -> CadResult<(EntityId, ChangeSet)> {
        self.transact("CONE", |doc| Ok(doc.add_cone(center, base, top, height)?))
    }

    /// Extrudes the closed profiles among `ids` `height` along Z into
    /// solids on their layers, erasing them (EXTRUDE, ADR 0024 step 4c);
    /// one undo step, none when nothing was extruded.
    pub fn extrude_entities(
        &mut self,
        ids: &[EntityId],
        height: f64,
    ) -> CadResult<(Extruded, ChangeSet)> {
        self.transact("EXTRUDE", |doc| Ok(doc.extrude_entities(ids, height)?))
    }

    /// Turns the closed profiles among `ids` `angle` radians about the axis
    /// from `axis_start` to `axis_end` into solids on their layers, erasing
    /// them (REVOLVE, ADR 0024 step 4d); one undo step.
    pub fn revolve_entities(
        &mut self,
        ids: &[EntityId],
        axis_start: Point2,
        axis_end: Point2,
        angle: f64,
    ) -> CadResult<(Revolved, ChangeSet)> {
        self.transact("REVOLVE", |doc| {
            Ok(doc.revolve_entities(ids, axis_start, axis_end, angle)?)
        })
    }

    /// Unites the 3D solids among `ids` into the first of them, erasing
    /// the others (UNION, ADR 0032 D5); one undo step, none when nothing
    /// changed.
    pub fn union_solids(&mut self, ids: &[EntityId]) -> CadResult<(Combined, ChangeSet)> {
        self.transact("UNION", |doc| Ok(doc.union_solids(ids)?))
    }

    /// Leaves in the first of the 3D solids among `ids` what they all
    /// share, erasing the others (INTERSECT, ADR 0032 D5); one undo step,
    /// none when nothing changed.
    pub fn intersect_solids(&mut self, ids: &[EntityId]) -> CadResult<(Combined, ChangeSet)> {
        self.transact("INTERSECT", |doc| Ok(doc.intersect_solids(ids)?))
    }

    /// Unites the 3D solids among `from` into the first of them less every
    /// 3D solid among `subtract`, erasing the others (SUBTRACT, ADR 0032
    /// D5); one undo step, none when nothing changed.
    pub fn subtract_solids(
        &mut self,
        from: &[EntityId],
        subtract: &[EntityId],
    ) -> CadResult<(Combined, ChangeSet)> {
        self.transact("SUBTRACT", |doc| Ok(doc.subtract_solids(from, subtract)?))
    }

    /// Makes a sphere around `center` on the current layer (SPHERE, ADR
    /// 0024 step 4b).
    pub fn add_sphere(&mut self, center: Point3, radius: f64) -> CadResult<(EntityId, ChangeSet)> {
        self.transact("SPHERE", |doc| Ok(doc.add_sphere(center, radius)?))
    }

    /// Makes a torus around the Z axis through `center` on the current
    /// layer (TORUS, ADR 0024 step 4b).
    pub fn add_torus(
        &mut self,
        center: Point3,
        radius: f64,
        tube: f64,
    ) -> CadResult<(EntityId, ChangeSet)> {
        self.transact("TORUS", |doc| Ok(doc.add_torus(center, radius, tube)?))
    }

    /// Makes a wedge in the box between two opposite corners, upright at
    /// `first`'s X, on the current layer (WEDGE, ADR 0024 step 4b).
    pub fn add_wedge(
        &mut self,
        first: Point3,
        opposite: Point3,
    ) -> CadResult<(EntityId, ChangeSet)> {
        self.transact("WEDGE", |doc| Ok(doc.add_wedge(first, opposite)?))
    }

    /// Draws an ellipse or elliptical arc on the current layer, in the
    /// World plane at elevation 0 (ELLIPSE, ADR 0040).
    pub fn add_ellipse(&mut self, ellipse: Ellipse) -> CadResult<(EntityId, ChangeSet)> {
        self.transact("ELLIPSE", |doc| Ok(doc.add_ellipse(ellipse, 0.0)?))
    }

    /// Draws a spline by its control vertices on the current layer, in the
    /// World plane at elevation 0 (SPLINE's CV method, ADR 0042).
    pub fn add_spline(&mut self, spline: &Spline) -> CadResult<(EntityId, ChangeSet)> {
        self.transact("SPLINE", |doc| Ok(doc.add_spline(spline, 0.0)?))
    }

    /// Draws the spline through `fit`'s points on the current layer, in the
    /// World plane at elevation 0 (SPLINE's Fit method, ADR 0042).
    pub fn add_fit_spline(&mut self, fit: &SplineFit) -> CadResult<(EntityId, ChangeSet)> {
        self.transact("SPLINE", |doc| Ok(doc.add_fit_spline(fit, 0.0)?))
    }

    /// Draws a circle on the current layer.
    pub fn add_circle(&mut self, center: Point2, radius: f64) -> CadResult<(EntityId, ChangeSet)> {
        self.add_geometry(Circle::new(center, radius)?.into())
    }

    /// Draws an arc on the current layer, counterclockwise from `start_angle`
    /// to `end_angle` (radians).
    pub fn add_arc(
        &mut self,
        center: Point2,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
    ) -> CadResult<(EntityId, ChangeSet)> {
        self.add_geometry(Arc::new(center, radius, start_angle, end_angle)?.into())
    }

    /// Draws any plan geometry on the current layer, at elevation 0.
    pub fn add_geometry(&mut self, geometry: Geometry2) -> CadResult<(EntityId, ChangeSet)> {
        self.transact(geometry.kind().name(), |doc| {
            let layer = doc.current_layer().id();
            Ok(doc.add_entity(PlanGeometry::new(geometry), layer)?)
        })
    }

    /// Erases entities of any type (ERASE).
    pub fn delete_entities(&mut self, ids: &[EntityId]) -> CadResult<ChangeSet> {
        self.edit_entities("ERASE", ids, |doc, e| Ok(doc.delete_entity(e.id())?))
    }

    /// Moves entities by `offset` (MOVE).
    pub fn move_entities(&mut self, ids: &[EntityId], offset: Vector2) -> CadResult<ChangeSet> {
        self.transform_entities("MOVE", ids, Transform2::translation(offset))
    }

    /// Rotates entities by `angle` radians counterclockwise around `center`
    /// (ROTATE).
    pub fn rotate_entities(
        &mut self,
        ids: &[EntityId],
        center: Point2,
        angle: f64,
    ) -> CadResult<ChangeSet> {
        self.transform_entities("ROTATE", ids, Transform2::rotation(center, angle)?)
    }

    /// Scales entities by `factor` around `center` (SCALE). Elevations are
    /// kept.
    pub fn scale_entities(
        &mut self,
        ids: &[EntityId],
        center: Point2,
        factor: f64,
    ) -> CadResult<ChangeSet> {
        self.transform_entities("SCALE", ids, Transform2::scaling(center, factor)?)
    }

    /// Mirrors entities across the infinite line through `axis` (MIRROR).
    pub fn mirror_entities(&mut self, ids: &[EntityId], axis: Line) -> CadResult<ChangeSet> {
        self.transform_entities("MIRROR", ids, Transform2::mirror(axis))
    }

    /// Copies entities, moving the copies by `offset` (COPY); returns the
    /// copies' ids, in the order of `ids` without repeats. Lines, circles,
    /// arcs, blocks and polylines can be copied (ADR 0016, D1); any other
    /// entity fails the whole operation. The copies take the originals'
    /// layer, color and other properties.
    pub fn copy_entities(
        &mut self,
        ids: &[EntityId],
        offset: Vector2,
    ) -> CadResult<(Vec<EntityId>, ChangeSet)> {
        let ids = distinct(ids);
        let t = Transform2::translation(offset);
        self.transact("COPY", |doc| {
            let mut copies = Vec::with_capacity(ids.len());
            for id in ids {
                let e = doc.entity(id).ok_or(CadError::EntityNotFound(id))?;
                check_unlocked(doc, &e)?;
                check_transformable(&e)?;
                let copy = doc.copy_entity(id)?;
                doc.transform_entity(copy, t)?;
                copies.push(copy);
            }
            Ok(copies)
        })
    }

    /// Makes independent copies on a rectangular grid (ARRAYRECT), leaving
    /// the original cell in place. Spacing is signed; `angle` rotates the
    /// grid axes, in radians, without rotating the objects. IDs are returned
    /// row first, then column, then distinct source order, excluding originals.
    /// Like legacy ARRAY, at least two and at most 100,000 cells are allowed.
    /// A single transaction owns every copy and rolls back on any error.
    #[allow(clippy::too_many_arguments)]
    pub fn rectangular_array(
        &mut self,
        ids: &[EntityId],
        rows: usize,
        columns: usize,
        row_spacing: f64,
        column_spacing: f64,
        angle: f64,
    ) -> CadResult<(Vec<EntityId>, ChangeSet)> {
        let (grid, ids) = self.rectangular_array_preview_plan(
            ids,
            rows,
            columns,
            row_spacing,
            column_spacing,
            angle,
        )?;
        let mut offsets = Vec::with_capacity(grid.cells() - 1);
        for offset in grid.offsets() {
            offsets.push(offset?);
        }
        self.transact("ARRAYRECT", |doc| {
            let mut copies = Vec::new();
            for offset in offsets {
                let transform = Transform2::translation(offset);
                for id in &ids {
                    let copy = doc.copy_entity(*id)?;
                    doc.transform_entity(copy, transform)?;
                    copies.push(copy);
                }
            }
            Ok(copies)
        })
    }

    /// Read-only placement and validated IDs shared by preview and commit.
    #[allow(clippy::too_many_arguments)]
    pub fn rectangular_array_preview_plan(
        &self,
        ids: &[EntityId],
        rows: usize,
        columns: usize,
        row_spacing: f64,
        column_spacing: f64,
        angle: f64,
    ) -> CadResult<(arq_geometry::RectangularGrid, Vec<EntityId>)> {
        let cells = rows
            .checked_mul(columns)
            .filter(|n| (2..=100_000).contains(n));
        if rows == 0 || columns == 0 || cells.is_none() {
            return Err(CadError::InvalidOperation(
                "a rectangular array needs 2 to 100000 cells and positive counts".into(),
            ));
        }
        if !row_spacing.is_finite()
            || !column_spacing.is_finite()
            || !angle.is_finite()
            || (rows > 1 && row_spacing == 0.0)
            || (columns > 1 && column_spacing == 0.0)
        {
            return Err(CadError::InvalidGeometry(
                "array spacing and angle must be finite; repeated axes need nonzero spacing".into(),
            ));
        }
        let grid =
            arq_geometry::RectangularGrid::new(rows, columns, row_spacing, column_spacing, angle)?;
        let ids = distinct(ids);
        for &id in &ids {
            let entity = self
                .document()
                .entity(id)
                .ok_or(CadError::EntityNotFound(id))?;
            check_unlocked(self.document(), &entity)?;
            check_transformable(&entity)?;
        }
        Ok((grid, ids))
    }

    /// Independent polar copies, ordered by item and distinct source ID order.
    /// The original is item zero. Full turns omit a duplicate final position;
    /// partial fills include the last angle. Angles are signed radians.
    /// Without rotation, base_point is required: the selection translates as a
    /// group so its base orbits center. One atomic ARRAYPOLAR undo step.
    pub fn polar_array(
        &mut self,
        ids: &[EntityId],
        center: Point2,
        count: usize,
        fill_angle: f64,
        rotate_items: bool,
        base_point: Option<Point2>,
    ) -> CadResult<(Vec<EntityId>, ChangeSet)> {
        let transforms = arq_geometry::polar_array_transforms(
            center,
            count,
            fill_angle,
            rotate_items,
            base_point,
        )?;
        let ids = distinct(ids);
        self.transact("ARRAYPOLAR", |doc| {
            let mut sources = Vec::new();
            for &id in &ids {
                let entity = doc.entity(id).ok_or(CadError::EntityNotFound(id))?;
                check_unlocked(doc, &entity)?;
                check_transformable(&entity)?;
                let bounds = entity.bounds().ok_or_else(|| {
                    CadError::InvalidGeometry("array sources need finite bounds".into())
                })?;
                sources.push((id, bounds));
            }
            let mut copies = Vec::new();
            for transform in transforms {
                for &(id, bounds) in &sources {
                    for corner in bounds.corners() {
                        transform.checked_apply_point(corner)?;
                    }
                    let copy = doc.copy_entity(id)?;
                    doc.transform_entity(copy, transform)?;
                    copies.push(copy);
                }
            }
            Ok(copies)
        })
    }

    /// N independent copies along an open planar guide, including its ends.
    /// Sources and guide stay unchanged; one atomic ARRAYPATH undo step.
    /// Alignment is relative to the first item's unchanged orientation.
    pub fn path_array(
        &mut self,
        ids: &[EntityId],
        guide: EntityId,
        base: Point2,
        count: usize,
        align: bool,
    ) -> CadResult<(Vec<EntityId>, ChangeSet)> {
        let (path, _) = self.document().join_path(guide)?;
        let transforms = arq_geometry::path_array_transforms(&path.vertices, base, count, align)?;
        let ids = distinct(ids);
        self.transact("ARRAYPATH", |doc| {
            let mut sources = Vec::new();
            for &id in &ids {
                let entity = doc.entity(id).ok_or(CadError::EntityNotFound(id))?;
                check_unlocked(doc, &entity)?;
                check_transformable(&entity)?;
                let bounds = entity.bounds().ok_or_else(|| {
                    CadError::InvalidGeometry("array sources need finite bounds".into())
                })?;
                sources.push((id, bounds));
            }
            let mut copies = Vec::new();
            for transform in transforms {
                for &(id, bounds) in &sources {
                    for corner in bounds.corners() {
                        transform.checked_apply_point(corner)?;
                    }
                    let copy = doc.copy_entity(id)?;
                    doc.transform_entity(copy, transform)?;
                    copies.push(copy);
                }
            }
            Ok(copies)
        })
    }

    /// FILLET: `a` and `b` (two lines, or a line and an arc) met by an arc of
    /// `radius` tangent to both, each kept on the side it was picked (`pa`,
    /// `pb`) and, when `trim`, trimmed or extended to it; radius 0 makes a
    /// sharp corner. The arc takes `a`'s properties. One undo step; refused
    /// for the same object twice, a locked layer, parallel lines and a
    /// radius too large. Returns the arc's id, if one was made.
    #[allow(clippy::too_many_arguments)]
    pub fn fillet_entities(
        &mut self,
        a: EntityId,
        pa: Point2,
        b: EntityId,
        pb: Point2,
        radius: f64,
        trim: bool,
    ) -> CadResult<(Option<EntityId>, ChangeSet)> {
        let tol = Tolerance::DEFAULT;
        self.corner("FILLET", a, b, |ga, gb| {
            arq_geometry::fillet(ga, pa, gb, pb, radius, trim, tol)
        })
    }

    /// CHAMFER: lines `a` and `b` cut across `first` along `a` and `second`
    /// along `b` from where they meet, each kept on the side it was picked
    /// and, when `trim`, cut back to the segment; distances 0 make a sharp
    /// corner. As [`fillet_entities`](Self::fillet_entities) otherwise.
    #[allow(clippy::too_many_arguments)]
    pub fn chamfer_entities(
        &mut self,
        a: EntityId,
        pa: Point2,
        b: EntityId,
        pb: Point2,
        first: f64,
        second: f64,
        trim: bool,
    ) -> CadResult<(Option<EntityId>, ChangeSet)> {
        let tol = Tolerance::DEFAULT;
        self.corner("CHAMFER", a, b, |ga, gb| match (ga, gb) {
            (Geometry2::Line(la), Geometry2::Line(lb)) => {
                arq_geometry::chamfer(la, pa, lb, pb, first, second, trim, tol)
            }
            _ => Err(GeometryError::OutOfRange { what: "curves" }),
        })
    }

    /// FILLET and CHAMFER's Polyline option: polyline `id`'s corners cut by
    /// `cut`, every one between straight segments, or with `at` the one the
    /// two segments picked there share. One step named FILLET or CHAMFER;
    /// refused on a locked layer. Returns the corners cut and those too
    /// short.
    pub fn cut_polyline_corners(
        &mut self,
        id: EntityId,
        cut: arq_geometry::CornerCut,
        at: Option<(Point2, Point2)>,
    ) -> CadResult<((usize, usize), ChangeSet)> {
        let name = match cut {
            arq_geometry::CornerCut::Fillet(_) => "FILLET",
            _ => "CHAMFER",
        };
        self.transact(name, |doc| {
            let e = doc.entity(id).ok_or(CadError::EntityNotFound(id))?;
            check_unlocked(doc, &e)?;
            Ok(doc.cut_polyline_corners(id, cut, at)?)
        })
    }

    /// FILLET and CHAMFER's step: `make` turns the two objects' geometry
    /// into their new geometry and the joint, written in one step.
    fn corner(
        &mut self,
        name: &str,
        a: EntityId,
        b: EntityId,
        make: impl FnOnce(Geometry2, Geometry2) -> Result<arq_geometry::Corner, GeometryError>,
    ) -> CadResult<(Option<EntityId>, ChangeSet)> {
        self.transact(name, |doc| {
            if a == b {
                return Err(DocumentError::from(GeometryError::OutOfRange {
                    what: "same object",
                })
                .into());
            }
            let mut plans = Vec::with_capacity(2);
            for id in [a, b] {
                let e = doc.entity(id).ok_or(CadError::EntityNotFound(id))?;
                check_unlocked(doc, &e)?;
                plans.push(
                    e.plan()
                        .ok_or(DocumentError::NotEditable { id, kind: e.kind() })?,
                );
            }
            let (plan_a, plan_b) = (plans[0], plans[1]);
            let corner = make(plan_a.geometry(), plan_b.geometry()).map_err(DocumentError::from)?;
            if let Some(g) = corner.first {
                doc.set_plan_geometry(a, plan_a.with_geometry(g))?;
            }
            if let Some(g) = corner.second {
                doc.set_plan_geometry(b, plan_b.with_geometry(g))?;
            }
            match corner.joint {
                Some(g) => Ok(Some(doc.add_geometry_like(a, plan_a.with_geometry(g))?)),
                None => Ok(None),
            }
        })
    }

    /// Trims line, arc, circle or lightweight polyline `id` at the edges
    /// nearest `at` (TRIM; a polyline as `Document::trim_polyline`): the
    /// part between them is taken away, a line or arc cut in the middle
    /// becomes two (the second a copy, every property kept), a circle the
    /// arc left. The edges are `edges` (Standard mode), or every other
    /// entity drawn (Quick); a text is never one. With no edge round the
    /// part, Quick (`erase_uncut`) erases the whole object, Standard
    /// refuses. Returns the entities left of it: none when erased.
    pub fn trim_entity(
        &mut self,
        id: EntityId,
        at: Point2,
        edges: Option<&[EntityId]>,
        erase_uncut: bool,
    ) -> CadResult<(Vec<EntityId>, ChangeSet)> {
        let candidates = if edges.is_none() {
            self.quick_edge_candidates(id, at, false)
        } else {
            None
        };
        let edges = edges.or(candidates.as_deref());
        let tol = Tolerance::DEFAULT;
        self.transact("TRIM", |doc| {
            let e = doc.entity(id).ok_or(CadError::EntityNotFound(id))?;
            check_unlocked(doc, &e)?;
            if e.kind() == "LWPOLYLINE" {
                let cutters = edge_curves(doc, id, edges);
                return Ok(doc.trim_polyline(id, at, &cutters, erase_uncut)?);
            }
            let plan = e
                .plan()
                .ok_or(DocumentError::NotEditable { id, kind: e.kind() })?;
            let curve = plan.geometry();
            let cutters = edge_curves(doc, id, edges);
            let cuts = cutting_points(curve, &cutters, tol);
            let pieces = match trim(curve, &cuts, at, tol).map_err(DocumentError::from)? {
                Some(pieces) => pieces,
                None if erase_uncut => Vec::new(),
                None => {
                    return Err(DocumentError::from(GeometryError::OutOfRange {
                        what: "cutting edge",
                    })
                    .into());
                }
            };
            replace_curve_pieces(doc, id, plan, &pieces)
        })
    }

    /// Broad-phase IDs only; uncertainty falls back to the exhaustive path.
    fn quick_edge_candidates(
        &self,
        id: EntityId,
        at: Point2,
        extending: bool,
    ) -> Option<Vec<EntityId>> {
        let extent = self.spatial_index.bounds()?;
        // EXTEND's current on-edge predicate accepts 1e-9 * coordinate scale.
        // Four times the global scale safely encloses that relative slack,
        // ray clipping roundoff and the geometry layer's linear tolerance.
        let scale = [
            extent.min().x(),
            extent.min().y(),
            extent.max().x(),
            extent.max().y(),
        ]
        .into_iter()
        .map(f64::abs)
        .fold(1.0, f64::max);
        let padding = 4.0 * arq_geometry::EXTEND_EDGE_RELATIVE_TOLERANCE * scale;
        let area = if extending {
            let (curve, pick) = self.document().extension_search_target(id, at).ok()?;
            extension_query_area(curve, pick, extent, padding)?
        } else {
            self.document()
                .trim_search_bounds(id)
                .ok()?
                .expanded(padding)
                .ok()?
        };
        Some(self.query_entities(area))
    }

    /// Joins lines, arcs and open lightweight polylines as AutoCAD 2021's
    /// JOIN does. With `source`: a source line takes the lines of `ids`
    /// collinear with it, a source arc the arcs on its circle, gaps filled,
    /// arcs counterclockwise from it, the circle when they go all the way
    /// round; a source polyline the lines, arcs and open polylines whose
    /// ends touch, chained on either end, turned round when needed, closed
    /// when the chain comes back to its start. Without, `ids` are joined in
    /// chains of touching ends, each from its first selected (only
    /// collinear lines make a line, only arcs of one circle an arc, any
    /// other chain a polyline), and what is left, lines and arcs, grouped
    /// as with a source. A result keeps its source's id and every property;
    /// a circle made from an arc, and a polyline made from a line or an
    /// arc, get ids of their own. The curves joined to it are erased.
    /// Curves that join nothing, of another kind, at another height or on a
    /// locked layer, are left as they are and counted as discarded. When
    /// nothing joins, nothing changes and no undo step is made. A source
    /// that cannot join, or is on a locked layer, is refused.
    pub fn join_entities(
        &mut self,
        source: Option<EntityId>,
        ids: &[EntityId],
    ) -> CadResult<(Joined, ChangeSet)> {
        let tol = Tolerance::DEFAULT;
        let doc = self.document();
        let mut seen = std::collections::HashSet::new();
        let ids: Vec<EntityId> = ids
            .iter()
            .copied()
            .filter(|&id| Some(id) != source && seen.insert(id))
            .collect();
        let usable: Vec<Piece> = ids.iter().filter_map(|&id| piece(doc, id)).collect();
        let mut joins = Vec::new();
        match source {
            Some(source) => {
                let e = doc.entity(source).ok_or(CadError::EntityNotFound(source))?;
                check_unlocked(doc, &e)?;
                let first = piece(doc, source).ok_or(DocumentError::NotEditable {
                    id: source,
                    kind: e.kind(),
                })?;
                let others: Vec<Piece> = usable
                    .into_iter()
                    .filter(|p| same_height(p, &first, tol))
                    .collect();
                let join = if first.plan.is_some() {
                    join_curves(&first, &others, tol)
                } else {
                    join_chain(&first, &others, tol)
                };
                joins.extend(join);
            }
            None => {
                for level in by_height(&usable, tol) {
                    joins.extend(join_many(&level, tol));
                }
            }
        }
        let joined: usize = joins.iter().map(|j| j.erased.len()).sum();
        // Many at once, each group's first is among `ids`, and a result.
        let firsts = if source.is_none() { joins.len() } else { 0 };
        let discarded = ids.len() - joined - firsts;
        if joined == 0 {
            let outcome = Joined {
                results: Vec::new(),
                joined,
                discarded,
            };
            return Ok((outcome, ChangeSet::default()));
        }
        let (results, changes) = self.transact("JOIN", |doc| {
            let mut results = Vec::new();
            for join in &joins {
                for &id in &join.erased {
                    doc.delete_entity(id)?;
                }
                let made = match &join.made {
                    Made::Curve(plan, Geometry2::Circle(_)) => {
                        debug_assert!(matches!(plan.geometry(), Geometry2::Arc(_)));
                        doc.arc_to_circle(join.source)?
                    }
                    Made::Curve(plan, result) => {
                        doc.set_plan_geometry(join.source, plan.with_geometry(*result))?;
                        join.source
                    }
                    Made::Polyline(chain, height) => {
                        let made = doc.write_joined_polyline(join.source, chain, *height)?;
                        if made != join.source {
                            doc.delete_entity(join.source)?;
                        }
                        made
                    }
                };
                results.push(made);
            }
            Ok(results)
        })?;
        let outcome = Joined {
            results,
            joined,
            discarded,
        };
        Ok((outcome, changes))
    }

    /// Gives the entities `targets` the properties of entity `source` that
    /// `settings` names (MATCHPROP, as AutoCAD's), in one undo step:
    /// color, layer, linetype, linetype scale, lineweight, transparency, the
    /// thickness when both have one, a polyline's uniform width and its
    /// linetype generation. Targets on a locked layer are left and counted;
    /// the source may be on one. When nothing changes, no undo step is made.
    pub fn match_properties(
        &mut self,
        source: EntityId,
        targets: &[EntityId],
        settings: MatchSettings,
    ) -> CadResult<(Matched, ChangeSet)> {
        let doc = self.document();
        doc.entity(source).ok_or(CadError::EntityNotFound(source))?;
        let mut seen = std::collections::HashSet::new();
        let mut editable = Vec::new();
        let mut locked = 0;
        for &id in targets {
            if id == source || !seen.insert(id) {
                continue;
            }
            let e = doc.entity(id).ok_or(CadError::EntityNotFound(id))?;
            if check_unlocked(doc, &e).is_err() {
                locked += 1;
            } else {
                editable.push(id);
            }
        }
        let (changed, changes) = self.transact("MATCHPROP", |doc| {
            let mut changed = 0;
            for &id in &editable {
                changed += usize::from(doc.match_properties(source, id, settings)?);
            }
            Ok(changed)
        })?;
        Ok((Matched { changed, locked }, changes))
    }

    /// Turns arc `id` into its full circle (JOIN's Close), every property
    /// kept; returns the circle's id. An elliptical arc becomes its whole
    /// ellipse, its id kept (ADR 0041 D4). Refused for anything else, a
    /// whole ellipse included, for what is not drawn in plan, and on a
    /// locked layer.
    pub fn close_arc(&mut self, id: EntityId) -> CadResult<(EntityId, ChangeSet)> {
        let e = self
            .document()
            .entity(id)
            .ok_or(CadError::EntityNotFound(id))?;
        check_unlocked(self.document(), &e)?;
        let refused = || DocumentError::NotEditable { id, kind: e.kind() };
        match (e.kind(), e.plan()) {
            ("ARC", Some(_)) => self.transact("JOIN", |doc| Ok(doc.arc_to_circle(id)?)),
            ("ELLIPSE", Some(plan)) => {
                let Geometry2::Ellipse(arc) = plan.geometry() else {
                    return Err(refused().into());
                };
                if arc.is_full() {
                    return Err(refused().into());
                }
                self.transact("JOIN", |doc| {
                    doc.set_plan_geometry(id, plan.with_geometry(arc.whole().into()))?;
                    Ok(id)
                })
            }
            _ => Err(refused().into()),
        }
    }

    /// Removes a projected gap on a line, arc, circle or lightweight polyline.
    /// Each piece retains the source properties. One atomic BREAK undo step.
    pub fn break_entity(
        &mut self,
        id: EntityId,
        first: Point2,
        second: Point2,
    ) -> CadResult<(Vec<EntityId>, ChangeSet)> {
        self.transact("BREAK", |doc| {
            let entity = doc.entity(id).ok_or(CadError::EntityNotFound(id))?;
            check_unlocked(doc, &entity)?;
            if entity.kind() == "LWPOLYLINE" {
                return Ok(doc.break_polyline(id, first, second)?);
            }
            let plan = entity.plan().ok_or(DocumentError::NotEditable {
                id,
                kind: entity.kind(),
            })?;
            let pieces = break_curve(plan.geometry(), first, second, Tolerance::DEFAULT)
                .map_err(DocumentError::from)?;
            replace_curve_pieces(doc, id, plan, &pieces)
        })
    }

    /// Extends line, arc or open lightweight polyline `id` at the end nearer
    /// `at` to the nearest of `edges` (Standard mode), or of every other
    /// entity drawn (Quick), its line or circle (a polyline's end segment's)
    /// reaches beyond that end (EXTEND). Refused when none does, and for
    /// anything else (a circle, a closed polyline).
    pub fn extend_entity(
        &mut self,
        id: EntityId,
        at: Point2,
        edges: Option<&[EntityId]>,
    ) -> CadResult<ChangeSet> {
        let candidates = if edges.is_none() {
            self.quick_edge_candidates(id, at, true)
        } else {
            None
        };
        let edges = edges.or(candidates.as_deref());
        let tol = Tolerance::DEFAULT;
        let (_, changes) = self.transact("EXTEND", |doc| {
            let e = doc.entity(id).ok_or(CadError::EntityNotFound(id))?;
            check_unlocked(doc, &e)?;
            if e.kind() == "LWPOLYLINE" {
                let boundaries = edge_curves(doc, id, edges);
                return Ok(doc.extend_polyline(id, at, &boundaries)?);
            }
            let plan = e
                .plan()
                .ok_or(DocumentError::NotEditable { id, kind: e.kind() })?;
            let boundaries = edge_curves(doc, id, edges);
            let grown = extend(plan.geometry(), &boundaries, at, tol)
                .map_err(DocumentError::from)?
                .ok_or(DocumentError::from(GeometryError::OutOfRange {
                    what: "boundary edge",
                }))?;
            doc.set_plan_geometry(id, plan.with_geometry(grown))?;
            Ok(())
        })?;
        Ok(changes)
    }

    /// Adds a copy of polyline `id` offset `distance` toward `toward`, or
    /// through `toward` when `distance` is None (OFFSET), every property
    /// kept; refused on a locked layer and where the path cannot be offset.
    pub fn offset_polyline(
        &mut self,
        id: EntityId,
        distance: Option<f64>,
        toward: Point2,
    ) -> CadResult<(EntityId, ChangeSet)> {
        self.transact("OFFSET", |doc| {
            let e = doc.entity(id).ok_or(CadError::EntityNotFound(id))?;
            check_unlocked(doc, &e)?;
            Ok(doc.offset_polyline(id, distance, toward)?)
        })
    }

    /// Applies `t` to entities that transform: lines, circles and arcs
    /// through their geometry, blocks and polylines as a whole (ADR 0016,
    /// D1). Any other entity fails the whole operation.
    fn transform_entities(
        &mut self,
        name: &str,
        ids: &[EntityId],
        t: Transform2,
    ) -> CadResult<ChangeSet> {
        self.edit_entities(name, ids, |doc, e| {
            check_transformable(e)?;
            Ok(doc.transform_entity(e.id(), t)?)
        })
    }

    /// Gives a line, circle or arc new geometry of the same kind, keeping
    /// its id, layer, color and elevation (a grip stretch, a property
    /// edit). Any other entity, or geometry of another kind, fails.
    pub fn set_entity_geometry(
        &mut self,
        id: EntityId,
        geometry: Geometry2,
    ) -> CadResult<ChangeSet> {
        self.edit_entities("EDIT", &[id], |doc, e| {
            let plan = e
                .plan()
                .ok_or(DocumentError::NotEditable { id, kind: e.kind() })?;
            Ok(doc.set_plan_geometry(id, plan.with_geometry(geometry))?)
        })
    }

    /// Writes a single-line text on the current layer (TEXT, ADR 0022).
    pub fn add_text(
        &mut self,
        content: &str,
        anchor: Point2,
        height: f64,
        rotation: f64,
    ) -> CadResult<(EntityId, ChangeSet)> {
        self.transact("TEXT", |doc| {
            Ok(doc.add_text(content, anchor, height, rotation)?)
        })
    }

    /// Draws a polyline on the current layer (PLINE): vertices with the
    /// bulge of the segment leaving each, open or `closed`.
    pub fn add_polyline(
        &mut self,
        vertices: &[(Point2, f64)],
        closed: bool,
    ) -> CadResult<(EntityId, ChangeSet)> {
        self.add_polyline_with(vertices, closed, PolylineStyle::default())
    }

    /// [`add_polyline`](Self::add_polyline) at an elevation, with a
    /// thickness and a constant width (RECTANG's options); nothing changes
    /// when one is not finite or the width is negative.
    pub fn add_polyline_with(
        &mut self,
        vertices: &[(Point2, f64)],
        closed: bool,
        style: PolylineStyle,
    ) -> CadResult<(EntityId, ChangeSet)> {
        self.transact("PLINE", |doc| {
            Ok(doc.add_polyline_with(vertices, closed, style)?)
        })
    }

    /// Makes block `name` from `ids`, base point `base`, and puts a
    /// reference to it in their place (BLOCK, ADR 0023 D2).
    pub fn create_block(
        &mut self,
        name: &str,
        base: Point2,
        ids: &[EntityId],
    ) -> CadResult<(EntityId, ChangeSet)> {
        let ids = distinct(ids);
        self.transact("BLOCK", |doc| {
            for &id in &ids {
                let e = doc.entity(id).ok_or(CadError::EntityNotFound(id))?;
                check_unlocked(doc, &e)?;
            }
            Ok(doc.create_block(name, base, &ids)?)
        })
    }

    /// Makes or redefines block `name` from `ids` as the Block Definition
    /// dialog says (BLOCK, ADR 0023 D2a): the objects retained, converted
    /// or deleted, the record's description, units and flags written. One
    /// undo step; returns the reference placed by Convert.
    pub fn define_block(
        &mut self,
        name: &str,
        base: Point2,
        ids: &[EntityId],
        objects: BlockObjects,
        definition: &BlockDefinition,
        redefine: bool,
    ) -> CadResult<(Option<EntityId>, ChangeSet)> {
        let ids = distinct(ids);
        self.transact("BLOCK", |doc| {
            for &id in &ids {
                let e = doc.entity(id).ok_or(CadError::EntityNotFound(id))?;
                check_unlocked(doc, &e)?;
            }
            Ok(doc.define_block(name, base, &ids, objects, definition, redefine)?)
        })
    }

    /// Renames block `name` to `new_name`; every reference follows (RENAME,
    /// ADR 0025 step 8c). One undo step.
    pub fn rename_block(&mut self, name: &str, new_name: &str) -> CadResult<ChangeSet> {
        Ok(self
            .transact("RENAME", |doc| Ok(doc.rename_block(name, new_name)?))?
            .1)
    }

    /// Purges unused blocks `names`, and with `nested` the blocks they held
    /// that nothing else uses (PURGE, ADR 0025 step 8e). One undo step;
    /// returns the blocks purged.
    pub fn purge_blocks(
        &mut self,
        names: &[String],
        nested: bool,
    ) -> CadResult<(Vec<String>, ChangeSet)> {
        self.transact("PURGE", |doc| Ok(doc.purge_blocks(names, nested)?))
    }

    /// Sets block `name`'s data for budgets; empty data removes it
    /// (BLOCKDATA, ADR 0027). One undo step; returns the block's name as
    /// the drawing spells it.
    pub fn set_block_data(
        &mut self,
        name: &str,
        data: &BlockData,
    ) -> CadResult<(String, ChangeSet)> {
        self.transact("BLOCKDATA", |doc| Ok(doc.set_block_data(name, data)?))
    }

    /// Replaces the drawing's currency register (CURRENCIES, ADR 0027 D3).
    /// One undo step.
    pub fn set_currencies(&mut self, currencies: &Currencies) -> CadResult<ChangeSet> {
        Ok(self
            .transact("CURRENCIES", |doc| Ok(doc.set_currencies(currencies)?))?
            .1)
    }

    /// A new drawing holding block `name`'s contents alone, for the block
    /// editor (BEDIT, ADR 0023 D3); this drawing is unchanged.
    pub fn block_editor(&self, name: &str) -> CadResult<CadDocument> {
        Ok(CadDocument::from_document(
            self.document().block_editor(name)?,
        ))
    }

    /// Makes `contents`' model space the contents of block `name`, as the
    /// block editor saves it: one undo step (BEDIT, ADR 0023 D3).
    pub fn replace_block_contents(
        &mut self,
        name: &str,
        contents: &Document,
    ) -> CadResult<ChangeSet> {
        Ok(self
            .transact("BEDIT", |doc| {
                Ok(doc.replace_block_contents(name, contents)?)
            })?
            .1)
    }

    /// Places block `name` (INSERT, ADR 0023 D2).
    pub fn insert_block(
        &mut self,
        name: &str,
        point: Point2,
        scale: f64,
        rotation: f64,
    ) -> CadResult<(EntityId, ChangeSet)> {
        self.transact("INSERT", |doc| {
            Ok(doc.insert_block(name, point, scale, rotation)?)
        })
    }

    /// Copies block `name` of `source` here, with what it needs, choosing
    /// by `choices` for each block whose name this drawing uses for other
    /// contents (ADR 0025 D6); returns its name here. One undo step, or
    /// part of INSERT's when grouped with it.
    pub fn copy_block(
        &mut self,
        source: &Document,
        name: &str,
        choices: &BTreeMap<String, ClashChoice>,
    ) -> CadResult<(String, ChangeSet)> {
        self.transact("INSERT", |doc| Ok(doc.copy_block(source, name, choices)?))
    }

    /// Writes a multiline text on the current layer (MTEXT, ADR 0022 D6).
    #[allow(clippy::too_many_arguments)]
    pub fn add_mtext(
        &mut self,
        content: &str,
        anchor: Point2,
        height: f64,
        rotation: f64,
        width: f64,
        justification: &str,
    ) -> CadResult<(EntityId, ChangeSet)> {
        self.transact("MTEXT", |doc| {
            Ok(doc.add_mtext(content, anchor, height, rotation, width, justification)?)
        })
    }

    /// Sets what is given of texts' content, height and rotation (radians),
    /// all or none (ADR 0022).
    pub fn set_text(
        &mut self,
        ids: &[EntityId],
        content: Option<&str>,
        height: Option<f64>,
        rotation: Option<f64>,
    ) -> CadResult<ChangeSet> {
        self.edit_entities("PROPERTIES", ids, |doc, e| {
            Ok(doc.set_text(e.id(), content, height, rotation)?)
        })
    }

    /// STRETCH: each selected entity's crossing windows, or a whole move
    /// when it has none. Every edit commits as one step or rolls back.
    pub fn stretch_entities(
        &mut self,
        ids: &[EntityId],
        offset: Vector2,
        crossing: &[(EntityId, arq_geometry::Bounds2)],
    ) -> CadResult<ChangeSet> {
        self.stretch_entities_with_polygons(ids, offset, crossing, &[])
    }

    /// STRETCH with rectangle and polygon provenance combined per entity.
    /// Validation precedes the atomic edit; points in overlapping areas move once.
    pub fn stretch_entities_with_polygons(
        &mut self,
        ids: &[EntityId],
        offset: Vector2,
        crossing: &[(EntityId, arq_geometry::Bounds2)],
        polygons: &[(EntityId, SelectionPolygon)],
    ) -> CadResult<ChangeSet> {
        let regions = stretch_regions(crossing, polygons);
        self.edit_entities("STRETCH", ids, |doc, e| {
            let areas = regions.get(&e.id()).map_or(&[][..], Vec::as_slice);
            Ok(doc.stretch_entity_in_areas(e.id(), areas, offset)?)
        })
    }

    /// Pure STRETCH preview, with the same source checks and plans as commit.
    pub fn stretch_preview(
        &self,
        ids: &[EntityId],
        offset: Vector2,
        crossing: &[(EntityId, arq_geometry::Bounds2)],
        polygons: &[(EntityId, SelectionPolygon)],
    ) -> CadResult<Vec<arq_document::Shape>> {
        let regions = stretch_regions(crossing, polygons);
        let doc = self.document();
        let mut shapes = Vec::new();
        for id in distinct(ids) {
            let entity = doc.entity(id).ok_or(CadError::EntityNotFound(id))?;
            check_unlocked(doc, &entity)?;
            shapes.extend(doc.stretch_shapes(
                id,
                regions.get(&id).map_or(&[][..], Vec::as_slice),
                offset,
            )?);
        }
        Ok(shapes)
    }

    /// Moves one lightweight-polyline vertex to a plan point (grip stretch).
    pub fn set_polyline_vertex(
        &mut self,
        id: EntityId,
        vertex_index: usize,
        point: Point2,
    ) -> CadResult<ChangeSet> {
        self.edit_entities("GRIP", &[id], |doc, _| {
            Ok(doc.set_polyline_vertex(id, vertex_index, point)?)
        })
    }

    /// Explodes selected block references into their transformed contents.
    pub fn explode_entities(&mut self, ids: &[EntityId]) -> CadResult<(Vec<EntityId>, ChangeSet)> {
        let ids = distinct(ids);
        self.transact("EXPLODE", |doc| {
            let mut created = Vec::new();
            for id in ids {
                let entity = doc.entity(id).ok_or(CadError::EntityNotFound(id))?;
                check_unlocked(doc, &entity)?;
                created.extend(doc.explode_insert(id)?);
            }
            Ok(created)
        })
    }

    /// Moves entities to another layer (CHPROP).
    pub fn set_entity_layer(&mut self, ids: &[EntityId], layer: LayerId) -> CadResult<ChangeSet> {
        self.edit_entities("CHPROP", ids, |doc, e| {
            Ok(doc.set_entity_layer(e.id(), layer)?)
        })
    }

    /// Sets the color of entities (CHPROP).
    pub fn set_entity_color(&mut self, ids: &[EntityId], color: Color) -> CadResult<ChangeSet> {
        self.edit_entities("CHPROP", ids, |doc, e| {
            Ok(doc.set_entity_color(e.id(), color)?)
        })
    }

    /// Runs `edit` on each distinct entity of `ids` in one transaction,
    /// after checking it exists and is not on a locked layer.
    fn edit_entities(
        &mut self,
        name: &str,
        ids: &[EntityId],
        mut edit: impl FnMut(&mut Document, &Entity) -> CadResult<()>,
    ) -> CadResult<ChangeSet> {
        let ids = distinct(ids);
        let ((), summary) = self.transact(name, |doc| {
            for id in ids {
                let e = doc.entity(id).ok_or(CadError::EntityNotFound(id))?;
                check_unlocked(doc, &e)?;
                edit(doc, &e)?;
            }
            Ok(())
        })?;
        Ok(summary)
    }

    /// Creates a layer with AutoCAD's defaults.
    pub fn create_layer(&mut self, name: &str) -> CadResult<(LayerId, ChangeSet)> {
        self.transact("LAYER", |doc| Ok(doc.create_layer(name)?))
    }

    /// Renames a layer; its entities follow.
    pub fn rename_layer(&mut self, id: LayerId, new_name: &str) -> CadResult<ChangeSet> {
        self.layer_op(|doc| doc.rename_layer(id, new_name))
    }

    /// Sets a layer's color (an ACI index or RGB).
    pub fn set_layer_color(&mut self, id: LayerId, color: Color) -> CadResult<ChangeSet> {
        self.layer_op(|doc| doc.set_layer_color(id, color))
    }

    /// Turns a layer on or off.
    pub fn set_layer_visible(&mut self, id: LayerId, visible: bool) -> CadResult<ChangeSet> {
        self.layer_op(|doc| doc.set_layer_visible(id, visible))
    }

    /// Freezes or thaws a layer.
    pub fn set_layer_frozen(&mut self, id: LayerId, frozen: bool) -> CadResult<ChangeSet> {
        self.layer_op(|doc| doc.set_layer_frozen(id, frozen))
    }

    /// Locks or unlocks a layer.
    pub fn set_layer_locked(&mut self, id: LayerId, locked: bool) -> CadResult<ChangeSet> {
        self.layer_op(|doc| doc.set_layer_locked(id, locked))
    }

    /// Whether a layer is plotted.
    pub fn set_layer_plottable(&mut self, id: LayerId, plottable: bool) -> CadResult<ChangeSet> {
        self.layer_op(|doc| doc.set_layer_plottable(id, plottable))
    }

    /// Deletes an empty, unprotected layer that is not current.
    pub fn delete_layer(&mut self, id: LayerId) -> CadResult<ChangeSet> {
        self.layer_op(|doc| doc.delete_layer(id))
    }

    /// Makes a layer current (CLAYER).
    pub fn set_current_layer(&mut self, id: LayerId) -> CadResult<ChangeSet> {
        self.layer_op(|doc| doc.set_current_layer(id))
    }

    fn layer_op(
        &mut self,
        f: impl FnOnce(&mut Document) -> arq_document::DocumentResult<()>,
    ) -> CadResult<ChangeSet> {
        let ((), summary) = self.transact("LAYER", |doc| Ok(f(doc)?))?;
        Ok(summary)
    }
}

/// Crossing provenance shared by the pure preview and atomic edit.
fn stretch_regions(
    crossing: &[(EntityId, arq_geometry::Bounds2)],
    polygons: &[(EntityId, SelectionPolygon)],
) -> BTreeMap<EntityId, Vec<CrossingArea>> {
    let mut regions = BTreeMap::<EntityId, Vec<CrossingArea>>::new();
    for &(id, area) in crossing {
        regions
            .entry(id)
            .or_default()
            .push(CrossingArea::Rectangle(area));
    }
    for (id, polygon) in polygons {
        regions
            .entry(*id)
            .or_default()
            .push(CrossingArea::Polygon(polygon.clone()));
    }
    regions
}

/// `ids` without repeats, in first-seen order.
fn distinct(ids: &[EntityId]) -> Vec<EntityId> {
    let mut seen = std::collections::HashSet::new();
    ids.iter().copied().filter(|id| seen.insert(*id)).collect()
}

fn check_transformable(e: &Entity) -> CadResult<()> {
    if e.is_transformable() {
        return Ok(());
    }
    Err(CadError::InvalidOperation(format!(
        "entity {} ({}) cannot be transformed in this version",
        e.id(),
        e.kind()
    )))
}

/// What MATCHPROP did ([`CadDocument::match_properties`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Matched {
    /// How many targets changed.
    pub changed: usize,
    /// How many were on a locked layer, and left.
    pub locked: usize,
}

/// What JOIN did ([`CadDocument::join_entities`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Joined {
    /// The curves joined into, in order: each source, or the circle an arc
    /// became.
    pub results: Vec<EntityId>,
    /// How many curves were joined to them, and erased.
    pub joined: usize,
    /// How many could not join and were left as they were.
    pub discarded: usize,
}

/// One source and what JOIN makes of it, worked out before any change.
struct Join {
    source: EntityId,
    made: Made,
    erased: Vec<EntityId>,
}

/// What a source becomes: its line or arc, or the circle an arc closes
/// into, in its plan; or a polyline at a height.
enum Made {
    Curve(PlanGeometry, Geometry2),
    Polyline(Chain, f64),
}

/// A curve that can join: a line, an arc, an elliptical arc or an open
/// lightweight polyline drawn in plan, not on a locked layer; its plan
/// geometry when a line or an arc (elliptical or not), the path it chains
/// by into a polyline (an elliptical arc has none: it joins only arcs of
/// its ellipse, ADR 0041 D4), and its height.
#[derive(Clone)]
struct Piece {
    id: EntityId,
    plan: Option<PlanGeometry>,
    path: Option<JoinPath>,
    height: f64,
}

fn piece(doc: &Document, id: EntityId) -> Option<Piece> {
    let e = doc.entity(id)?;
    if !matches!(e.kind(), "LINE" | "ARC" | "ELLIPSE" | "LWPOLYLINE")
        || check_unlocked(doc, &e).is_err()
    {
        return None;
    }
    if e.kind() == "ELLIPSE" {
        let plan = e.plan()?;
        return Some(Piece {
            id,
            plan: Some(plan),
            path: None,
            height: plan.elevation(),
        });
    }
    let (path, height) = doc.join_path(id).ok()?;
    Some(Piece {
        id,
        plan: e.plan(),
        path: Some(path),
        height,
    })
}

/// A source line or arc joined with the lines, or the arcs, of `others` it
/// takes; None when it takes none.
fn join_curves(source: &Piece, others: &[Piece], tol: Tolerance) -> Option<Join> {
    let plan = source.plan?;
    let curves: Vec<(EntityId, Geometry2)> = others
        .iter()
        .filter_map(|p| p.plan.map(|plan| (p.id, plan.geometry())))
        .collect();
    let (result, taken) = match plan.geometry() {
        Geometry2::Line(line) => {
            let (ids, lines): (Vec<EntityId>, Vec<Line>) = curves
                .iter()
                .filter_map(|&(id, g)| match g {
                    Geometry2::Line(l) => Some((id, l)),
                    _ => None,
                })
                .unzip();
            let (joined, taken) = join_lines(line, &lines, tol);
            let taken: Vec<EntityId> = taken.into_iter().map(|i| ids[i]).collect();
            (Geometry2::Line(joined), taken)
        }
        Geometry2::Arc(arc) => {
            let (ids, arcs): (Vec<EntityId>, Vec<Arc>) = curves
                .iter()
                .filter_map(|&(id, g)| match g {
                    Geometry2::Arc(a) => Some((id, a)),
                    _ => None,
                })
                .unzip();
            let (joined, taken) = join_arcs(arc, &arcs, tol);
            (joined, taken.into_iter().map(|i| ids[i]).collect())
        }
        Geometry2::Ellipse(ellipse) => {
            let (ids, arcs): (Vec<EntityId>, Vec<arq_geometry::Ellipse>) = curves
                .iter()
                .filter_map(|&(id, g)| match g {
                    Geometry2::Ellipse(e) => Some((id, e)),
                    _ => None,
                })
                .unzip();
            let (joined, taken) = join_elliptical_arcs(ellipse, &arcs, tol);
            (joined, taken.into_iter().map(|i| ids[i]).collect())
        }
        Geometry2::Circle(_) => return None,
    };
    (!taken.is_empty()).then_some(Join {
        source: source.id,
        made: Made::Curve(plan, result),
        erased: taken,
    })
}

/// `first` with the curves of `others` whose ends touch chained on, as a
/// polyline; None when none touches. When `first` has one width all along,
/// the lines and arcs chained on take it, as ProgeCAD gives them (case J3,
/// maintainer, 2026-11-20); polylines keep their own.
fn join_chain(first: &Piece, others: &[Piece], tol: Tolerance) -> Option<Join> {
    let first_path = first.path.as_ref()?;
    let width = uniform_width(first, tol);
    // Only what has a path chains: an elliptical arc never does.
    let chainable: Vec<usize> = (0..others.len())
        .filter(|&i| others[i].path.is_some())
        .collect();
    let paths: Vec<JoinPath> = chainable
        .iter()
        .map(|&i| {
            let p = &others[i];
            let path = p.path.clone().expect("chainable");
            match (width, p.plan) {
                (Some(w), Some(_)) => JoinPath {
                    widths: vec![(w, w); path.widths.len()],
                    ..path
                },
                _ => path,
            }
        })
        .collect();
    let mut chain = chain_paths(first_path, &paths, tol);
    if chain.taken.is_empty() {
        return None;
    }
    // Taken as indices into `others`.
    chain.taken = chain.taken.iter().map(|&k| chainable[k]).collect();
    let erased = chain.taken.iter().map(|&i| others[i].id).collect();
    Some(Join {
        source: first.id,
        made: Made::Polyline(chain, first.height),
        erased,
    })
}

/// The width a piece has all along (none for a line or an arc, whose width
/// is nothing); None for a varying width.
fn uniform_width(piece: &Piece, tol: Tolerance) -> Option<f64> {
    let path = piece.path.as_ref()?;
    let (w, _) = *path.widths.first()?;
    let same = |v: f64| (v - w).abs() <= tol.linear();
    path.widths
        .iter()
        .all(|&(a, b)| same(a) && same(b))
        .then_some(w)
}

/// JOIN many at once on one level: chains of touching ends, in selection
/// order, each from its first (collinear lines alone stay a line, arcs of
/// one circle alone an arc); then the lines and arcs left grouped as with
/// a source, gaps filled.
fn join_many(level: &[Piece], tol: Tolerance) -> Vec<Join> {
    let mut free = vec![true; level.len()];
    let mut joins = Vec::new();
    for seed in 0..level.len() {
        if !free[seed] {
            continue;
        }
        let rest: Vec<usize> = (0..level.len()).filter(|&i| i != seed && free[i]).collect();
        let others: Vec<Piece> = rest.iter().map(|&i| level[i].clone()).collect();
        let Some(chained) = join_chain(&level[seed], &others, tol) else {
            continue;
        };
        let Made::Polyline(chain, _) = &chained.made else {
            unreachable!("a chain")
        };
        let members: Vec<Piece> = chain.taken.iter().map(|&k| others[k].clone()).collect();
        for &k in &chain.taken {
            free[rest[k]] = false;
        }
        free[seed] = false;
        // All collinear lines stay a line; all arcs of one circle an arc, or
        // the circle when they close it.
        let alike =
            join_curves(&level[seed], &members, tol).filter(|j| j.erased.len() == members.len());
        joins.push(alike.unwrap_or(chained));
    }
    let left: Vec<Piece> = (0..level.len())
        .filter(|&i| free[i] && level[i].plan.is_some())
        .map(|i| level[i].clone())
        .collect();
    let curves: Vec<Geometry2> = left
        .iter()
        .filter_map(|p| p.plan)
        .map(|p| p.geometry())
        .collect();
    for group in join_groups(&curves, tol) {
        let (first, rest) = group.split_first().expect("a group has two");
        let others: Vec<Piece> = rest.iter().map(|&i| left[i].clone()).collect();
        joins.extend(join_curves(&left[*first], &others, tol));
    }
    joins
}

/// Whether two curves lie at one height: only those join.
fn same_height(a: &Piece, b: &Piece, tol: Tolerance) -> bool {
    (a.height - b.height).abs() <= tol.linear()
}

/// `pieces` split by height, each level in selection order.
fn by_height(pieces: &[Piece], tol: Tolerance) -> Vec<Vec<Piece>> {
    let mut levels: Vec<Vec<Piece>> = Vec::new();
    for piece in pieces {
        match levels
            .iter_mut()
            .find(|level| same_height(&level[0], piece, tol))
        {
            Some(level) => level.push(piece.clone()),
            None => levels.push(vec![piece.clone()]),
        }
    }
    levels
}

fn check_unlocked(doc: &Document, e: &Entity) -> CadResult<()> {
    if doc.layer_by_name(e.layer()).is_some_and(|l| l.is_locked()) {
        return Err(CadError::InvalidOperation(format!(
            "entity {} is on locked layer {}",
            e.id(),
            e.layer()
        )));
    }
    Ok(())
}

/// Finite ray to the index envelope, or an arc's full supporting circle
/// (an elliptical arc's whole ellipse).
/// No bounds()/drawing_all() scan is needed to bound a ray.
fn extension_query_area(
    curve: Geometry2,
    pick: Point2,
    extent: arq_geometry::Bounds2,
    padding: f64,
) -> Option<arq_geometry::Bounds2> {
    match curve {
        Geometry2::Circle(_) => None,
        Geometry2::Ellipse(e) if e.is_full() => None,
        Geometry2::Arc(arc) => {
            // A partial arc may have finite bounds even if its supporting
            // circle overflows. Refuse the broad phase, never panic here.
            let (center, radius) = (arc.center(), arc.radius());
            round_query_area(
                arq_geometry::Bounds2::new(
                    Point2::new(center.x() - radius, center.y() - radius).ok()?,
                    Point2::new(center.x() + radius, center.y() + radius).ok()?,
                ),
                padding,
            )
        }
        // An elliptical arc grows round its whole ellipse.
        Geometry2::Ellipse(e) => round_query_area(e.whole().bounds(), padding),
        Geometry2::Line(line) => {
            let at_end = arq_geometry::extends_at_end(curve, pick);
            let tip = if at_end { line.end() } else { line.start() };
            let direction = line.vector().normalized().ok()? * if at_end { 1.0 } else { -1.0 };
            let extent = extent.expanded(padding).ok()?;
            let exit = |p: f64, d: f64, lo: f64, hi: f64| {
                if d > 0.0 {
                    (hi - p) / d
                } else if d < 0.0 {
                    (lo - p) / d
                } else {
                    f64::INFINITY
                }
            };
            let t = exit(tip.x(), direction.x(), extent.min().x(), extent.max().x()).min(exit(
                tip.y(),
                direction.y(),
                extent.min().y(),
                extent.max().y(),
            ));
            if !t.is_finite() || t < 0.0 {
                return None;
            }
            let end = Point2::new(tip.x() + direction.x() * t, tip.y() + direction.y() * t).ok()?;
            arq_geometry::Bounds2::new(tip, end).expanded(padding).ok()
        }
    }
}

/// A round curve's whole extent, padded as EXTEND's on-edge test needs.
fn round_query_area(bounds: arq_geometry::Bounds2, padding: f64) -> Option<arq_geometry::Bounds2> {
    let scale = [
        bounds.min().x(),
        bounds.min().y(),
        bounds.max().x(),
        bounds.max().y(),
    ]
    .into_iter()
    .map(f64::abs)
    .fold(1.0, f64::max);
    bounds
        .expanded(padding.max(4.0 * arq_geometry::EXTEND_EDGE_RELATIVE_TOLERANCE * scale))
        .ok()
}

/// The curves TRIM cuts at and EXTEND reaches: the lines, arcs, circles
/// and ellipses `edges` draw (every entity drawn but `target` when None),
/// those inside blocks and polylines included; texts never.
fn edge_curves(doc: &Document, target: EntityId, edges: Option<&[EntityId]>) -> Vec<Geometry2> {
    let ids: Vec<EntityId> = match edges {
        Some(edges) => edges.to_vec(),
        None => doc
            .entities()
            .into_iter()
            .map(|entity| entity.id())
            .collect(),
    };
    ids.into_iter()
        .filter(|&id| id != target)
        .filter(|&id| {
            doc.entity(id)
                .is_some_and(|e| !matches!(e.kind(), "TEXT" | "MTEXT" | "ATTRIB" | "ATTDEF"))
        })
        .flat_map(|id| doc.drawing(id).unwrap_or_default())
        .filter_map(|item| match item.shape {
            arq_document::Shape::Line(l) => Some(Geometry2::Line(l)),
            arq_document::Shape::Arc(a) => Some(Geometry2::Arc(a)),
            arq_document::Shape::Circle(c) => Some(Geometry2::Circle(c)),
            arq_document::Shape::Ellipse(e) => Some(Geometry2::Ellipse(e)),
            _ => None,
        })
        .collect()
}

/// Applies already-computed pieces using the existing document writers.
fn replace_curve_pieces(
    doc: &mut Document,
    id: EntityId,
    plan: PlanGeometry,
    pieces: &[Geometry2],
) -> CadResult<Vec<EntityId>> {
    let mut left = Vec::new();
    match (plan.geometry(), pieces) {
        (_, []) => doc.delete_entity(id)?,
        (Geometry2::Circle(_), [Geometry2::Arc(arc)]) => {
            left.push(doc.circle_to_arc(id, *arc)?);
        }
        (_, [first, rest @ ..]) => {
            doc.set_plan_geometry(id, plan.with_geometry(*first))?;
            left.push(id);
            for piece in rest {
                let copy = doc.copy_entity(id)?;
                doc.set_plan_geometry(copy, plan.with_geometry(*piece))?;
                left.push(copy);
            }
        }
    }
    Ok(left)
}

#[cfg(test)]
#[path = "operations_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "quick_edges_tests.rs"]
mod quick_edges_tests;
