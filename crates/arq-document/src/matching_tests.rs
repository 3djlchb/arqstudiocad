//! MATCHPROP gives a source's properties to a target: the basic ones, the
//! thickness, a polyline's uniform width and linetype generation; only the
//! settings chosen, and nothing when the target already matches.
use acadrust::types::{LineWeight, Transparency, Vector2};
use arq_geometry::{Circle, Line, Point2};

use super::*;
use crate::entity::{Color, PlanGeometry};

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn line(doc: &mut Document) -> EntityId {
    let layer = doc.current_layer().id();
    let plan = PlanGeometry::new(Line::new(p(0.0, 0.0), p(1.0, 0.0)).unwrap().into());
    doc.add_entity(plan, layer).unwrap()
}

fn circle(doc: &mut Document) -> EntityId {
    let layer = doc.current_layer().id();
    let plan = PlanGeometry::new(Circle::new(p(5.0, 5.0), 1.0).unwrap().into());
    doc.add_entity(plan, layer).unwrap()
}

fn raw(doc: &Document, id: EntityId) -> &EntityType {
    doc.cad().get_entity(handle(id)).unwrap()
}

fn polyline(doc: &mut Document, constant: f64, widths: &[(f64, f64)]) -> EntityId {
    let mut poly = LwPolyline::new();
    for (k, x) in [0.0, 1.0, 2.0].into_iter().enumerate() {
        poly.add_point_with_bulge(Vector2::new(x, 0.0), 0.0);
        if let Some(&(a, b)) = widths.get(k) {
            poly.vertices[k].start_width = a;
            poly.vertices[k].end_width = b;
        }
    }
    poly.constant_width = constant;
    doc.cad_mut()
        .add_entity(EntityType::LwPolyline(poly))
        .unwrap()
        .value()
}

/// A source line on its own layer, with every basic property set.
fn styled_source(doc: &mut Document) -> EntityId {
    let walls = doc.create_layer("A-WALLS").unwrap();
    let source = line(doc);
    doc.set_entity_layer(source, walls).unwrap();
    doc.set_entity_color(source, Color::Index(3)).unwrap();
    let EntityType::Line(l) = doc.cad_mut().get_entity_mut(handle(source)).unwrap() else {
        unreachable!()
    };
    l.common.linetype_scale = 2.5;
    l.common.line_weight = LineWeight::Value(50);
    l.common.transparency = Transparency::from_percent(40.0);
    l.thickness = 4.0;
    source
}

#[test]
fn every_basic_property_and_the_thickness_go_to_the_target() {
    let mut doc = Document::new();
    let source = styled_source(&mut doc);
    let target = circle(&mut doc);
    assert!(
        doc.match_properties(source, target, MatchSettings::ALL)
            .unwrap()
    );
    let (from, to) = (raw(&doc, source).common(), raw(&doc, target).common());
    assert_eq!(to.layer, "A-WALLS");
    assert_eq!(to.layer_handle, from.layer_handle);
    assert_eq!(to.color, from.color);
    assert_eq!(to.linetype_scale, 2.5);
    assert_eq!(to.line_weight, LineWeight::Value(50));
    assert_eq!(to.transparency, from.transparency);
    let EntityType::Circle(c) = raw(&doc, target) else {
        unreachable!()
    };
    assert_eq!(c.thickness, 4.0);
    assert_eq!(doc.entity(target).unwrap().color(), Color::Index(3));
    assert!(doc.check_model().is_empty());
    assert!(
        !doc.match_properties(source, target, MatchSettings::ALL)
            .unwrap(),
        "already matching: nothing changes"
    );
}

