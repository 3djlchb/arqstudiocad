//! Entity access on [`Document`].

use acadrust::entities::LwPolyline;
use acadrust::types::{Transform, Vector2, Vector3};
use acadrust::{EntityType, Handle};
use arq_geometry::{
    Bounds2, Chain, CornerCut, CrossingArea, Geometry2, GeometryError, JoinPath, Point2, Tolerance,
    Transform2, break_closed_polyline, break_polyline, cut_polyline_corners, extend_polyline,
    offset_polyline, offset_polyline_through, shared_vertex, trim_polyline,
};

use crate::document::Document;
use crate::drawing::Shape;
use crate::entity::Color;
use crate::entity::{Entity, EntityId, PlanGeometry, PolylineStyle};
use crate::error::{DocumentError, DocumentResult};
use crate::layers::LayerId;
use crate::ocs::{PlanNormal, plan_normal};
use crate::translate::{
    attachment_named, attribute_as_text, bounds_of, color_to_codec, edit_text, handle,
    is_whole_transformable, new_entity, new_mtext, new_text, read_entity, transform_whole,
    write_plan,
};

/// A lightweight polyline seen in the world's plan: its vertices there,
/// their bulges turned the world's way, and what writes them back.
struct WorldPolyline {
    vertices: Vec<(Point2, f64)>,
    closed: bool,
    normal: PlanNormal,
    elevation: f64,
}

/// Shared, read-only STRETCH decision, used by commit and preview.
enum StretchPlan {
    Unchanged,
    Move,
    Curve(PlanGeometry),
    /// A spline through fit points, some moved, fitted again at its height.
    Fit(arq_geometry::SplineFit, f64),
    Polyline {
        path: WorldPolyline,
        targets: Vec<(usize, Point2)>,
    },
}

impl Document {
    /// STRETCH in plan: moves whole selections, or just line endpoints and
    /// polyline vertices inside crossing windows. Partial standalone arcs
    /// retain their sagitta. Solids remain unchanged.
    pub fn stretch_entity(
        &mut self,
        id: EntityId,
        areas: &[Bounds2],
        offset: arq_geometry::Vector2,
    ) -> DocumentResult<()> {
        let regions: Vec<_> = areas.iter().copied().map(CrossingArea::Rectangle).collect();
        self.stretch_entity_in_areas(id, &regions, offset)
    }

    /// The same STRETCH writer for rectangles and validated crossing polygons.
    pub fn stretch_entity_in_areas(
        &mut self,
        id: EntityId,
        areas: &[CrossingArea],
        offset: arq_geometry::Vector2,
    ) -> DocumentResult<()> {
        match self.stretch_plan(id, areas, offset)? {
            StretchPlan::Unchanged => Ok(()),
            StretchPlan::Move => self.transform_entity(id, Transform2::translation(offset)),
            StretchPlan::Curve(plan) => self.set_plan_geometry(id, plan),
            StretchPlan::Fit(fit, z) => self.set_fit_spline(id, &fit, z),
            StretchPlan::Polyline { targets, .. } => {
                for (index, point) in targets {
                    self.set_polyline_vertex(id, index, point)?;
                }
                Ok(())
            }
        }
    }

