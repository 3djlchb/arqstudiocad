//! Round-trip corpus (ADR 0011, D3.12): open → save → reopen, then edit →
//! save → reopen → undo, as DWG and as DXF, checking that everything the API
//! shows survives.
//!
//! Three sources:
//!
//! 1. drawings generated here, in every version the codec writes; runs with
//!    every `cargo test`;
//! 2. the codec's sample files, fetched with
//!    `uv run python tools/fetch_codec_samples.py`;
//!    `cargo test -p arq-core --test corpus -- --ignored codec`;
//! 3. real drawings named by `ARQ_CORPUS_DIR` (a folder, searched
//!    recursively, or one file), never committed; `cargo test -p arq-core --test corpus -- --ignored local`.
//!
//! The last two fail when their folder is missing or empty: a check that
//! inspects nothing is a failure (contract §12).

use std::fmt::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use arq_core::{
    CadDocument, Color, Document, Entity, EntityId, Layer, LayerId, PlanGeometry, Shape,
};
use arq_geometry::{Point2, Tolerance, Vector2};

/// DXF versions the codec writes; DWG starts at R13 (AC1012).
const VERSIONS: [&str; 9] = [
    "AC1009", "AC1012", "AC1014", "AC1015", "AC1018", "AC1021", "AC1024", "AC1027", "AC1032",
];

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).expect("finite")
}

/// Everything the API shows of a drawing, entities sorted by id.
#[derive(Debug, PartialEq)]
struct Observed {
    entities: Vec<Entity>,
    layers: Vec<Layer>,
    current: LayerId,
}

fn observe(doc: &Document) -> Observed {
    let mut entities = doc.entities();
    entities.sort_by_key(Entity::id);
    Observed {
        entities,
        layers: doc.layers(),
        current: doc.current_layer().id(),
    }
}

/// How a saved and reopened drawing may differ from the one saved without
/// losing anything: geometry within the contract's tolerance (DXF stores
/// angles in degrees, so radians come back within an ulp or two); before
/// AutoCAD 2004 (AC1018), which has no true color, RGB as an ACI index; and
/// before AutoCAD 2000 (AC1015), which cannot mark a layer not plottable,
/// every layer plottable.
fn difference(expected: &Observed, found: &Observed, version: &str) -> Option<String> {
    let true_color = version >= "AC1018";
    let plot_flag = version >= "AC1015";
    let color_ok = |a: Color, b: Color| {
        a == b || (!true_color && matches!((a, b), (Color::Rgb { .. }, Color::Index(1..))))
    };
    if expected.current != found.current {
        return Some(format!(
            "current layer {} became {}",
            expected.current, found.current
        ));
    }
    if expected.entities.len() != found.entities.len() {
        return Some(format!(
            "{} entities became {}",
            expected.entities.len(),
            found.entities.len()
        ));
    }
    for (a, b) in expected.entities.iter().zip(&found.entities) {
        let same = a.id() == b.id()
            && a.kind() == b.kind()
            && a.layer() == b.layer()
            && color_ok(a.color(), b.color())
            && option_eq(a.bounds(), b.bounds(), |x, y| x.approx_eq(y, tol()))
            && option_eq(a.plan(), b.plan(), same_plan);
        if !same {
            return Some(format!(
                "entity changed:
      was {a:?}
      now {b:?}"
            ));
        }
    }
    if expected.layers.len() != found.layers.len() {
        return Some(format!(
            "{} layers became {}",
            expected.layers.len(),
            found.layers.len()
        ));
    }
    for (a, b) in expected.layers.iter().zip(&found.layers) {
        let same = a.id() == b.id()
            && a.name() == b.name()
            && color_ok(a.color(), b.color())
            && a.linetype() == b.linetype()
            && a.is_visible() == b.is_visible()
            && a.is_frozen() == b.is_frozen()
            && a.is_locked() == b.is_locked()
            && (a.is_plottable() == b.is_plottable() || (!plot_flag && b.is_plottable()));
        if !same {
            return Some(format!(
                "layer changed:
      was {a:?}
      now {b:?}"
            ));
        }
    }
    None
}

fn tol() -> Tolerance {
    Tolerance::default()
}