#[test]
fn only_the_properties_chosen_go() {
    let mut doc = Document::new();
    let source = styled_source(&mut doc);
    let target = line(&mut doc);
    let only_color = MatchSettings {
        color: true,
        ..MatchSettings::NONE
    };
    assert!(doc.match_properties(source, target, only_color).unwrap());
    let to = raw(&doc, target).common();
    assert_eq!(doc.entity(target).unwrap().color(), Color::Index(3));
    assert_ne!(to.layer, "A-WALLS");
    assert_eq!(to.linetype_scale, 1.0);
    let EntityType::Line(l) = raw(&doc, target) else {
        unreachable!()
    };
    assert_eq!(l.thickness, 0.0);
    assert!(
        !doc.match_properties(source, target, MatchSettings::NONE)
            .unwrap()
    );
}

#[test]
fn a_polyline_gives_a_uniform_width_and_its_linetype_generation() {
    let mut doc = Document::new();
    let source = polyline(&mut doc, 0.5, &[]);
    let EntityType::LwPolyline(s) = doc.cad_mut().get_entity_mut(handle(source)).unwrap() else {
        unreachable!()
    };
    s.plinegen = true;
    let target = polyline(&mut doc, 0.0, &[(0.1, 0.2), (0.3, 0.4)]);
    assert!(
        doc.match_properties(source, target, MatchSettings::ALL)
            .unwrap()
    );
    let EntityType::LwPolyline(t) = raw(&doc, target) else {
        unreachable!()
    };
    assert!(t.plinegen);
    assert_eq!(t.constant_width, 0.5);
    assert!(
        t.vertices
            .iter()
            .all(|v| v.start_width == 0.0 && v.end_width == 0.0)
    );
}

#[test]
fn a_varying_width_is_not_given_as_in_autocad() {
    let mut doc = Document::new();
    let source = polyline(&mut doc, 0.0, &[(0.1, 0.9), (0.9, 0.1)]);
    let target = polyline(&mut doc, 0.25, &[]);
    doc.match_properties(source, target, MatchSettings::ALL)
        .unwrap();
    let EntityType::LwPolyline(t) = raw(&doc, target) else {
        unreachable!()
    };
    assert_eq!(t.constant_width, 0.25, "its own width kept");
}

#[test]
fn matched_properties_survive_dwg_and_dxf() {
    let mut doc = Document::new();
    let source = styled_source(&mut doc);
    let target = circle(&mut doc);
    doc.match_properties(source, target, MatchSettings::ALL)
        .unwrap();
    let dxf = Document::from_dxf_bytes(doc.to_dxf_bytes().unwrap()).unwrap();
    let dwg = Document::read(
        doc.to_dwg_bytes().unwrap(),
        crate::format::FileFormat::Dwg,
        std::path::Path::new("matchprop.dwg"),
    )
    .unwrap();
    for back in [dxf, dwg] {
        let circles: Vec<_> = back
            .cad()
            .entities()
            .filter_map(|e| match e {
                EntityType::Circle(c) => Some(c.clone()),
                _ => None,
            })
            .collect();
        let [c] = &circles[..] else {
            panic!("{} circles", circles.len())
        };
        assert_eq!(c.common.layer, "A-WALLS");
        assert_eq!(c.common.linetype_scale, 2.5);
        assert_eq!(c.common.line_weight, LineWeight::Value(50));
        assert_eq!(c.common.transparency, Transparency::from_percent(40.0));
        assert_eq!(c.thickness, 4.0);
        let id = c.common.handle.value();
        assert_eq!(back.entity(id).unwrap().color(), Color::Index(3));
    }
}

/// A text in a style of its own, `ROTULO`, height 2.5, narrowed and slanted.
fn styled_text(doc: &mut Document) -> EntityId {
    let mut style = acadrust::tables::TextStyle::new("ROTULO");
    style.handle = doc.cad_mut().allocate_handle();
    doc.cad_mut().text_styles.add(style).unwrap();
    let id = doc.add_text("A-101", p(0.0, 0.0), 2.5, 0.3).unwrap();
    let EntityType::Text(t) = doc.cad_mut().get_entity_mut(handle(id)).unwrap() else {
        unreachable!()
    };
    t.style = "ROTULO".to_owned();
    t.width_factor = 0.8;
    t.oblique_angle = 0.25;
    id
}

