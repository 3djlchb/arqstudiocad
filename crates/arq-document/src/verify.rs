//! What survives writing a drawing and reading it again (ADR 0017 D4's
//! `validate_roundtrip`, D5.2). Saving checks the file it wrote with it
//! before replacing anything, and the round-trip corpus uses the same rule.

use std::collections::{BTreeMap, BTreeSet};

use arq_geometry::Tolerance;

use crate::document::Document;
use crate::entity::{Color, Entity, PlanGeometry};
use crate::layers::Layer;

/// Differences listed at most; the rest are counted.
const MAX_DIFFERENCES: usize = 10;

impl Document {
    /// Where `written`, this drawing as read back from a file, differs from
    /// it: an empty list when nothing the platform shows was lost.
    ///
    /// Allowed, as no loss: geometry within the contract's tolerance (DXF
    /// stores angles in degrees); a linetype name in other capitals (names
    /// ignore case: an R12 drawing's `CONTINUOUS` is `Continuous` in DWG);
    /// before AutoCAD 2004 (AC1018), which has
    /// no true color, RGB as an ACI index; before AutoCAD 2000 (AC1015),
    /// which cannot mark a layer not plotted, every layer plotted. The
    /// older of the two drawings' versions decides.
    pub fn validate_roundtrip(&self, written: &Document) -> Vec<String> {
        let version = self.version_code().min(written.version_code());
        let rules = Rules {
            true_color: version.as_str() >= "AC1018",
            plot_flag: version.as_str() >= "AC1015",
        };
        let mut differences = Vec::new();
        let (current, back) = (self.current_layer().id(), written.current_layer().id());
        if current != back {
            differences.push(format!("current layer {current} became {back}"));
        }
        compare(
            &sorted(self.entities()),
            &sorted(written.entities()),
            "entities",
            Entity::id,
            |a, b| rules.same_entity(a, b),
            &mut differences,
        );
        compare(
            &self.layers(),
            &written.layers(),
            "layers",
            Layer::id,
            |a, b| rules.same_layer(a, b),
            &mut differences,
        );
        if differences.len() > MAX_DIFFERENCES {
            let more = differences.len() - MAX_DIFFERENCES;
            differences.truncate(MAX_DIFFERENCES);
            differences.push(format!("and {more} more"));
        }
        differences
    }
}

fn sorted(mut entities: Vec<Entity>) -> Vec<Entity> {
    entities.sort_by_key(Entity::id);
    entities
}

/// Pairs `expected` and `found` by id and reports what is missing, added
/// or changed.
fn compare<T: std::fmt::Debug>(
    expected: &[T],
    found: &[T],
    what: &str,
    id: impl Fn(&T) -> u64,
    same: impl Fn(&T, &T) -> bool,
    differences: &mut Vec<String>,
) {
    if expected.len() != found.len() {
        differences.push(format!("{} {what} became {}", expected.len(), found.len()));
    }
    let by_id: BTreeMap<u64, &T> = found.iter().map(|b| (id(b), b)).collect();
    for a in expected {
        match by_id.get(&id(a)) {
            None => differences.push(format!("lost: {a:?}")),
            Some(b) if !same(a, b) => differences.push(format!("changed: was {a:?}, now {b:?}")),
            Some(_) => {}
        }
    }
    let ids: BTreeSet<u64> = expected.iter().map(&id).collect();
    for b in found {
        if !ids.contains(&id(b)) {
            differences.push(format!("added: {b:?}"));
        }
    }
}

struct Rules {
    true_color: bool,
    plot_flag: bool,
}

impl Rules {
    fn color(&self, a: Color, b: Color) -> bool {
        a == b || (!self.true_color && matches!((a, b), (Color::Rgb { .. }, Color::Index(1..))))
    }

    fn same_entity(&self, a: &Entity, b: &Entity) -> bool {
        a.kind() == b.kind()
            && a.layer() == b.layer()
            && self.color(a.color(), b.color())
            && option_eq(a.bounds(), b.bounds(), |x, y| x.approx_eq(y, tol()))
            && option_eq(a.plan(), b.plan(), same_plan)
    }

    fn same_layer(&self, a: &Layer, b: &Layer) -> bool {
        a.name() == b.name()
            && self.color(a.color(), b.color())
            && a.linetype().eq_ignore_ascii_case(b.linetype())
            && a.is_visible() == b.is_visible()
            && a.is_frozen() == b.is_frozen()
            && a.is_locked() == b.is_locked()
            && (a.is_plottable() == b.is_plottable() || (!self.plot_flag && b.is_plottable()))
    }
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

/// Fault injection for tests: the next save on this thread has its bytes
/// changed before they are written.
#[cfg(any(test, feature = "test-support"))]
pub(crate) mod fault {
    use std::cell::Cell;

    /// What a test does to the bytes about to be written.
    type Damage = fn(&mut Vec<u8>);

    std::thread_local! {
        static NEXT: Cell<Option<Damage>> = const { Cell::new(None) };
    }

    pub(crate) fn set(damage: Damage) {
        NEXT.set(Some(damage));
    }

    pub(crate) fn apply(bytes: &mut Vec<u8>) {
        if let Some(damage) = NEXT.take() {
            damage(bytes);
        }
    }
}

/// Test helpers for other crates (feature `test-support`).
#[cfg(feature = "test-support")]
impl Document {
    /// The next save on this thread has its bytes changed by `damage`
    /// before they are written, as a faulty disk or engine would.
    pub fn damage_next_save_for_tests(damage: fn(&mut Vec<u8>)) {
        fault::set(damage);
    }
}

#[cfg(test)]
#[path = "verify_tests.rs"]
mod tests;