fn option_eq<T: Copy>(a: Option<T>, b: Option<T>, eq: impl Fn(T, T) -> bool) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// Same curve: kind, ends, box and length within tolerance. For a circle the
/// box fixes center and radius; for an arc ends, box and length fix it.
fn same_plan(a: PlanGeometry, b: PlanGeometry) -> bool {
    let (ga, gb) = (a.geometry(), b.geometry());
    let near = |x: f64, y: f64| (x - y).abs() <= tol().linear() * (1.0 + x.abs());
    ga.kind() == gb.kind()
        && near(a.elevation(), b.elevation())
        && option_eq(ga.start_point(), gb.start_point(), |x, y| {
            x.approx_eq(y, tol())
        })
        && option_eq(ga.end_point(), gb.end_point(), |x, y| x.approx_eq(y, tol()))
        && ga.bounds().approx_eq(gb.bounds(), tol())
        && near(ga.length(), gb.length())
}

/// Saves a copy of `cad` as `<stem>.<ext>` and reads it back.
fn save_and_reopen(
    cad: &CadDocument,
    dir: &Path,
    stem: &str,
    ext: &str,
) -> Result<Observed, String> {
    let path = dir.join(format!("{stem}.{ext}"));
    cad.snapshot()
        .save(&path)
        .map_err(|e| format!("save as {ext}: {e}"))?;
    let back = CadDocument::open(&path).map_err(|e| format!("reopen {ext}: {e}"))?;
    if let Some(recovery) = back.document().recovery() {
        return Err(format!(
            "reopen {ext}: read as damaged: {:?}",
            recovery.problems()
        ));
    }
    model_in_step(back.document(), &format!("reopened {ext}"))?;
    Ok(observe(back.document()))
}

/// The own model and the engine describe the same drawing (ADR 0018 D2).
fn model_in_step(doc: &Document, when: &str) -> Result<(), String> {
    let problems = doc.check_model();
    if problems.is_empty() {
        return Ok(());
    }
    Err(format!("own model out of step {when}: {problems:?}"))
}

/// Every drawing is saved in both formats; an R12 one becomes 2018 as DWG.
fn formats(_version: &str) -> &'static [&'static str] {
    &["dwg", "dxf"]
}

/// The edits applied to every drawing: move what can be moved, erase one
/// entity, draw a line on a new layer with its own color.
fn edit(cad: &mut CadDocument) -> Result<(), String> {
    let doc = cad.document();
    let locked = |e: &Entity| doc.layer_by_name(e.layer()).is_some_and(|l| l.is_locked());
    let entities: Vec<Entity> = doc.entities().into_iter().filter(|e| !locked(e)).collect();
    // Blocks and polylines move as a whole too (ADR 0016, D1).
    let movable: Vec<EntityId> = entities
        .iter()
        .filter(|e| e.is_transformable())
        .map(Entity::id)
        .collect();
    // Prefer erasing a read-only entity: erasing must work on any type.
    let erase = entities
        .iter()
        .find(|e| !e.is_editable())
        .or_else(|| entities.first())
        .map(Entity::id);
    let mut layer_name = "ARQ-CORPUS".to_owned();
    while doc.layer_by_name(&layer_name).is_some() {
        layer_name.push('X');
    }

    let step =
        |r: arq_core::CadResult<_>, what: &str| r.map(|_| ()).map_err(|e| format!("{what}: {e}"));
    cad.begin_group("CORPUS");
    let movable_rest: Vec<EntityId> = movable
        .into_iter()
        .filter(|&id| Some(id) != erase)
        .collect();
    step(
        cad.move_entities(&movable_rest, Vector2::new(1.5, -2.25).expect("finite")),
        "move",
    )?;
    if let Some(id) = erase {
        step(cad.delete_entities(&[id]), "erase")?;
    }
    let (layer, _) = cad
        .create_layer(&layer_name)
        .map_err(|e| format!("create layer: {e}"))?;
    let (line, _) = cad
        .add_line(p(-3.0, 7.0), p(12.5, 0.125))
        .map_err(|e| format!("draw: {e}"))?;
    step(cad.set_entity_layer(&[line], layer), "change layer")?;
    step(
        cad.set_entity_color(&[line], Color::Index(3)),
        "change color",
    )?;
    cad.end_group().map_err(|e| format!("end group: {e}"))?;
    Ok(())
}