#[test]
fn a_text_gives_its_style_height_width_and_slant_but_not_its_rotation() {
    let mut doc = Document::new();
    let source = styled_text(&mut doc);
    let target = doc.add_text("B-202", p(9.0, 9.0), 1.0, 0.0).unwrap();
    assert!(
        doc.match_properties(source, target, MatchSettings::ALL)
            .unwrap()
    );
    let EntityType::Text(t) = raw(&doc, target) else {
        unreachable!()
    };
    assert_eq!(
        (t.style.as_str(), t.height, t.width_factor, t.oblique_angle),
        ("ROTULO", 2.5, 0.8, 0.25)
    );
    assert_eq!(t.rotation, 0.0, "its own rotation");
    assert_eq!(t.value, "B-202", "its own words");
    assert!(doc.check_model().is_empty());
}

#[test]
fn an_mtext_takes_and_gives_only_the_style_and_height() {
    let mut doc = Document::new();
    let source = styled_text(&mut doc);
    let mtext = doc
        .add_mtext("Notes", p(0.0, 5.0), 1.0, 0.0, 10.0, "TopLeft")
        .unwrap();
    assert!(
        doc.match_properties(source, mtext, MatchSettings::ALL)
            .unwrap()
    );
    let EntityType::MText(m) = raw(&doc, mtext) else {
        unreachable!()
    };
    assert_eq!((m.style.as_str(), m.height), ("ROTULO", 2.5));
    // Back to a text: its width factor and slant stay its own.
    let text = doc.add_text("C", p(0.0, 8.0), 1.0, 0.0).unwrap();
    let EntityType::Text(t) = doc.cad_mut().get_entity_mut(handle(text)).unwrap() else {
        unreachable!()
    };
    (t.width_factor, t.oblique_angle) = (0.6, 0.1);
    doc.match_properties(mtext, text, MatchSettings::ALL)
        .unwrap();
    let EntityType::Text(t) = raw(&doc, text) else {
        unreachable!()
    };
    assert_eq!((t.style.as_str(), t.height), ("ROTULO", 2.5));
    assert_eq!(
        (t.width_factor, t.oblique_angle),
        (0.6, 0.1),
        "its own kept"
    );
}

#[test]
fn the_text_setting_off_leaves_texts_alone() {
    let mut doc = Document::new();
    let source = styled_text(&mut doc);
    let target = doc.add_text("D", p(0.0, 3.0), 1.0, 0.0).unwrap();
    let without_text = MatchSettings {
        text: false,
        ..MatchSettings::ALL
    };
    doc.match_properties(source, target, without_text).unwrap();
    let EntityType::Text(t) = raw(&doc, target) else {
        unreachable!()
    };
    assert_eq!(t.height, 1.0);
    assert_ne!(t.style, "ROTULO");
}

#[test]
fn a_texts_look_survives_dwg_and_dxf() {
    let mut doc = Document::new();
    let source = styled_text(&mut doc);
    let target = doc.add_text("E", p(0.0, 4.0), 1.0, 0.0).unwrap();
    doc.match_properties(source, target, MatchSettings::ALL)
        .unwrap();
    let dxf = Document::from_dxf_bytes(doc.to_dxf_bytes().unwrap()).unwrap();
    let dwg = Document::read(
        doc.to_dwg_bytes().unwrap(),
        crate::format::FileFormat::Dwg,
        std::path::Path::new("text.dwg"),
    )
    .unwrap();
    for back in [dxf, dwg] {
        let found: Vec<_> = back
            .cad()
            .entities()
            .filter_map(|e| match e {
                EntityType::Text(t) if t.value == "E" => Some(t.clone()),
                _ => None,
            })
            .collect();
        let [t] = &found[..] else {
            panic!("{} texts", found.len())
        };
        assert_eq!(t.style, "ROTULO");
        assert_eq!(t.height, 2.5);
        assert!((t.width_factor - 0.8).abs() < 1e-12);
        assert!((t.oblique_angle - 0.25).abs() < 1e-12);
    }
}