    /// Read-only centerline shapes of the exact STRETCH result. No document
    /// clone, history edit or codec mutation; only selected entities are read.
    pub fn stretch_shapes(
        &self,
        id: EntityId,
        areas: &[CrossingArea],
        offset: arq_geometry::Vector2,
    ) -> DocumentResult<Vec<Shape>> {
        let curve = |g| match g {
            Geometry2::Line(line) => Shape::Line(line),
            Geometry2::Circle(circle) => Shape::Circle(circle),
            Geometry2::Arc(arc) => Shape::Arc(arc),
            Geometry2::Ellipse(ellipse) => Shape::Ellipse(ellipse),
            Geometry2::Spline(spline) => Shape::Spline(spline),
        };
        match self.stretch_plan(id, areas, offset)? {
            StretchPlan::Unchanged => Ok(Vec::new()),
            StretchPlan::Curve(plan) => Ok(vec![curve(plan.geometry())]),
            StretchPlan::Fit(fit, _) => Ok(vec![Shape::Spline(std::sync::Arc::new(fit.spline()?))]),
            StretchPlan::Polyline { path, .. } => {
                let count = if path.closed {
                    path.vertices.len()
                } else {
                    path.vertices.len() - 1
                };
                (0..count)
                    .map(|i| {
                        let (a, bulge) = path.vertices[i];
                        let b = path.vertices[(i + 1) % path.vertices.len()].0;
                        Ok(match arq_geometry::Arc::from_bulge(a, b, bulge) {
                            Ok((arc, _)) => Shape::Arc(arc),
                            Err(_) => Shape::Line(arq_geometry::Line::new(a, b)?),
                        })
                    })
                    .collect()
            }
            StretchPlan::Move => {
                let t = Transform2::translation(offset);
                self.drawing(id)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|item| {
                        Ok(match item.shape {
                            Shape::Line(l) => Shape::Line(t.apply_line(l)?),
                            Shape::Circle(c) => Shape::Circle(t.apply_circle(c)?),
                            Shape::Arc(a) => Shape::Arc(t.apply_arc(a)?),
                            Shape::Ellipse(e) => Shape::Ellipse(e.transformed(t)?),
                            Shape::Spline(s) => {
                                Shape::Spline(std::sync::Arc::new(s.transformed(t)?))
                            }
                            Shape::Point(p) => Shape::Point(t.apply_point(p)),
                            Shape::Text(b) => Shape::Text(Bounds2::new(
                                t.apply_point(b.min()),
                                t.apply_point(b.max()),
                            )),
                            Shape::Placeholder(b) => Shape::Placeholder(Bounds2::new(
                                t.apply_point(b.min()),
                                t.apply_point(b.max()),
                            )),
                            Shape::Triangle(points) => {
                                Shape::Triangle(points.map(|p| t.apply_point(p)))
                            }
                        })
                    })
                    .collect()
            }
        }
    }

    /// A spline stretched: through fit points, those inside moved and the
    /// spline fitted again through them, its knots method and tangents
    /// kept; by its vertices, those inside moved, the copies a closed one
    /// repeats following their own. Left when none is inside, moved whole
    /// when all are.
    fn stretch_spline(
        &self,
        id: EntityId,
        spline: &arq_geometry::Spline,
        z: f64,
        inside: &dyn Fn(Point2) -> bool,
        offset: arq_geometry::Vector2,
    ) -> DocumentResult<StretchPlan> {
        let shift = |q: Point2| Point2::new(q.x() + offset.x(), q.y() + offset.y());
        let decide = |hits: &[bool]| {
            if !hits.contains(&true) {
                Some(StretchPlan::Unchanged)
            } else if !hits.contains(&false) {
                Some(StretchPlan::Move)
            } else {
                None
            }
        };
        let fit = match self.cad().get_entity(handle(id)) {
            Some(EntityType::Spline(sp)) => crate::drawing::spline_fit(sp),
            _ => None,
        };
        if let Some(fit) = fit {
            let hits: Vec<bool> = fit.points().iter().map(|&q| inside(q)).collect();
            if let Some(plan) = decide(&hits) {
                return Ok(plan);
            }
            let points = fit
                .points()
                .iter()
                .zip(&hits)
                .map(|(&q, &hit)| if hit { shift(q) } else { Ok(q) })
                .collect::<Result<Vec<_>, _>>()?;
            let moved = arq_geometry::SplineFit::new(
                points,
                fit.knots(),
                fit.start_tangent(),
                fit.end_tangent(),
                fit.is_closed(),
            )?;
            return Ok(StretchPlan::Fit(moved, z));
        }
        // How many points a closed one repeats at its end: its degree when
        // periodic, its first when clamped round to it.
        let (p, points) = (spline.degree(), spline.points());
        let n = points.len();
        let repeated = if !spline.is_closed() {
            0
        } else if n > p && points[n - p..] == points[..p] {
            p
        } else if points[n - 1] == points[0] {
            1
        } else {
            0
        };
        let own = n - repeated;
        let hits: Vec<bool> = points[..own].iter().map(|&q| inside(q)).collect();
        if let Some(plan) = decide(&hits) {
            return Ok(plan);
        }
        let moved = (0..n)
            .map(|i| {
                let q = points[i];
                if hits[i % own] { shift(q) } else { Ok(q) }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let stretched = arq_geometry::Spline::new(
            p,
            moved,
            spline.weights().to_vec(),
            spline.knots().to_vec(),
            spline.is_closed(),
        )?;
        Ok(StretchPlan::Curve(PlanGeometry::at_elevation(
            stretched.into(),
            z,
        )?))
    }

    /// Writes `fit`'s spline into SPLINE `id` at height `z`: its control
    /// points and knots, and its fit points, knots method and tangents.
    fn set_fit_spline(
        &mut self,
        id: EntityId,
        fit: &arq_geometry::SplineFit,
        z: f64,
    ) -> DocumentResult<()> {
        let spline = fit.spline()?;
        let e = self
            .cad_mut()
            .get_entity_mut(handle(id))
            .ok_or(DocumentError::EntityNotFound(id))?;
        let EntityType::Spline(sp) = e else {
            let kind = e.as_entity().entity_type();
            return Err(DocumentError::KindMismatch { id, kind });
        };
        crate::drawing::write_plan_spline(sp, &spline, z);
        write_fit(sp, fit, z);
        self.sync_entity(id);
        Ok(())
    }

    fn stretch_plan(
        &self,
        id: EntityId,
        areas: &[CrossingArea],
        offset: arq_geometry::Vector2,
    ) -> DocumentResult<StretchPlan> {
        let e = self.entity(id).ok_or(DocumentError::EntityNotFound(id))?;
        if e.kind() == "3DSOLID" {
            return Ok(StretchPlan::Unchanged);
        }
        if let Some(bounds) = e.bounds() {
            for point in [bounds.min(), bounds.max()] {
                Point2::new(point.x() + offset.x(), point.y() + offset.y())?;
            }
        }
        let tol = Tolerance::default();
        let inside = |point| areas.iter().any(|area| area.contains(point, tol));
        if areas.is_empty() {
            return Ok(StretchPlan::Move);
        }
        if let Some(plan) = e.plan() {
            match plan.geometry() {
                Geometry2::Line(line) => {
                    let a = inside(line.start());
                    let b = inside(line.end());
                    if a || b {
                        let start = if a {
                            Point2::new(
                                line.start().x() + offset.x(),
                                line.start().y() + offset.y(),
                            )?
                        } else {
                            line.start()
                        };
                        let end = if b {
                            Point2::new(line.end().x() + offset.x(), line.end().y() + offset.y())?
                        } else {
                            line.end()
                        };
                        let line = arq_geometry::Line::new(start, end)?;
                        return Ok(StretchPlan::Curve(plan.with_geometry(line.into())));
                    }
                    return Ok(StretchPlan::Unchanged);
                }
                Geometry2::Circle(circle) => {
                    return if inside(circle.center()) {
                        Ok(StretchPlan::Move)
                    } else {
                        Ok(StretchPlan::Unchanged)
                    };
                }
                Geometry2::Arc(arc) => {
                    let a = inside(arc.start_point());
                    let b = inside(arc.end_point());
                    if a && b {
                        return Ok(StretchPlan::Move);
                    }
                    if !a && !b {
                        return Ok(StretchPlan::Unchanged);
                    }
                    let moved =
                        |point: Point2| Point2::new(point.x() + offset.x(), point.y() + offset.y());
                    let start = if a {
                        moved(arc.start_point())?
                    } else {
                        arc.start_point()
                    };
                    let end = if b {
                        moved(arc.end_point())?
                    } else {
                        arc.end_point()
                    };
                    let stretched = arc.with_endpoints_preserving_sagitta(start, end, tol)?;
                    return Ok(StretchPlan::Curve(plan.with_geometry(stretched.into())));
                }
                // As a circle when whole, as an arc otherwise: moved, or
                // left, whole (ADR 0041 D4).
                Geometry2::Ellipse(ellipse) => {
                    let moved = match ellipse.ends() {
                        None => inside(ellipse.center()),
                        Some((a, b)) => match (inside(a), inside(b)) {
                            (true, true) => true,
                            (false, false) => false,
                            _ => {
                                return Err(DocumentError::NotEditable {
                                    id,
                                    kind: "ELLIPSE",
                                });
                            }
                        },
                    };
                    return Ok(if moved {
                        StretchPlan::Move
                    } else {
                        StretchPlan::Unchanged
                    });
                }
                // Its points inside moved (ADR 0043 D4).
                Geometry2::Spline(spline) => {
                    return self.stretch_spline(id, &spline, plan.elevation(), &inside, offset);
                }
            }
        }
        if e.kind() == "LWPOLYLINE" {
            let grips = self.grips(&[id]);
            if grips.is_empty() {
                return Err(DocumentError::NotEditable {
                    id,
                    kind: "LWPOLYLINE",
                });
            }
            // Compute every target before writing any vertex. The core's
            // transaction restores all entities if a later write fails.
            let targets = grips
                .iter()
                .filter(|g| inside(g.point))
                .map(|g| {
                    Ok((
                        g.vertex_index.expect("polyline vertex"),
                        Point2::new(g.point.x() + offset.x(), g.point.y() + offset.y())?,
                    ))
                })
                .collect::<Result<Vec<_>, GeometryError>>()?;
            if targets.is_empty() {
                return Ok(StretchPlan::Unchanged);
            }
            let path = self.polyline_in_world(id)?;
            let mut vertices = path.vertices;
            for &(index, point) in &targets {
                vertices[index].0 = point;
            }
            for pair in vertices.windows(2) {
                arq_geometry::Line::new(pair[0].0, pair[1].0)?;
            }
            if path.closed && vertices.len() > 1 {
                arq_geometry::Line::new(vertices[vertices.len() - 1].0, vertices[0].0)?;
            }
            return Ok(StretchPlan::Polyline {
                path: WorldPolyline { vertices, ..path },
                targets,
            });
        }
        if !matches!(e.kind(), "INSERT" | "TEXT" | "MTEXT") {
            return Err(DocumentError::NotEditable { id, kind: e.kind() });
        }
        Ok(if self.grips(&[id]).iter().any(|g| inside(g.point)) {
            StretchPlan::Move
        } else {
            StretchPlan::Unchanged
        })
    }

    /// Replaces one block reference with its transformed contents. Its
    /// visible attributes, and the definition's constant ones, become
    /// single-line texts showing their values, as AutoCAD Express's BURST
    /// (ADR 0023 D1); the other attribute definitions are not copied.
    pub fn explode_insert(&mut self, id: EntityId) -> DocumentResult<Vec<EntityId>> {
        let insert = match self.cad().get_entity(handle(id)) {
            Some(EntityType::Insert(insert)) => insert.clone(),
            Some(entity) => {
                return Err(DocumentError::NotEditable {
                    id,
                    kind: entity.as_entity().entity_type(),
                });
            }
            None => return Err(DocumentError::EntityNotFound(id)),
        };
        let record = self
            .cad()
            .block_records
            .get(&insert.block_name)
            .ok_or_else(|| {
                DocumentError::UnsupportedOperation("the block definition is missing".to_owned())
            })?;
        if !record.explodable {
            // Allow exploding is off in the block's definition (ADR 0023 D2a).
            return Err(DocumentError::UnsupportedOperation(format!(
                "{} cannot be exploded",
                insert.block_name
            )));
        }
        // INSERT transforms contents around the block's base point. The codec's
        // explode helper omits this adjustment, so normalize the cloned source
        // entities first to match the document's drawing transform.
        let base = record.base_point;
        let base_offset = Transform::from_translation(Vector3::new(-base.x, -base.y, -base.z));
        let mut contents: Vec<_> = self
            .cad()
            .entities_in_block(&insert.block_name)
            .filter_map(|entity| match entity {
                EntityType::AttributeDefinition(_) => attribute_as_text(entity),
                other => Some(other.clone()),
            })
            .collect();
        for entity in &mut contents {
            entity.apply_transform(&base_offset);
        }
        let mut parts = insert.explode(&contents);
        // The reference's own attributes already sit where they are drawn.
        parts.extend(
            insert
                .attributes
                .iter()
                .filter_map(|a| attribute_as_text(&EntityType::AttributeEntity(a.clone()))),
        );
        if parts.is_empty() {
            return Err(DocumentError::UnsupportedOperation(
                "the block has no contents that can be exploded".to_owned(),
            ));
        }
        let mut ids = Vec::with_capacity(parts.len());
        for mut part in parts.drain(..) {
            let common = part.common_mut();
            common.handle = Handle::NULL;
            common.owner_handle = Handle::NULL;
            common.raw_record = None;
            common.reactors.clear();
            common.xdictionary_handle = None;
            common.graphic_data = None;
            let handle = self.cad_mut().add_entity(part)?;
            ids.push(handle.value());
            self.sync_entity(handle.value());
        }
        self.delete_entity(id)?;
        Ok(ids)
    }

    /// Moves one vertex of a lightweight polyline in plan, preserving its
    /// bulge, widths, elevation and entity properties.
    pub fn set_polyline_vertex(
        &mut self,
        id: EntityId,
        vertex_index: usize,
        point: arq_geometry::Point2,
    ) -> DocumentResult<()> {
        let raw = self
            .cad_mut()
            .get_entity_mut(handle(id))
            .ok_or(DocumentError::EntityNotFound(id))?;
        let EntityType::LwPolyline(polyline) = raw else {
            return Err(DocumentError::NotEditable {
                id,
                kind: raw.as_entity().entity_type(),
            });
        };
        let normal = plan_normal(polyline.normal.x, polyline.normal.y, polyline.normal.z).ok_or(
            DocumentError::NotEditable {
                id,
                kind: "LWPOLYLINE",
            },
        )?;
        let vertex =
            polyline
                .vertices
                .get_mut(vertex_index)
                .ok_or(DocumentError::InvalidVertexIndex {
                    id,
                    index: vertex_index,
                })?;
        let (x, y, _) = normal.map_point(point.x(), point.y(), polyline.elevation);
        vertex.location = acadrust::types::Vector2::new(x, y);
        Ok(())
    }

    /// Lightweight polyline `id`'s vertices in world plan coordinates, its
    /// bulges turned the world's way, whether it is closed, and its plane;
    /// refused for anything else and on a tilted plane.
    fn polyline_in_world(&self, id: EntityId) -> DocumentResult<WorldPolyline> {
        let raw = self
            .cad()
            .get_entity(handle(id))
            .ok_or(DocumentError::EntityNotFound(id))?;
        let EntityType::LwPolyline(polyline) = raw else {
            return Err(DocumentError::NotEditable {
                id,
                kind: raw.as_entity().entity_type(),
            });
        };
        let normal = plan_normal(polyline.normal.x, polyline.normal.y, polyline.normal.z).ok_or(
            DocumentError::NotEditable {
                id,
                kind: "LWPOLYLINE",
            },
        )?;
        let turn = if normal == PlanNormal::Down {
            -1.0
        } else {
            1.0
        };
        let vertices = polyline
            .vertices
            .iter()
            .map(|v| {
                let (x, y, _) = normal.map_point(v.location.x, v.location.y, polyline.elevation);
                Ok((Point2::new(x, y)?, v.bulge * turn))
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        Ok(WorldPolyline {
            vertices,
            closed: polyline.is_closed,
            normal,
            elevation: polyline.elevation,
        })
    }

    /// Gives lightweight polyline `id` these world vertices, open, back in
    /// its own plane; its per-vertex widths go, its constant width stays.
    fn set_polyline_path(
        &mut self,
        id: EntityId,
        vertices: &[(Point2, f64)],
        closed: bool,
        normal: PlanNormal,
        elevation: f64,
    ) {
        let turn = if normal == PlanNormal::Down {
            -1.0
        } else {
            1.0
        };
        if let Some(EntityType::LwPolyline(polyline)) = self.cad_mut().get_entity_mut(handle(id)) {
            polyline.vertices.clear();
            for &(p, bulge) in vertices {
                let (x, y, _) = normal.map_point(p.x(), p.y(), elevation);
                polyline.add_point_with_bulge(Vector2::new(x, y), bulge * turn);
            }
            polyline.is_closed = closed;
        }
        self.sync_entity(id);
    }

    /// Trims lightweight polyline `id` at its crossings with `edges` around
    /// `at` (TRIM): the first piece left keeps its id, a second is a copy
    /// with every property, a closed polyline opens. With no crossing
    /// round that part, `erase_uncut` erases it (Quick), else it is refused.
    /// Returns the entities left: none when erased.
    pub fn trim_polyline(
        &mut self,
        id: EntityId,
        at: Point2,
        edges: &[Geometry2],
        erase_uncut: bool,
    ) -> DocumentResult<Vec<EntityId>> {
        let WorldPolyline {
            vertices,
            closed,
            normal,
            elevation,
        } = self.polyline_in_world(id)?;
        let pieces = match trim_polyline(&vertices, closed, edges, at, Tolerance::DEFAULT)? {
            Some(pieces) => pieces,
            None if erase_uncut => Vec::new(),
            None => {
                return Err(DocumentError::Geometry(GeometryError::OutOfRange {
                    what: "cutting edge",
                }));
            }
        };
        let Some((first, rest)) = pieces.split_first() else {
            self.delete_entity(id)?;
            return Ok(Vec::new());
        };
        let mut left = vec![id];
        for piece in rest {
            let copy = self.copy_entity(id)?;
            self.set_polyline_path(copy, piece, false, normal, elevation);
            left.push(copy);
        }
        self.set_polyline_path(id, first, false, normal, elevation);
        Ok(left)
    }

    /// BREAK a lightweight polyline at projected points, preserving its
    /// properties and interpolating each original segment's widths. Closed
    /// paths open with the approved vertex-parameter traversal semantics.
    pub fn break_polyline(
        &mut self,
        id: EntityId,
        first: Point2,
        second: Point2,
    ) -> DocumentResult<Vec<EntityId>> {
        let WorldPolyline {
            vertices,
            closed,
            normal,
            elevation,
        } = self.polyline_in_world(id)?;
        let pieces = if closed {
            vec![break_closed_polyline(
                &vertices,
                first,
                second,
                Tolerance::DEFAULT,
            )?]
        } else {
            break_polyline(&vertices, first, second, Tolerance::DEFAULT)?
        };
        let EntityType::LwPolyline(source) = self.cad().get_entity(handle(id)).unwrap() else {
            unreachable!()
        };
        let widths: Vec<_> = source
            .vertices
            .iter()
            .map(|v| {
                if closed && source.constant_width > 0.0 {
                    (source.constant_width, source.constant_width)
                } else {
                    (v.start_width, v.end_width)
                }
            })
            .collect();
        if pieces.is_empty() {
            self.delete_entity(id)?;
            return Ok(Vec::new());
        }
        let mut ids = vec![id];
        // Copy the complete source before rewriting any of its pieces.
        for _ in 1..pieces.len() {
            ids.push(self.copy_entity(id)?);
        }
        for (&target, piece) in ids.iter().zip(pieces) {
            self.set_polyline_path(target, &piece.vertices, false, normal, elevation);
            let EntityType::LwPolyline(poly) =
                self.cad_mut().get_entity_mut(handle(target)).unwrap()
            else {
                unreachable!()
            };
            if closed {
                // The opened path represents effective widths per segment;
                // the terminal vertex has no outgoing width.
                poly.constant_width = 0.0;
            }
            for (vertex, span) in poly.vertices.iter_mut().zip(piece.spans) {
                let (a, b) = widths[span.segment];
                vertex.start_width = a + (b - a) * span.from;
                vertex.end_width = a + (b - a) * span.to;
            }
            self.sync_entity(target);
        }
        Ok(ids)
    }

    /// Cuts lightweight polyline `id`'s corners by `cut` (FILLET and
    /// CHAMFER's Polyline option): every corner between two straight
    /// segments, or, with `at`, only the one the two segments picked there
    /// share. Returns how many corners were cut and how many were too short;
    /// nothing changes when none was cut. Refused when the two segments
    /// picked are not next to each other.
    pub fn cut_polyline_corners(
        &mut self,
        id: EntityId,
        cut: CornerCut,
        at: Option<(Point2, Point2)>,
    ) -> DocumentResult<(usize, usize)> {
        let WorldPolyline {
            vertices,
            closed,
            normal,
            elevation,
        } = self.polyline_in_world(id)?;
        let tol = Tolerance::DEFAULT;
        let segments = if closed {
            vertices.len()
        } else {
            vertices.len().saturating_sub(1)
        };
        let only = match at {
            None => None,
            Some((a, b)) => {
                let nearest = |p: Point2| -> DocumentResult<usize> {
                    let mut best: Option<(f64, usize)> = None;
                    for s in 0..segments {
                        let (from, bulge) = vertices[s];
                        let to = vertices[(s + 1) % vertices.len()].0;
                        let curve = if bulge == 0.0 {
                            Geometry2::Line(arq_geometry::Line::new(from, to)?)
                        } else {
                            Geometry2::Arc(arq_geometry::Arc::from_bulge(from, to, bulge)?.0)
                        };
                        let d = curve.distance_to(p);
                        if best.is_none_or(|(bd, _)| d < bd) {
                            best = Some((d, s));
                        }
                    }
                    best.map(|(_, s)| s)
                        .ok_or(DocumentError::Geometry(GeometryError::ShortContour))
                };
                let vertex = shared_vertex(nearest(a)?, nearest(b)?, segments, closed).ok_or(
                    DocumentError::Geometry(GeometryError::OutOfRange { what: "segments" }),
                )?;
                Some(vertex)
            }
        };
        let done = cut_polyline_corners(&vertices, closed, cut, only, tol)?;
        if done.done > 0 {
            self.set_polyline_path(id, &done.vertices, closed, normal, elevation);
        }
        Ok((done.done, done.too_short))
    }

    /// Operative curve bounds for Quick TRIM, independent of tessellation/width.
    pub fn trim_search_bounds(&self, id: EntityId) -> DocumentResult<Bounds2> {
        let entity = self.entity(id).ok_or(DocumentError::EntityNotFound(id))?;
        let tol = Tolerance::DEFAULT;
        if let Some(plan) = entity.plan() {
            let curve = plan.geometry();
            let margin = match curve {
                Geometry2::Arc(a) => a.radius() * tol.angular(),
                _ => 0.0,
            } + tol.linear();
            return Ok(curve.bounds().expanded(margin)?);
        }
        let path = self.polyline_in_world(id)?;
        Ok(arq_geometry::polyline_trim_bounds(
            &path.vertices,
            path.closed,
            tol,
        )?)
    }

    /// Same world curve and end choice used by the existing EXTEND operation.
    pub fn extension_search_target(
        &self,
        id: EntityId,
        at: Point2,
    ) -> DocumentResult<(Geometry2, Point2)> {
        let entity = self.entity(id).ok_or(DocumentError::EntityNotFound(id))?;
        if let Some(plan) = entity.plan() {
            return Ok((plan.geometry(), at));
        }
        let path = self.polyline_in_world(id)?;
        Ok(arq_geometry::polyline_extension_target(
            &path.vertices,
            path.closed,
            at,
        )?)
    }

    /// Extends open lightweight polyline `id` at the end nearer `at` to the
    /// nearest of `edges` its end segment reaches (EXTEND); refused when
    /// none does, and for a closed polyline.
    pub fn extend_polyline(
        &mut self,
        id: EntityId,
        at: Point2,
        edges: &[Geometry2],
    ) -> DocumentResult<()> {
        let WorldPolyline {
            vertices,
            closed,
            normal,
            elevation,
        } = self.polyline_in_world(id)?;
        let grown = extend_polyline(&vertices, closed, edges, at, Tolerance::DEFAULT)?.ok_or(
            DocumentError::Geometry(GeometryError::OutOfRange {
                what: "boundary edge",
            }),
        )?;
        self.set_polyline_path(id, &grown, false, normal, elevation);
        Ok(())
    }

    /// Adds an entity drawing `plan` with every common property of `like`
    /// (layer, color, linetype, lineweight, XDATA…): FILLET's arc and
    /// CHAMFER's segment take the first object's. Returns its id.
    pub fn add_geometry_like(
        &mut self,
        like: EntityId,
        plan: PlanGeometry,
    ) -> DocumentResult<EntityId> {
        let common = self
            .cad()
            .get_entity(handle(like))
            .ok_or(DocumentError::EntityNotFound(like))?
            .common()
            .clone();
        let mut made = new_entity(plan, &common.layer);
        *made.common_mut() = common;
        self.add_copy(made, Handle::NULL)
    }

    /// Replaces circle `id` with `arc`, which lies on its circle (world plan
    /// angles), every common property kept (layer, color, linetype,
    /// lineweight, XDATA…) and its thickness and plane (TRIM, which leaves an
    /// arc of a circle cut twice). Returns the arc's id.
    pub fn circle_to_arc(
        &mut self,
        id: EntityId,
        arc: arq_geometry::Arc,
    ) -> DocumentResult<EntityId> {
        let Some(EntityType::Circle(circle)) = self.cad().get_entity(handle(id)) else {
            let kind = self
                .cad()
                .get_entity(handle(id))
                .map(|e| e.as_entity().entity_type())
                .ok_or(DocumentError::EntityNotFound(id))?;
            return Err(DocumentError::NotEditable { id, kind });
        };
        let normal = plan_normal(circle.normal.x, circle.normal.y, circle.normal.z)
            .ok_or(DocumentError::NotEditable { id, kind: "CIRCLE" })?;
        // Its angles in the circle's own plane, still counterclockwise.
        let (start, end) = normal.map_arc(arc.start_angle(), arc.end_angle());
        let mut made = acadrust::Arc::new();
        made.common = circle.common.clone();
        made.center = circle.center;
        made.radius = circle.radius;
        made.thickness = circle.thickness;
        made.normal = circle.normal;
        made.start_angle = start;
        made.end_angle = end;
        let new = self.add_copy(EntityType::Arc(made), Handle::NULL)?;
        self.delete_entity(id)?;
        Ok(new)
    }

    /// Replaces arc `id` with its full circle, every common property kept
    /// (layer, color, linetype, lineweight, XDATA…) and its thickness and
    /// plane (JOIN, when arcs go all the way round or Close is chosen).
    /// Returns the circle's id.
    pub fn arc_to_circle(&mut self, id: EntityId) -> DocumentResult<EntityId> {
        let Some(EntityType::Arc(arc)) = self.cad().get_entity(handle(id)) else {
            let kind = self
                .cad()
                .get_entity(handle(id))
                .map(|e| e.as_entity().entity_type())
                .ok_or(DocumentError::EntityNotFound(id))?;
            return Err(DocumentError::NotEditable { id, kind });
        };
        let mut made = acadrust::Circle::new();
        made.common = arc.common.clone();
        made.center = arc.center;
        made.radius = arc.radius;
        made.thickness = arc.thickness;
        made.normal = arc.normal;
        let new = self.add_copy(EntityType::Circle(made), Handle::NULL)?;
        self.delete_entity(id)?;
        Ok(new)
    }

    /// A line, an arc or an open lightweight polyline drawn in plan, as
    /// JOIN chains it: its path in world plan coordinates, each segment's
    /// widths (a constant width on every segment), and its height in the
    /// world. Refused for anything else, a closed polyline included.
    pub fn join_path(&self, id: EntityId) -> DocumentResult<(JoinPath, f64)> {
        let e = self.entity(id).ok_or(DocumentError::EntityNotFound(id))?;
        match (e.kind(), e.plan()) {
            ("LINE", Some(plan)) | ("ARC", Some(plan)) => {
                let path = match plan.geometry() {
                    Geometry2::Line(line) => JoinPath::line(line),
                    Geometry2::Arc(arc) => JoinPath::arc(arc),
                    Geometry2::Circle(_) | Geometry2::Ellipse(_) | Geometry2::Spline(_) => {
                        unreachable!("a line or an arc")
                    }
                };
                return Ok((path, plan.elevation()));
            }
            ("LWPOLYLINE", _) => {}
            (kind, _) => return Err(DocumentError::NotEditable { id, kind }),
        }
        let WorldPolyline {
            vertices,
            closed,
            normal,
            elevation,
        } = self.polyline_in_world(id)?;
        let Some(EntityType::LwPolyline(polyline)) = self.cad().get_entity(handle(id)) else {
            unreachable!("polyline_in_world read a polyline")
        };
        if closed || vertices.len() < 2 {
            return Err(DocumentError::NotEditable {
                id,
                kind: "closed LWPOLYLINE",
            });
        }
        let constant = polyline.constant_width;
        let widths = polyline.vertices[..vertices.len() - 1]
            .iter()
            .map(|v| {
                if constant > 0.0 {
                    (constant, constant)
                } else {
                    (v.start_width, v.end_width)
                }
            })
            .collect();
        let mut vertices = vertices;
        let last = vertices.len() - 1;
        vertices[last].1 = 0.0;
        let height = normal.map_point(0.0, 0.0, elevation).2;
        Ok((JoinPath { vertices, widths }, height))
    }

    /// Writes `chain`, at world height `height`, as JOIN's polyline: into
    /// lightweight polyline `like` itself, in its own plane, every property
    /// kept; or, when `like` is a line or an arc, into a new polyline with
    /// every common property and the thickness of `like`. Each segment
    /// keeps its widths; a constant width stays only when every segment
    /// still has it. Returns the polyline's id.
    pub fn write_joined_polyline(
        &mut self,
        like: EntityId,
        chain: &Chain,
        height: f64,
    ) -> DocumentResult<EntityId> {
        let raw = self
            .cad()
            .get_entity(handle(like))
            .ok_or(DocumentError::EntityNotFound(like))?;
        let (id, normal) = match raw {
            EntityType::LwPolyline(polyline) => {
                let normal = plan_normal(polyline.normal.x, polyline.normal.y, polyline.normal.z)
                    .ok_or(DocumentError::NotEditable {
                    id: like,
                    kind: "LWPOLYLINE",
                })?;
                (like, normal)
            }
            EntityType::Line(_) | EntityType::Arc(_) => {
                let (common, thickness) = match raw {
                    EntityType::Line(line) => (line.common.clone(), line.thickness),
                    EntityType::Arc(arc) => (arc.common.clone(), arc.thickness),
                    _ => unreachable!(),
                };
                let mut made = LwPolyline::new();
                made.common = common;
                made.thickness = thickness;
                made.elevation = height;
                made.normal = Vector3::new(0.0, 0.0, 1.0);
                (
                    self.add_copy(EntityType::LwPolyline(made), Handle::NULL)?,
                    PlanNormal::Up,
                )
            }
            other => {
                return Err(DocumentError::NotEditable {
                    id: like,
                    kind: other.as_entity().entity_type(),
                });
            }
        };
        let turn = if normal == PlanNormal::Down {
            -1.0
        } else {
            1.0
        };
        let elevation = normal.map_point(0.0, 0.0, height).2;
        let widths = &chain.path.widths;
        let Some(EntityType::LwPolyline(polyline)) = self.cad_mut().get_entity_mut(handle(id))
        else {
            unreachable!("a polyline")
        };
        let constant = polyline.constant_width;
        let keep_constant = constant > 0.0 && widths.iter().all(|&w| w == (constant, constant));
        polyline.constant_width = if keep_constant { constant } else { 0.0 };
        polyline.elevation = elevation;
        polyline.vertices.clear();
        for (k, &(p, bulge)) in chain.path.vertices.iter().enumerate() {
            let (x, y, _) = normal.map_point(p.x(), p.y(), elevation);
            polyline.add_point_with_bulge(Vector2::new(x, y), bulge * turn);
            let (start, end) = widths.get(k).copied().unwrap_or((0.0, 0.0));
            if !keep_constant {
                let vertex = polyline.vertices.last_mut().expect("just added");
                vertex.start_width = start;
                vertex.end_width = end;
            }
        }
        polyline.is_closed = chain.closed;
        self.sync_entity(id);
        Ok(id)
    }

    /// Adds a copy of lightweight polyline `id` offset `distance` toward
    /// `toward`, or through `toward` when `distance` is None (OFFSET):
    /// every property kept, its vertices those of the offset path in its
    /// own plane, their widths kept when no segment was dropped. Refused on
    /// a tilted plane and when the path cannot be offset there.
    pub fn offset_polyline(
        &mut self,
        id: EntityId,
        distance: Option<f64>,
        toward: Point2,
    ) -> DocumentResult<EntityId> {
        let raw = self
            .cad()
            .get_entity(handle(id))
            .ok_or(DocumentError::EntityNotFound(id))?;
        let EntityType::LwPolyline(polyline) = raw else {
            return Err(DocumentError::NotEditable {
                id,
                kind: raw.as_entity().entity_type(),
            });
        };
        let normal = plan_normal(polyline.normal.x, polyline.normal.y, polyline.normal.z).ok_or(
            DocumentError::NotEditable {
                id,
                kind: "LWPOLYLINE",
            },
        )?;
        // Its vertices are in its own coordinates (OCS) already; the point
        // comes in the world's, mapped by its plane (its own inverse).
        let path = polyline
            .vertices
            .iter()
            .map(|v| Ok((Point2::new(v.location.x, v.location.y)?, v.bulge)))
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let (x, y, _) = normal.map_point(toward.x(), toward.y(), polyline.elevation);
        let toward = Point2::new(x, y)?;
        let tol = Tolerance::DEFAULT;
        let offset = match distance {
            Some(d) => offset_polyline(&path, polyline.is_closed, d, toward, tol)?,
            None => offset_polyline_through(&path, polyline.is_closed, toward, tol)?,
        };
        let widths: Vec<(f64, f64)> = polyline
            .vertices
            .iter()
            .map(|v| (v.start_width, v.end_width))
            .collect();
        let copy = self.copy_entity(id)?;
        if let Some(EntityType::LwPolyline(made)) = self.cad_mut().get_entity_mut(handle(copy)) {
            made.vertices.clear();
            for (i, (p, bulge)) in offset.iter().enumerate() {
                made.add_point_with_bulge(Vector2::new(p.x(), p.y()), *bulge);
                if widths.len() == offset.len()
                    && let (Some(v), Some(&(start, end))) =
                        (made.vertices.last_mut(), widths.get(i))
                {
                    v.start_width = start;
                    v.end_width = end;
                }
            }
        }
        self.sync_entity(copy);
        Ok(copy)
    }

    /// A snapshot of the entity with this id, in any space: from the own
    /// model when it owns the entity, else from the engine.
    pub fn entity(&self, id: EntityId) -> Option<Entity> {
        self.owned_view(id).or_else(|| {
            self.cad()
                .get_entity(handle(id))
                .map(|e| self.read_with_extent(e))
        })
    }

    /// Snapshots of every entity in model space, in drawing order.
    pub fn entities(&self) -> Vec<Entity> {
        self.cad()
            .model_space_entities()
            .map(|e| {
                let id = e.as_entity().handle().value();
                self.owned_view(id)
                    .unwrap_or_else(|| self.read_with_extent(e))
            })
            .collect()
    }

    /// An engine entity as the core sees it; a block reference or a
    /// dimension boxed by what it draws, which the engine does not know.
    fn read_with_extent(&self, e: &EntityType) -> Entity {
        let mut entity = read_entity(e);
        if let Some(extent) = self.block_extent(e) {
            entity.bounds = Some(extent);
        }
        entity
    }

    /// The drawn box of a block reference or a dimension, and of a
    /// polyline, whose arcs the engine's box (its vertices) leaves out;
    /// `None` for other kinds, whose own box is right.
    fn block_extent(&self, e: &EntityType) -> Option<Bounds2> {
        match e {
            EntityType::Insert(_)
            | EntityType::Dimension(_)
            | EntityType::LwPolyline(_)
            | EntityType::Polyline2D(_)
            | EntityType::Solid3D(_)
            | EntityType::Region(_)
            | EntityType::Body(_)
            | EntityType::Surface(_) => self.drawn_extent(e),
            _ => None,
        }
    }

    /// The plan bounds of every model-space entity that has them, in drawing
    /// order: a cheap pass for spatial queries (no layer names copied).
    pub fn entity_bounds(&self) -> impl Iterator<Item = (EntityId, Bounds2)> + '_ {
        self.cad().model_space_entities().filter_map(|e| {
            let bounds = self.block_extent(e).or_else(|| bounds_of(e))?;
            Some((e.as_entity().handle().value(), bounds))
        })
    }

    /// Adds plan geometry to model space on `layer`; returns its id.
    pub fn add_entity(&mut self, plan: PlanGeometry, layer: LayerId) -> DocumentResult<EntityId> {
        let name = self
            .layer(layer)
            .ok_or(DocumentError::LayerIdNotFound(layer))?
            .name()
            .to_owned();
        let h = self.cad_mut().add_entity(new_entity(plan, &name))?;
        self.sync_entity(h.value());
        Ok(h.value())
    }

    /// Replaces the plan geometry of an editable entity of the same kind,
    /// keeping its id, layer, color, thickness, normal direction and XDATA.
    pub fn set_plan_geometry(&mut self, id: EntityId, plan: PlanGeometry) -> DocumentResult<()> {
        let editable = self
            .entity(id)
            .ok_or(DocumentError::EntityNotFound(id))?
            .is_editable();
        let e = self
            .cad_mut()
            .get_entity_mut(handle(id))
            .ok_or(DocumentError::EntityNotFound(id))?;
        if !editable {
            let kind = e.as_entity().entity_type();
            return Err(DocumentError::NotEditable { id, kind });
        }
        write_plan(e, plan)?;
        self.sync_entity(id);
        Ok(())
    }

    /// Moves, rotates, scales or mirrors an entity that
    /// [`Entity::is_transformable`]: editable ones through their plan
    /// geometry, blocks and polylines as a whole (ADR 0016, D1). Anything
    /// else is [`DocumentError::NotEditable`].
    pub fn transform_entity(&mut self, id: EntityId, t: Transform2) -> DocumentResult<()> {
        let e = self.entity(id).ok_or(DocumentError::EntityNotFound(id))?;
        // A spline moves whole, its fit points and tangents with it (ADR
        // 0042 D4), not by its plan geometry, which holds no fit.
        if let Some(plan) = e.plan()
            && e.kind() != "SPLINE"
        {
            let moved = plan.with_geometry(t.apply(&plan.geometry())?);
            return self.set_plan_geometry(id, moved);
        }
        let raw = self
            .cad_mut()
            .get_entity_mut(handle(id))
            .ok_or(DocumentError::EntityNotFound(id))?;
        if !is_whole_transformable(raw) {
            let kind = raw.as_entity().entity_type();
            return Err(DocumentError::NotEditable { id, kind });
        }
        let spline = matches!(raw, EntityType::Spline(_));
        transform_whole(raw, t);
        if let Some(history) = crate::acis::history_of(raw) {
            self.drop_history(history);
        }
        // Of the kinds transformed whole, the model owns splines in the plan
        // only (ADR 0043 D4).
        if spline {
            self.sync_entity(id);
        }
        Ok(())
    }

    /// Sets a moved solid's construction history to none, as AutoCAD's
    /// History property does: its steps no longer say where it is
    /// (maintainer, 2026-11-05). Undone with the change.
    pub(crate) fn drop_history(&mut self, history: Handle) {
        use acadrust::objects::{DynamicBlockData, ObjectType};
        let recording = matches!(
            self.cad().objects.get(&history),
            Some(ObjectType::DynamicBlock(o))
                if matches!(&o.data, DynamicBlockData::SolidHistory(h) if h.record_history || h.show_history)
        );
        if !recording {
            return;
        }
        self.touch_structure();
        if let Some(ObjectType::DynamicBlock(o)) = self.cad_mut().objects.get_mut(&history)
            && let DynamicBlockData::SolidHistory(h) = &mut o.data
        {
            h.record_history = false;
            h.show_history = false;
        }
    }

    /// Adds a copy of a transformable entity to model space, with the same
    /// layer, color and other properties, where the original is; returns
    /// its id. A block's attributes get handles of their own; links the
    /// copy cannot share (reactors, an extension dictionary, cached proxy
    /// graphics) are left out, as copying drops them in AutoCAD too.
    pub fn copy_entity(&mut self, id: EntityId) -> DocumentResult<EntityId> {
        let original = self
            .cad()
            .get_entity(handle(id))
            .ok_or(DocumentError::EntityNotFound(id))?;
        if !read_entity(original).is_transformable() {
            let kind = original.as_entity().entity_type();
            return Err(DocumentError::NotEditable { id, kind });
        }
        self.clone_entity(id, Handle::NULL)
    }

    /// Deletes an entity of any type (erasing needs no geometry, D3.4).
    ///
    /// Uses the codec's shared-image removal, which keeps block membership so
    /// undo can restore it exactly.
    pub fn delete_entity(&mut self, id: EntityId) -> DocumentResult<()> {
        self.cad_mut()
            .remove_entity_arc(handle(id))
            .ok_or(DocumentError::EntityNotFound(id))?;
        self.model.entities.remove(&id);
        Ok(())
    }

    /// Draws a lightweight polyline on the current layer (PLINE): each
    /// vertex with the bulge of the segment leaving it (0 for a straight
    /// segment, the tangent of a quarter of an arc's angle otherwise,
    /// positive counterclockwise); `closed` joins the last vertex to the
    /// first. Two vertices at least, no segment of zero length.
    pub fn add_polyline(
        &mut self,
        vertices: &[(Point2, f64)],
        closed: bool,
    ) -> DocumentResult<EntityId> {
        self.add_polyline_with(vertices, closed, PolylineStyle::default())
    }

    /// [`add_polyline`](Self::add_polyline) at an elevation, with a
    /// thickness and a constant width (RECTANG's Elevation, Thickness and
    /// Width): each finite, the width not negative.
    pub fn add_polyline_with(
        &mut self,
        vertices: &[(Point2, f64)],
        closed: bool,
        style: PolylineStyle,
    ) -> DocumentResult<EntityId> {
        style.check()?;
        if vertices.len() < 2 {
            return Err(DocumentError::Geometry(GeometryError::OutOfRange {
                what: "polyline vertices",
            }));
        }
        let segments = if closed {
            vertices.len()
        } else {
            vertices.len() - 1
        };
        for i in 0..segments {
            let (a, bulge) = vertices[i];
            let b = vertices[(i + 1) % vertices.len()].0;
            if !bulge.is_finite() {
                return Err(DocumentError::Geometry(GeometryError::NonFinite {
                    what: "bulge",
                }));
            }
            if a == b {
                return Err(DocumentError::Geometry(GeometryError::ZeroLength));
            }
        }
        let mut polyline = LwPolyline::new();
        for &(point, bulge) in vertices {
            polyline.add_point_with_bulge(Vector2::new(point.x(), point.y()), bulge);
        }
        polyline.is_closed = closed;
        polyline.elevation = style.elevation;
        polyline.thickness = style.thickness;
        polyline.constant_width = style.width;
        polyline.common.layer = self.current_layer().name().to_owned();
        let h = self
            .cad_mut()
            .add_entity(EntityType::LwPolyline(polyline))?;
        self.sync_entity(h.value());
        Ok(h.value())
    }

    /// The drawing's default text height (TEXTSIZE), 2.5 if it holds none.
    pub fn text_size(&self) -> f64 {
        let size = self.cad().header.text_height;
        if size.is_finite() && size > 0.0 {
            size
        } else {
            2.5
        }
    }

    /// Writes a single-line TEXT, left-justified on the baseline at
    /// `anchor`, `rotation` radians from East, on the current layer with
    /// the current text style (ADR 0022 D1); returns its id.
    pub fn add_text(
        &mut self,
        content: &str,
        anchor: Point2,
        height: f64,
        rotation: f64,
    ) -> DocumentResult<EntityId> {
        check_single_line(content)?;
        check_text_sizes(Some(height), Some(rotation))?;
        let layer = self.current_layer().name().to_owned();
        let style = self.current_text_style();
        let text = new_text(content, anchor, height, rotation, &layer, &style);
        let h = self.cad_mut().add_entity(text)?;
        self.sync_entity(h.value());
        Ok(h.value())
    }

    /// Draws `ellipse`, an ellipse or elliptical arc in world plan
    /// coordinates, on the current layer at `elevation`, in the World plane
    /// (ELLIPSE, ADR 0040); returns its id.
    pub fn add_ellipse(
        &mut self,
        ellipse: arq_geometry::Ellipse,
        elevation: f64,
    ) -> DocumentResult<EntityId> {
        if !elevation.is_finite() {
            return Err(DocumentError::Geometry(GeometryError::NonFinite {
                what: "elevation",
            }));
        }
        let mut el = acadrust::entities::Ellipse::new();
        el.normal = Vector3::new(0.0, 0.0, 1.0);
        el.center = Vector3::new(0.0, 0.0, elevation);
        crate::drawing::write_plan_ellipse(&mut el, ellipse);
        el.common.layer = self.current_layer().name().to_owned();
        let h = self.cad_mut().add_entity(EntityType::Ellipse(el))?;
        self.sync_entity(h.value());
        Ok(h.value())
    }

    /// Draws `spline`, made by its control vertices (SPLINE's CV method,
    /// ADR 0042 D4), in world plan coordinates, on the current layer at
    /// `elevation`, in the World plane; returns its id.
    pub fn add_spline(
        &mut self,
        spline: &arq_geometry::Spline,
        elevation: f64,
    ) -> DocumentResult<EntityId> {
        let mut sp = new_spline(spline, elevation)?;
        // Not by fit points; DWG's closed bit as AutoCAD sets it.
        sp.dwg_flags1 = if spline.is_closed() { 4 } else { 0 };
        self.add_spline_entity(sp)
    }

    /// Draws the spline through `fit`'s points (SPLINE's Fit method, ADR
    /// 0042 D4), in world plan coordinates, on the current layer at
    /// `elevation`, in the World plane: its control points and knots as
    /// well as its fit points, knots method and tangents; returns its id.
    pub fn add_fit_spline(
        &mut self,
        fit: &arq_geometry::SplineFit,
        elevation: f64,
    ) -> DocumentResult<EntityId> {
        let mut sp = new_spline(&fit.spline()?, elevation)?;
        write_fit(&mut sp, fit, elevation);
        self.add_spline_entity(sp)
    }

    fn add_spline_entity(
        &mut self,
        mut sp: acadrust::entities::Spline,
    ) -> DocumentResult<EntityId> {
        sp.common.layer = self.current_layer().name().to_owned();
        let h = self.cad_mut().add_entity(EntityType::Spline(sp))?;
        self.sync_entity(h.value());
        Ok(h.value())
    }

    /// Writes an MTEXT on the current layer with the current text style
    /// (ADR 0022 D6): `content` is plain text whose line breaks are
    /// paragraphs, wrapped to `width` (0: no wrapping), hanging from
    /// `anchor` as `justification` says (`"TopLeft"`…); returns its id.
    pub fn add_mtext(
        &mut self,
        content: &str,
        anchor: Point2,
        height: f64,
        rotation: f64,
        width: f64,
        justification: &str,
    ) -> DocumentResult<EntityId> {
        if content.trim().is_empty() {
            return Err(DocumentError::InvalidText(
                "a text needs at least one character",
            ));
        }
        check_text_sizes(Some(height), Some(rotation))?;
        if !(width.is_finite() && width >= 0.0) {
            return Err(DocumentError::InvalidText(
                "a text width must be a finite number, zero or greater",
            ));
        }
        let attachment = attachment_named(justification).ok_or(DocumentError::InvalidText(
            "unknown multiline text justification",
        ))?;
        let layer = self.current_layer().name().to_owned();
        let style = self.current_text_style();
        let text = new_mtext(
            content, anchor, height, rotation, width, attachment, &layer, &style,
        );
        let h = self.cad_mut().add_entity(text)?;
        self.sync_entity(h.value());
        Ok(h.value())
    }

    /// The current text style (TEXTSTYLE), or "Standard" when the drawing
    /// lacks it.
    fn current_text_style(&self) -> String {
        let wanted = &self.cad().header.current_text_style_name;
        if self.cad().text_styles.get(wanted).is_some() {
            wanted.clone()
        } else {
            "Standard".to_owned()
        }
    }

    /// Sets what is given of a text's content, height and rotation (world
    /// radians). A single-line text takes one non-empty line; an MTEXT with
    /// formatting codes keeps its content, and Aligned and Fit texts their
    /// height and rotation (ADR 0022 D3, D4).
    pub fn set_text(
        &mut self,
        id: EntityId,
        content: Option<&str>,
        height: Option<f64>,
        rotation: Option<f64>,
    ) -> DocumentResult<()> {
        let e = self.entity(id).ok_or(DocumentError::EntityNotFound(id))?;
        let text = e
            .text()
            .ok_or(DocumentError::NotEditable { id, kind: e.kind() })?;
        if let Some(content) = content {
            if !text.content_editable() {
                return Err(DocumentError::UnsupportedOperation(
                    "this MTEXT holds formatting codes; its content is not edited yet".to_owned(),
                ));
            }
            if text.is_multiline() {
                if content.trim().is_empty() {
                    return Err(DocumentError::InvalidText(
                        "a text needs at least one character",
                    ));
                }
            } else {
                check_single_line(content)?;
            }
        }
        if (height.is_some() || rotation.is_some()) && !text.sizes_editable() {
            return Err(DocumentError::UnsupportedOperation(
                "an aligned or fit text takes its height and rotation from its points".to_owned(),
            ));
        }
        check_text_sizes(height, rotation)?;
        let raw = self
            .cad_mut()
            .get_entity_mut(handle(id))
            .ok_or(DocumentError::EntityNotFound(id))?;
        edit_text(raw, content, height, rotation);
        Ok(())
    }

    /// Moves an entity of any type to another layer.
    pub fn set_entity_layer(&mut self, id: EntityId, layer: LayerId) -> DocumentResult<()> {
        let name = self
            .layer(layer)
            .ok_or(DocumentError::LayerIdNotFound(layer))?
            .name()
            .to_owned();
        let e = self
            .cad_mut()
            .get_entity_mut(handle(id))
            .ok_or(DocumentError::EntityNotFound(id))?;
        let common = e.common_mut();
        common.layer = name;
        common.layer_handle = Some(acadrust::Handle::new(layer));
        self.sync_entity(id);
        Ok(())
    }

    /// Sets the color of an entity of any type.
    pub fn set_entity_color(&mut self, id: EntityId, color: Color) -> DocumentResult<()> {
        if color == Color::Index(0) {
            return Err(DocumentError::InvalidColorIndex(0));
        }
        let e = self
            .cad_mut()
            .get_entity_mut(handle(id))
            .ok_or(DocumentError::EntityNotFound(id))?;
        e.common_mut().color = color_to_codec(color);
        self.sync_entity(id);
        Ok(())
    }
}

fn check_single_line(content: &str) -> DocumentResult<()> {
    if content.is_empty() {
        return Err(DocumentError::InvalidText(
            "a text needs at least one character",
        ));
    }
    if content.contains(['\n', '\r']) {
        return Err(DocumentError::InvalidText(
            "a single-line text holds one line",
        ));
    }
    Ok(())
}

fn check_text_sizes(height: Option<f64>, rotation: Option<f64>) -> DocumentResult<()> {
    if height.is_some_and(|h| !(h.is_finite() && h > 0.0)) {
        return Err(DocumentError::InvalidText(
            "a text height must be a finite number greater than zero",
        ));
    }
    if rotation.is_some_and(|r| !r.is_finite()) {
        return Err(DocumentError::InvalidText(
            "a text rotation must be a finite number",
        ));
    }
    Ok(())
}

/// Test helpers for other crates (feature `test-support`).
#[cfg(feature = "test-support")]
impl Document {
    /// Inserts a POINT on layer "0": an entity that is read-only in this
    /// version (it cannot be transformed or copied).
    pub fn insert_point_for_tests(&mut self, x: f64, y: f64) -> EntityId {
        let point = acadrust::Point::from_coords(x, y, 0.0);
        self.cad_mut()
            .add_entity(acadrust::EntityType::Point(point))
            .expect("adding a point")
            .value()
    }

    /// Inserts a single-line TEXT on layer "0", as a drawing from another
    /// CAD would contain.
    pub fn insert_text_for_tests(&mut self, value: &str, x: f64, y: f64) -> EntityId {
        let text = acadrust::Text::with_value(value, acadrust::types::Vector3::new(x, y, 0.0));
        self.cad_mut()
            .add_entity(acadrust::EntityType::Text(text))
            .expect("adding a text")
            .value()
    }

    /// Attaches XDATA of application `app` to an entity: data the platform
    /// does not show, which only the inventory of a save sees.
    pub fn attach_xdata_for_tests(&mut self, id: EntityId, app: &str) {
        let _ = self
            .cad_mut()
            .app_ids
            .add(acadrust::tables::AppId::new(app));
        let mut record = acadrust::xdata::ExtendedDataRecord::new(app);
        record.add_value(acadrust::xdata::XDataValue::String("kept?".to_owned()));
        self.cad_mut()
            .get_entity_mut(acadrust::Handle::new(id))
            .expect("an entity")
            .common_mut()
            .extended_data
            .add_record(record);
    }

    /// A fault for [`damage_next_save_for_tests`](Self::damage_next_save_for_tests):
    /// drops every XDATA group (codes 1000 to 1071) from a DXF.
    pub fn drop_dxf_xdata_for_tests(bytes: &mut Vec<u8>) {
        let text = String::from_utf8(bytes.clone()).expect("a DXF");
        let lines: Vec<&str> = text.split('\n').collect();
        let mut kept = Vec::new();
        let mut i = 0;
        while i + 1 < lines.len() {
            let code: i32 = lines[i].trim().parse().unwrap_or(0);
            if !(1000..=1071).contains(&code) {
                kept.push(lines[i]);
                kept.push(lines[i + 1]);
            }
            i += 2;
        }
        kept.extend(&lines[i..]);
        *bytes = kept.join("\n").into_bytes();
    }

    /// Sets the file-format version the next save writes, by code
    /// (`"AC1015"`…); `false` for an unknown code. Choosing the version is
    /// not a user feature yet; the round-trip corpus needs it.
    pub fn set_version_for_tests(&mut self, code: &str) -> bool {
        match acadrust::types::DxfVersion::parse(code) {
            Some(version) => {
                self.cad_mut().version = version;
                true
            }
            None => false,
        }
    }
}

/// A SPLINE of `spline`'s control points at `elevation`, its knots and
/// weights, planar, in the World plane; periodic when closed, as AutoCAD
/// makes a closed one.
/// Writes `fit`'s fit points at height `z`, its knots method and tangents
/// into `sp`, marked as made by fit points.
fn write_fit(sp: &mut acadrust::entities::Spline, fit: &arq_geometry::SplineFit, z: f64) {
    let at = |p: Point2| Vector3::new(p.x(), p.y(), z);
    let direction = |t: Option<arq_geometry::Vector2>| {
        t.map_or(Vector3::ZERO, |d| Vector3::new(d.x(), d.y(), 0.0))
    };
    sp.fit_points = fit.points().iter().copied().map(at).collect();
    sp.knot_parameterization = fit.knots().code();
    sp.begin_tangent = direction(fit.start_tangent());
    sp.end_tangent = direction(fit.end_tangent());
    // By fit points, with their knot parameterization.
    sp.dwg_flags1 = 1 | 8 | if fit.is_closed() { 4 } else { 0 };
}

pub(crate) fn new_spline(
    spline: &arq_geometry::Spline,
    elevation: f64,
) -> DocumentResult<acadrust::entities::Spline> {
    if !elevation.is_finite() {
        return Err(DocumentError::Geometry(GeometryError::NonFinite {
            what: "elevation",
        }));
    }
    let mut sp = acadrust::entities::Spline::new();
    sp.normal = Vector3::new(0.0, 0.0, 1.0);
    sp.degree = i32::try_from(spline.degree()).expect("a spline's degree is at most 25");
    sp.knots = spline.knots().to_vec();
    sp.control_points = spline
        .points()
        .iter()
        .map(|p| Vector3::new(p.x(), p.y(), elevation))
        .collect();
    sp.weights = spline.weights().to_vec();
    sp.flags.closed = spline.is_closed();
    sp.flags.periodic = spline.is_closed();
    sp.flags.rational = !spline.weights().is_empty();
    sp.flags.planar = true;
    sp.flags.linear = spline.degree() == 1;
    // AutoCAD's own tolerances for a spline it makes.
    sp.knot_tolerance = 1e-10;
    sp.control_tolerance = 1e-10;
    Ok(sp)
}

#[cfg(test)]
#[path = "spline_make_tests.rs"]
mod spline_make_tests;

#[cfg(test)]
#[path = "whole_tests.rs"]
mod whole_tests;

#[cfg(test)]
#[path = "plan_ellipse_tests.rs"]
mod plan_ellipse_tests;

#[cfg(test)]
#[path = "plan_spline_tests.rs"]
mod plan_spline_tests;

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI};

    use acadrust::types::Vector3;
    use acadrust::{EntityType, Handle};
    use arq_geometry::{Arc, Circle, Geometry2, Line, Point2, Tolerance, Transform2, Vector2};

    use super::*;
    use crate::entity::Color;

    const TOL: Tolerance = Tolerance::DEFAULT;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    fn plan(g: impl Into<Geometry2>, z: f64) -> PlanGeometry {
        PlanGeometry::at_elevation(g.into(), z).unwrap()
    }

    fn layer0(doc: &Document) -> LayerId {
        doc.layer_by_name("0").unwrap().id()
    }

    /// Adds a raw codec entity, as a file written by another CAD would contain.
    fn add_raw(doc: &mut Document, e: EntityType) -> EntityId {
        doc.cad_mut().add_entity(e).unwrap().value()
    }

    fn codec_arc(center: (f64, f64, f64), r: f64, s: f64, e: f64, normal_z: f64) -> EntityType {
        let mut a = acadrust::Arc::new();
        a.center = Vector3::new(center.0, center.1, center.2);
        a.radius = r;
        a.start_angle = s;
        a.end_angle = e;
        a.normal = Vector3::new(0.0, 0.0, normal_z);
        EntityType::Arc(a)
    }

    /// A 4 × 2 rectangle (OCS), 0.25 wide, at elevation 3, 1.5 thick, on
    /// a plane whose normal is `normal_z` up.
    fn wide_box(doc: &mut Document, normal_z: f64) -> EntityId {
        let mut pl = LwPolyline::new();
        for (x, y) in [(0.0, 0.0), (4.0, 0.0), (4.0, 2.0), (0.0, 2.0)] {
            pl.add_point_with_bulge(acadrust::types::Vector2::new(x, y), 0.0);
        }
        pl.is_closed = true;
        pl.constant_width = 0.25;
        pl.elevation = 3.0;
        pl.thickness = 1.5;
        pl.normal = Vector3::new(0.0, 0.0, normal_z);
        add_raw(doc, EntityType::LwPolyline(pl))
    }

    fn ocs_points(doc: &Document, id: EntityId) -> (Vec<(f64, f64)>, LwPolyline) {
        let Some(EntityType::LwPolyline(pl)) = doc.cad().get_entity(handle(id)) else {
            panic!("a polyline")
        };
        let r = |v: f64| (v * 1e9).round() / 1e9 + 0.0;
        let points = pl
            .vertices
            .iter()
            .map(|v| (r(v.location.x), r(v.location.y)))
            .collect();
        (points, pl.clone())
    }

    #[test]
    fn a_polyline_offset_keeps_its_properties_in_its_plane() {
        let mut doc = Document::new();
        let id = wide_box(&mut doc, 1.0);
        let inner = doc.offset_polyline(id, Some(0.5), p(2.0, 1.0)).unwrap();
        let (points, pl) = ocs_points(&doc, inner);
        assert_eq!(points, [(0.5, 0.5), (3.5, 0.5), (3.5, 1.5), (0.5, 1.5)]);
        assert!(pl.is_closed);
        assert_eq!(
            (pl.constant_width, pl.elevation, pl.thickness),
            (0.25, 3.0, 1.5),
            "every property kept"
        );
        assert_ne!(inner, id, "a copy");
        // Through a point 1 outside.
        let through = doc.offset_polyline(id, None, p(2.0, -1.0)).unwrap();
        let (points, _) = ocs_points(&doc, through);
        assert_eq!(points, [(-1.0, -1.0), (5.0, -1.0), (5.0, 3.0), (-1.0, 3.0)]);
    }

    #[test]
    fn a_mirrored_polyline_is_offset_toward_the_world_point() {
        // Seen from below, OCS x runs west: the box spans world x -4 to 0.
        let mut doc = Document::new();
        let id = wide_box(&mut doc, -1.0);
        let inner = doc.offset_polyline(id, Some(0.5), p(-2.0, 1.0)).unwrap();
        assert_eq!(
            ocs_points(&doc, inner).0,
            [(0.5, 0.5), (3.5, 0.5), (3.5, 1.5), (0.5, 1.5)],
            "inside: the world point is inside the box"
        );
        let outer = doc.offset_polyline(id, Some(0.5), p(1.0, 1.0)).unwrap();
        assert_eq!(
            ocs_points(&doc, outer).0,
            [(-0.5, -0.5), (4.5, -0.5), (4.5, 2.5), (-0.5, 2.5)]
        );
    }

    #[test]
    fn what_is_not_a_plan_polyline_is_not_offset_here() {
        let mut doc = Document::new();
        let line = add_raw(&mut doc, {
            let mut l = acadrust::Line::new();
            l.end = Vector3::new(1.0, 0.0, 0.0);
            EntityType::Line(l)
        });
        assert!(matches!(
            doc.offset_polyline(line, Some(1.0), p(0.0, 1.0)),
            Err(DocumentError::NotEditable { .. })
        ));
        let tilted = wide_box(&mut doc, 0.0);
        if let Some(EntityType::LwPolyline(pl)) = doc.cad_mut().get_entity_mut(handle(tilted)) {
            pl.normal = Vector3::new(1.0, 0.0, 0.0);
        }
        assert!(doc.offset_polyline(tilted, Some(1.0), p(0.0, 1.0)).is_err());
        let before = doc.entities().len();
        let id = wide_box(&mut doc, 1.0);
        assert!(
            doc.offset_polyline(id, Some(1.0), p(2.0, 1.0)).is_err(),
            "inward to its middle: nothing left"
        );
        assert_eq!(doc.entities().len(), before + 1, "nothing added");
    }

    #[test]
    fn a_mirrored_polyline_is_trimmed_and_extended_where_it_is_seen() {
        // Seen from below, the box spans world x -4 to 0, y 0 to 2.
        let mut doc = Document::new();
        let id = wide_box(&mut doc, -1.0);
        let cutter = Geometry2::Line(Line::new(p(-2.0, -1.0), p(-2.0, 3.0)).unwrap());
        let left = doc
            .trim_polyline(id, p(-4.0, 1.0), &[cutter], false)
            .unwrap();
        assert_eq!(left, [id]);
        let WorldPolyline {
            vertices: world,
            closed,
            ..
        } = doc.polyline_in_world(id).unwrap();
        assert!(!closed, "opened");
        let xs: Vec<f64> = world.iter().map(|(q, _)| q.x()).collect();
        assert!(
            xs.iter().all(|&x| (-2.0 - 1e-9..=1e-9).contains(&x)),
            "the half nearer 0: {xs:?}"
        );
        let pl = ocs_points(&doc, id).1;
        assert_eq!((pl.constant_width, pl.elevation), (0.25, 3.0), "kept");
        // Extended from its end at world (-2, y) back to a line at x = -5.
        let wall = Geometry2::Line(Line::new(p(-5.0, -10.0), p(-5.0, 10.0)).unwrap());
        let end = world.last().unwrap().0;
        doc.extend_polyline(id, end, &[wall]).unwrap();
        let world = doc.polyline_in_world(id).unwrap().vertices;
        assert!(
            (world.last().unwrap().0.x() + 5.0).abs() < 1e-9,
            "{world:?}"
        );
    }

    #[test]
    fn added_entities_read_back_exactly() {
        let mut doc = Document::new();
        let shapes: [Geometry2; 3] = [
            Line::new(p(0.0, 0.0), p(10.0, 5.0)).unwrap().into(),
            Circle::new(p(3.0, 4.0), 2.5).unwrap().into(),
            Arc::new(p(1.0, 1.0), 4.0, 0.25, 2.0).unwrap().into(),
        ];
        let ids: Vec<_> = shapes
            .iter()
            .map(|g| doc.add_entity(plan(g.clone(), 7.5), layer0(&doc)).unwrap())
            .collect();
        assert_eq!(
            ids.len(),
            ids.iter().collect::<std::collections::HashSet<_>>().len(),
            "unique ids"
        );
        for (id, g) in ids.iter().zip(shapes) {
            let e = doc.entity(*id).unwrap();
            assert_eq!(e.plan().unwrap().geometry(), g);
            assert_eq!(e.plan().unwrap().elevation(), 7.5);
            assert_eq!((e.layer(), e.color()), ("0", Color::ByLayer));
            assert_eq!(e.bounds(), Some(g.bounds()));
            assert!(e.is_editable());
        }
        let kinds: Vec<_> = doc.entities().iter().map(|e| e.kind()).collect();
        assert_eq!(kinds, ["LINE", "CIRCLE", "ARC"]);
    }

    #[test]
    fn unknown_layer_is_rejected() {
        let line = Line::new(p(0.0, 0.0), p(1.0, 0.0)).unwrap();
        let r = Document::new().add_entity(plan(line, 0.0), 424_242);
        assert_eq!(r, Err(DocumentError::LayerIdNotFound(424_242)));
    }

    #[test]
    fn dwg_and_dxf_round_trip_keep_ids_and_geometry() {
        let dir = tempfile::tempdir().unwrap();
        let mut doc = Document::new();
        let (x, y) = (500_000.0, 9_977_000.0); // UTM 17S
        let ids = [
            doc.add_entity(
                plan(Line::new(p(x, y), p(x + 12.0, y + 3.0)).unwrap(), 2_850.0),
                layer0(&doc),
            )
            .unwrap(),
            doc.add_entity(
                plan(Circle::new(p(x + 5.0, y), 1.25).unwrap(), 2_850.0),
                layer0(&doc),
            )
            .unwrap(),
            doc.add_entity(
                plan(Arc::new(p(x, y + 8.0), 3.0, 5.5, 0.4).unwrap(), 2_850.0),
                layer0(&doc),
            )
            .unwrap(),
        ];
        for name in ["r.dwg", "r.dxf"] {
            let path = dir.path().join(name);
            doc.save(&path).unwrap();
            let back = Document::open(&path).unwrap();
            for id in ids {
                let (a, b) = (doc.entity(id).unwrap(), back.entity(id).unwrap());
                let (ga, gb) = (a.plan().unwrap().geometry(), b.plan().unwrap().geometry());
                assert_eq!(a.kind(), b.kind(), "{name}");
                assert!(
                    (b.plan().unwrap().elevation() - 2_850.0).abs() < 1e-9,
                    "{name}"
                );
                for t in [0.0, 0.5, 1.0] {
                    assert!(
                        ga.point_at(t).approx_eq(gb.point_at(t), TOL),
                        "{name}: {id} at {t}"
                    );
                }
            }
        }
    }

    #[test]
    fn mirrored_ocs_arc_is_read_in_world_coordinates() {
        // Written by another CAD with normal (0, 0, -1): OCS center (2, 3, 4),
        // radius 5, from 0 to 90°.
        let mut doc = Document::new();
        let id = add_raw(
            &mut doc,
            codec_arc((2.0, 3.0, 4.0), 5.0, 0.0, FRAC_PI_2, -1.0),
        );
        let e = doc.entity(id).unwrap();
        let pl = e.plan().expect("a mirrored-OCS arc is still in plan");
        let Geometry2::Arc(a) = pl.geometry() else {
            panic!("not an arc")
        };
        assert_eq!(pl.elevation(), -4.0);
        assert!(a.center().approx_eq(p(-2.0, 3.0), TOL));
        // OCS end (2, 8) becomes the world start; OCS start (7, 3) the world end.
        assert!(a.start_point().approx_eq(p(-2.0, 8.0), TOL));
        assert!(a.end_point().approx_eq(p(-7.0, 3.0), TOL));
        assert!((a.start_angle() - FRAC_PI_2).abs() < 1e-12 && (a.end_angle() - PI).abs() < 1e-12);
    }

    #[test]
    fn editing_a_mirrored_arc_keeps_its_ocs() {
        let mut doc = Document::new();
        let id = add_raw(
            &mut doc,
            codec_arc((2.0, 3.0, 4.0), 5.0, 0.0, FRAC_PI_2, -1.0),
        );
        let before = doc.entity(id).unwrap().plan().unwrap();
        let moved = Transform2::translation(Vector2::new(10.0, -1.0).unwrap())
            .apply(&before.geometry())
            .unwrap();
        doc.set_plan_geometry(id, before.with_geometry(moved.clone()))
            .unwrap();
        let after = doc.entity(id).unwrap().plan().unwrap();
        for t in [0.0, 0.3, 1.0] {
            assert!(
                after
                    .geometry()
                    .point_at(t)
                    .approx_eq(moved.point_at(t), TOL)
            );
        }
        assert_eq!(after.elevation(), -4.0);
        let Some(EntityType::Arc(raw)) = doc.cad().get_entity(Handle::new(id)) else {
            panic!()
        };
        assert_eq!(raw.normal.z, -1.0, "still stored as the other CAD wrote it");
    }

    #[test]
    fn edits_keep_color_thickness_and_layer() {
        let mut doc = Document::new();
        let mut c = acadrust::Circle::new();
        c.radius = 2.0;
        c.thickness = 3.5;
        c.common.color = acadrust::Color::Index(1);
        let id = add_raw(&mut doc, EntityType::Circle(c));
        let bigger = Circle::new(p(1.0, 1.0), 4.0).unwrap();
        doc.set_plan_geometry(id, plan(bigger, 0.0)).unwrap();
        let e = doc.entity(id).unwrap();
        assert_eq!(e.color(), Color::Index(1));
        assert_eq!(e.plan().unwrap().geometry(), bigger.into());
        let Some(EntityType::Circle(raw)) = doc.cad().get_entity(Handle::new(id)) else {
            panic!()
        };
        assert_eq!(raw.thickness, 3.5);
    }

    #[test]
    fn tilted_sloped_and_unsupported_entities_are_read_only() {
        let mut doc = Document::new();
        let mut tilted = acadrust::Circle::new();
        tilted.radius = 1.0;
        tilted.normal = Vector3::new(1.0, 0.0, 0.0);
        let tilted = add_raw(&mut doc, EntityType::Circle(tilted));
        let sloped = add_raw(
            &mut doc,
            EntityType::Line(acadrust::Line::from_coords(0.0, 0.0, 0.0, 5.0, 0.0, 2.0)),
        );
        let text = add_raw(
            &mut doc,
            EntityType::Text(acadrust::Text::with_value(
                "PLANTA",
                Vector3::new(1.0, 2.0, 0.0),
            )),
        );
        for (id, kind) in [(tilted, "CIRCLE"), (sloped, "LINE"), (text, "TEXT")] {
            let e = doc.entity(id).unwrap();
            assert_eq!(e.kind(), kind);
            assert!(!e.is_editable(), "{kind} must be read-only");
            let any = plan(Circle::new(p(0.0, 0.0), 1.0).unwrap(), 0.0);
            assert_eq!(
                doc.set_plan_geometry(id, any),
                Err(DocumentError::NotEditable { id, kind })
            );
        }
        assert_eq!(
            doc.entities().len(),
            3,
            "read-only entities are still listed"
        );
    }

    #[test]
    fn kind_mismatch_and_missing_entity() {
        let mut doc = Document::new();
        let id = doc
            .add_entity(
                plan(Line::new(p(0.0, 0.0), p(1.0, 0.0)).unwrap(), 0.0),
                layer0(&doc),
            )
            .unwrap();
        let circle = plan(Circle::new(p(0.0, 0.0), 1.0).unwrap(), 0.0);
        assert_eq!(
            doc.set_plan_geometry(id, circle.clone()),
            Err(DocumentError::KindMismatch { id, kind: "LINE" })
        );
        assert_eq!(
            doc.set_plan_geometry(999_999, circle),
            Err(DocumentError::EntityNotFound(999_999))
        );
        assert!(doc.entity(999_999).is_none());
    }

    mod properties {
        use super::*;
        use proptest::prelude::*;
        use std::f64::consts::TAU;

        fn geometry() -> impl Strategy<Value = Geometry2> {
            let c = -1e6..1e6f64;
            prop_oneof![
                (c.clone(), c.clone(), c.clone(), c.clone())
                    .prop_filter("length", |(a, b, x, y)| (a - x).hypot(b - y) > 1e-3)
                    .prop_map(|(a, b, x, y)| Line::new(p(a, b), p(x, y)).unwrap().into()),
                (c.clone(), c.clone(), 1e-3..1e5f64)
                    .prop_map(|(x, y, r)| Circle::new(p(x, y), r).unwrap().into()),
                (c.clone(), c, 1e-3..1e5f64, 0.0..TAU, 0.01..(TAU - 0.01))
                    .prop_map(|(x, y, r, s, w)| Arc::new(p(x, y), r, s, s + w).unwrap().into()),
            ]
        }

        proptest! {
            /// Whatever plan geometry is written, through either OCS
            /// orientation, the same geometry is read back.
            #[test]
            fn write_then_read_returns_the_geometry(
                g in geometry(), z in -1e4..1e4f64, mirrored in any::<bool>(),
            ) {
                let mut doc = Document::new();
                let nz = if mirrored { -1.0 } else { 1.0 };
                let raw = match g {
                    Geometry2::Line(_) => EntityType::Line(acadrust::Line::from_coords(0.0, 0.0, 0.0, 1.0, 0.0, 0.0)),
                    Geometry2::Circle(_) => { let mut c = acadrust::Circle::new(); c.radius = 1.0; c.normal = Vector3::new(0.0, 0.0, nz); EntityType::Circle(c) }
                    Geometry2::Arc(_) => codec_arc((0.0, 0.0, 0.0), 1.0, 0.0, 1.0, nz),
                    Geometry2::Ellipse(_) | Geometry2::Spline(_) => {
                        unreachable!("the strategy makes no ellipse or spline")
                    }
                };
                let id = add_raw(&mut doc, raw);
                doc.set_plan_geometry(id, plan(g.clone(), z)).unwrap();
                let back = doc.entity(id).unwrap().plan().unwrap();
                let tol = Tolerance::new(1e-12 * (1.0 + 2e6), 1e-9).unwrap();
                for t in [0.0, 0.37, 1.0] {
                    prop_assert!(back.geometry().point_at(t).approx_eq(g.point_at(t), tol));
                }
                prop_assert!((back.elevation() - z).abs() <= 1e-9 * (1.0 + z.abs()));
                prop_assert_eq!(back.geometry().kind(), g.kind());
            }
        }
    }
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod text_tests;

#[cfg(test)]
#[path = "stretch_tests.rs"]
mod stretch_tests;

#[cfg(test)]
#[path = "break_polyline_tests.rs"]
mod break_polyline_tests;

#[cfg(test)]
#[path = "join_polyline_tests.rs"]
mod join_polyline_tests;

#[cfg(test)]
#[path = "dxf_width_tests.rs"]
mod dxf_width_tests;