/// The whole check for one drawing; returns the problems found.
fn round_trip(mut cad: CadDocument, dir: &Path) -> Vec<String> {
    let mut problems = Vec::new();
    let version = cad.document().version_code();
    let mut check_files = |cad: &CadDocument, stem: &str, expected: &Observed| {
        for ext in formats(&version) {
            match save_and_reopen(cad, dir, stem, ext) {
                Ok(found) => {
                    if let Some(d) = difference(expected, &found, &version) {
                        problems.push(format!("{ext} {stem}: {d}"));
                    }
                }
                Err(e) => problems.push(e),
            }
        }
    };
    let mut model_problems = Vec::new();
    if let Err(e) = model_in_step(cad.document(), "after opening") {
        model_problems.push(e);
    }
    let before = observe(cad.document());
    check_files(&cad, "as-opened", &before);
    if let Err(e) = edit(&mut cad) {
        problems.push(format!("edit: {e}"));
        return problems;
    }
    if let Err(e) = model_in_step(cad.document(), "after editing") {
        model_problems.push(e);
    }
    let edited = observe(cad.document());
    check_files(&cad, "edited", &edited);
    // In memory nothing is converted: undo must restore exactly.
    match cad.undo() {
        Err(e) => problems.push(format!("undo: {e}")),
        Ok(_) => {
            let after = observe(cad.document());
            if after != before {
                let d = difference(&before, &after, "AC1032")
                    .unwrap_or_else(|| "differs within tolerance".to_owned());
                problems.push(format!("undo: {d}"));
            }
        }
    }
    problems.extend(model_problems);
    problems
}

/// Runs `check` on each item, catching panics, and fails listing every
/// problem; prints one line per item.
fn run_all<T>(items: Vec<(String, T)>, check: impl Fn(T, &Path) -> Result<String, Vec<String>>) {
    assert!(
        !items.is_empty(),
        "the corpus is empty: nothing was checked"
    );
    let mut report = String::new();
    let mut failed = 0;
    for (name, item) in items {
        let dir = tempfile::tempdir().expect("temp dir");
        let outcome = catch_unwind(AssertUnwindSafe(|| check(item, dir.path())))
            .unwrap_or_else(|panic| Err(vec![format!("panic: {}", panic_message(&panic))]));
        match outcome {
            Ok(summary) => println!("ok    {name}  {summary}"),
            Err(problems) => {
                failed += 1;
                println!("FAIL  {name}");
                for p in &problems {
                    println!("      {p}");
                    let _ = writeln!(report, "{name}: {p}");
                }
            }
        }
    }
    assert!(failed == 0, "{failed} drawing(s) failed:\n{report}");
}

fn panic_message(panic: &Box<dyn std::any::Any + Send>) -> String {
    panic
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown".to_owned())
}

fn check_file(path: PathBuf, dir: &Path) -> Result<String, Vec<String>> {
    let cad = CadDocument::open(&path).map_err(|e| vec![format!("open: {e}")])?;
    let doc = cad.document();
    // The corpus holds intact drawings: none may read as damaged (ADR 0017 D5.1).
    if let Some(recovery) = doc.recovery() {
        return Err(vec![format!("read as damaged: {:?}", recovery.problems())]);
    }
    let model = doc.entities();
    let editable = model.iter().filter(|e| e.is_editable()).count();
    let transformable = model.iter().filter(|e| e.is_transformable()).count();
    let mut kinds: Vec<(&str, usize)> = Vec::new();
    for e in &model {
        match kinds.iter_mut().find(|(k, _)| *k == e.kind()) {
            Some((_, n)) => *n += 1,
            None => kinds.push((e.kind(), 1)),
        }
    }
    kinds.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    let kinds: Vec<String> = kinds.iter().map(|(k, n)| format!("{k} {n}")).collect();
    let (drawn, drawing_problems) = drawing_coverage(doc);
    let summary = format!(
        "{}  {} entities, {} in model space, {editable} editable, {transformable} transformable: {}
      {drawn}",
        doc.version_code(),
        doc.entity_count(),
        model.len(),
        kinds.join(", ")
    );
    let mut problems = drawing_problems;
    problems.extend(whole_transform_problems(&path));
    problems.extend(round_trip(cad, dir));
    if problems.is_empty() {
        Ok(summary)
    } else {
        Err(problems)
    }
}

/// Points along everything drawn for `id`, as the drawing primitives give
/// them: ends and middles of lines and arcs, and circles' centers (a
/// rotated circle is the same circle, sampled from a new east point; the
/// rotation and move checked here keep radii).
fn drawn_points(doc: &Document, id: EntityId) -> Vec<Point2> {
    let mut points = Vec::new();
    for item in doc.drawing(id).unwrap_or_default() {
        let g: arq_geometry::Geometry2 = match item.shape {
            Shape::Line(l) => l.into(),
            Shape::Arc(a) => a.into(),
            Shape::Circle(c) => {
                points.push(c.center());
                continue;
            }
            _ => continue,
        };
        points.extend([0.0, 0.25, 0.5, 0.75, 1.0].map(|t| g.point_at(t)));
    }
    points
}

/// Rotates and moves every block and polyline of the drawing (ADR 0016,
/// D1) and checks each is drawn where the transformation puts it.
fn whole_transform_problems(path: &Path) -> Vec<String> {
    let Ok(mut cad) = CadDocument::open(path) else {
        return Vec::new(); // reported by the round trip
    };
    let Some(center) = cad.bounds().map(|b| b.center()) else {
        return Vec::new();
    };
    let doc = cad.document();
    let locked = |e: &Entity| doc.layer_by_name(e.layer()).is_some_and(|l| l.is_locked());
    let ids: Vec<EntityId> = doc
        .entities()
        .iter()
        .filter(|e| e.is_transformable() && !e.is_editable() && !locked(e))
        .map(Entity::id)
        .collect();
    let t = arq_geometry::Transform2::rotation(center, 0.4)
        .expect("finite")
        .then(arq_geometry::Transform2::translation(
            Vector2::new(3.25, -1.5).expect("finite"),
        ));
    let expected: Vec<Vec<Point2>> = ids
        .iter()
        .map(|id| {
            drawn_points(doc, *id)
                .into_iter()
                .map(|q| t.apply_point(q))
                .collect()
        })
        .collect();
    if let Err(e) = cad
        .rotate_entities(&ids, center, 0.4)
        .and_then(|_| cad.move_entities(&ids, Vector2::new(3.25, -1.5).expect("finite")))
    {
        return vec![format!("transform blocks and polylines: {e}")];
    }
    let size = cad.bounds().map_or(1.0, |b| b.width().max(b.height()));
    let tol = Tolerance::new(1e-9 * (1.0 + size), 1e-9).expect("valid");
    let mut problems = Vec::new();
    for (id, want) in ids.iter().zip(expected) {
        let found = drawn_points(cad.document(), *id);
        let same = found.len() == want.len()
            && want
                .iter()
                .all(|w| found.iter().any(|f| f.approx_eq(*w, tol)));
        if !same {
            let kind = cad.document().entity(*id).map_or("?", |e| e.kind());
            problems.push(format!(
                "entity {id} ({kind}) is not drawn where it was moved"
            ));
        }
    }
    problems.truncate(20);
    problems
}

/// What the drawing primitives make of a drawing (ADR 0013, D5.2): a line
/// for the report, and a problem for every visible entity that draws nothing
/// although it has an extent.
fn drawing_coverage(doc: &Document) -> (String, Vec<String>) {
    let mut shapes = [0usize; 6]; // line, circle, arc, point, text, placeholder
    let mut placeholder_kinds: Vec<&str> = Vec::new();
    let mut problems = Vec::new();
    for (id, items) in doc.drawing_all() {
        let entity = doc.entity(id).expect("drawn entities exist");
        let visible = doc
            .layer_by_name(entity.layer())
            .is_none_or(|l| l.is_visible() && !l.is_frozen());
        for item in &items {
            let slot = match item.shape {
                Shape::Line(_) => 0,
                Shape::Circle(_) => 1,
                Shape::Arc(_) => 2,
                Shape::Point(_) => 3,
                Shape::Text(_) => 4,
                Shape::Placeholder(_) => {
                    if !placeholder_kinds.contains(&entity.kind()) {
                        placeholder_kinds.push(entity.kind());
                    }
                    5
                }
            };
            shapes[slot] += 1;
            if matches!(item.color, Color::ByLayer | Color::ByBlock | Color::None) {
                problems.push(format!("entity {id} ({}): unresolved color", entity.kind()));
            }
        }
        let nothing_to_draw = matches!(entity.kind(), "VIEWPORT" | "XLINE" | "RAY");
        if items.is_empty() && visible && !nothing_to_draw && entity.bounds().is_some() {
            problems.push(format!("entity {id} ({}) draws nothing", entity.kind()));
        }
    }
    let [l, c, a, pt, t, ph] = shapes;
    let mut line = format!(
        "draws {} shapes: {l} lines, {c} circles, {a} arcs, {pt} points, {t} texts, {ph} placeholders",
        l + c + a + pt + t + ph
    );
    if !placeholder_kinds.is_empty() {
        let _ = write!(line, " ({})", placeholder_kinds.join(", "));
    }
    problems.truncate(20); // enough to see the pattern
    (line, problems)
}

/// Every `.dwg` and `.dxf` under `root`, sorted, with paths relative to it.
fn drawings_in(root: &Path) -> Vec<(String, PathBuf)> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("dwg") || e.eq_ignore_ascii_case("dxf"))
            {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    walk(root, &mut files);
    files.sort();
    files
        .into_iter()
        .map(|f| {
            let name = f.strip_prefix(root).unwrap_or(&f).display().to_string();
            (name, f)
        })
        .collect()
}

/// A drawing exercising what the API edits, plus a read-only TEXT.
fn generated(version: &str) -> CadDocument {
    let mut doc = Document::new();
    assert!(doc.set_version_for_tests(version), "{version}");
    doc.insert_text_for_tests("NOTA", 2.0, 3.0);
    let mut cad = CadDocument::from_document(doc);
    let (walls, _) = cad.create_layer("A-MUROS").expect("layer");
    let (doors, _) = cad.create_layer("A-PUERTAS").expect("layer");
    cad.set_layer_color(
        walls,
        Color::Rgb {
            r: 200,
            g: 120,
            b: 40,
        },
    )
    .expect("color");
    cad.set_layer_color(doors, Color::Index(5)).expect("color");
    cad.set_layer_plottable(doors, false).expect("plot");
    cad.set_current_layer(walls).expect("current");
    let (a, _) = cad.add_line(p(0.0, 0.0), p(10.0, 0.0)).expect("line");
    // Survey coordinates keep their precision.
    let (b, _) = cad
        .add_line(p(500_000.125, 9_977_000.5), p(500_010.0, 9_977_005.25))
        .expect("line");
    let (c, _) = cad.add_circle(p(5.0, 5.0), 2.5).expect("circle");
    let (d, _) = cad.add_arc(p(-4.0, 1.0), 3.0, 0.25, 4.5).expect("arc");
    cad.set_entity_layer(&[c, d], doors).expect("layer");
    cad.set_entity_color(&[a], Color::Index(1)).expect("color");
    cad.set_entity_color(&[b], Color::Rgb { r: 1, g: 2, b: 3 })
        .expect("color");
    cad.set_layer_locked(doors, true).expect("lock");
    cad
}

#[test]
fn generated_drawings_round_trip_in_every_version() {
    let items = VERSIONS
        .iter()
        .map(|v| (format!("generated {v}"), *v))
        .collect();
    run_all(items, |version, dir| {
        let cad = generated(version);
        let found = cad.document().version_code();
        if found != version {
            return Err(vec![format!("asked for {version}, got {found}")]);
        }
        let problems = round_trip(cad, dir);
        if problems.is_empty() {
            Ok(String::new())
        } else {
            Err(problems)
        }
    });
}

#[test]
#[ignore = "needs the codec samples: uv run python tools/fetch_codec_samples.py"]
fn codec_samples_round_trip() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/corpus/acadrust");
    assert!(
        root.is_dir(),
        "{} is missing: run `uv run python tools/fetch_codec_samples.py`",
        root.display()
    );
    run_all(drawings_in(&root), check_file);
}

#[test]
#[ignore = "needs ARQ_CORPUS_DIR: real drawings (a folder or one file), never committed"]
fn local_drawings_round_trip() {
    let root = std::env::var_os("ARQ_CORPUS_DIR")
        .map(PathBuf::from)
        .expect("set ARQ_CORPUS_DIR to a .dwg/.dxf file or a folder of them");
    let items = if root.is_file() {
        let name = root.file_name().map_or_else(
            || root.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        vec![(name, root)]
    } else {
        assert!(
            root.is_dir(),
            "ARQ_CORPUS_DIR does not exist: {}",
            root.display()
        );
        drawings_in(&root)
    };
    run_all(items, check_file);
}
