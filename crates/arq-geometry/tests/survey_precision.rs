//! End-to-end precision at survey coordinates (ADR 0010, local frames).
//!
//! The same scene is built near the origin and translated to UTM zone 17S
//! coordinates (continental Ecuador). Every operation of the crate must give
//! the same answer in both places once the offset is removed, to within
//! 1e-8 model units (10 nanometres in metres). The worst case measured is
//! about 1.5e-9: the resolution of `f64` itself at ~1e7.

use std::f64::consts::FRAC_PI_3;

use arq_geometry::snap;
use arq_geometry::{
    Arc, Circle, Geometry2, Intersections, Line, Point2, Tolerance, Transform2, Vector2, intersect,
};

/// Agreement required between the local and the survey-coordinate result.
const AGREE: f64 = 1e-8;
const TOL: Tolerance = Tolerance::DEFAULT;

/// Quito-like and Guayaquil-like positions in UTM zone 17S (easting, northing).
const SITES: [(f64, f64); 2] = [(500_000.0, 9_977_000.0), (622_000.0, 9_760_000.0)];

/// A small architectural scene, offset by `o`.
struct Scene {
    o: Vector2,
}

impl Scene {
    fn p(&self, x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap() + self.o
    }
    fn wall(&self) -> Line {
        Line::new(self.p(-12.0, 3.5), self.p(15.0, 3.5)).unwrap()
    }
    fn beam(&self) -> Line {
        Line::new(self.p(2.0, -8.0), self.p(-1.0, 14.0)).unwrap()
    }
    fn column(&self) -> Circle {
        Circle::new(self.p(0.25, 0.75), 4.75).unwrap()
    }
    fn stair(&self) -> Circle {
        Circle::new(self.p(6.5, -1.25), 3.3).unwrap()
    }
    fn door_swing(&self) -> Arc {
        Arc::new(self.p(0.25, 0.75), 4.75, 0.3, 2.4).unwrap()
    }
    fn vault(&self) -> Arc {
        Arc::from_three_points(self.p(-3.0, 0.0), self.p(0.4, 3.1), self.p(3.7, 0.2), TOL).unwrap()
    }
    /// Removes the offset, so results can be compared with the local scene.
    fn local(&self, q: Point2) -> Point2 {
        q - self.o
    }
}

fn assert_same_point(local: Point2, survey: Point2, what: &str) {
    let d = local.distance_to(survey);
    assert!(
        d <= AGREE,
        "{what}: local {local:?} vs survey {survey:?} differ by {d:e}"
    );
}

fn assert_same_points(local: &[Point2], survey: &[Point2], what: &str) {
    assert_eq!(local.len(), survey.len(), "{what}: different counts");
    for l in local {
        let best = survey
            .iter()
            .map(|s| l.distance_to(*s))
            .fold(f64::INFINITY, f64::min);
        assert!(
            best <= AGREE,
            "{what}: {l:?} has no counterpart (closest {best:e})"
        );
    }
}

fn localize(scene: &Scene, i: &Intersections) -> Vec<Point2> {
    i.points().iter().map(|q| scene.local(*q)).collect()
}

#[test]
fn every_operation_agrees_at_survey_coordinates() {
    let here = Scene { o: Vector2::ZERO };
    for (e, n) in SITES {
        let there = Scene {
            o: Vector2::new(e, n).unwrap(),
        };
        let site = format!("site ({e}, {n})");

        // Arc through three points.
        assert_same_point(
            here.vault().center(),
            there.local(there.vault().center()),
            &format!("{site}: vault center"),
        );
        assert!((here.vault().radius() - there.vault().radius()).abs() <= AGREE);

        // Intersections of every kind.
        let pairs = |s: &Scene| -> [(Geometry2, Geometry2); 6] {
            [
                (s.wall().into(), s.beam().into()),
                (s.wall().into(), s.column().into()),
                (s.beam().into(), s.door_swing().into()),
                (s.column().into(), s.stair().into()),
                (s.door_swing().into(), s.vault().into()),
                (s.stair().into(), s.vault().into()),
            ]
        };
        let mut crossings = 0;
        for (k, ((a0, b0), (a1, b1))) in pairs(&here).into_iter().zip(pairs(&there)).enumerate() {
            let local = intersect(a0, b0, TOL);
            let survey = intersect(a1, b1, TOL);
            assert_same_points(
                local.points(),
                &localize(&there, &survey),
                &format!("{site}: pair {k}"),
            );
            assert_eq!(local.overlaps().len(), survey.overlaps().len());
            crossings += local.points().len() + local.overlaps().len();
        }
        // Empty results would make the comparison above meaningless: the scene
        // has exactly 7 crossings, and losing any of them must fail the test.
        assert_eq!(crossings, 7, "{site}: crossings found in the scene");

        // Transformations around a pivot inside the scene.
        let pivot = |s: &Scene| s.p(1.5, -2.0);
        let ops = |s: &Scene| -> [Transform2; 3] {
            [
                Transform2::rotation(pivot(s), FRAC_PI_3).unwrap(),
                Transform2::scaling(pivot(s), 1.75).unwrap(),
                Transform2::mirror(s.beam()),
            ]
        };
        for (t0, t1) in ops(&here).into_iter().zip(ops(&there)) {
            for (g0, g1) in [
                (Geometry2::from(here.wall()), Geometry2::from(there.wall())),
                (here.door_swing().into(), there.door_swing().into()),
            ] {
                let (i0, i1) = (t0.apply(g0).unwrap(), t1.apply(g1).unwrap());
                for s in [0.0, 0.37, 1.0] {
                    assert_same_point(
                        i0.point_at(s),
                        there.local(i1.point_at(s)),
                        &format!("{site}: transform"),
                    );
                }
                let back = t1.inverse().apply(i1).unwrap();
                assert_same_point(
                    g1.point_at(0.5),
                    back.point_at(0.5),
                    &format!("{site}: inverse"),
                );
            }
        }

        // Snaps and closest points from a cursor in the scene.
        let cursor = |s: &Scene| s.p(-7.25, 9.5);
        let tan = |s: &Scene| snap::tangent_points(s.column().into(), cursor(s), TOL);
        let per = |s: &Scene| snap::perpendicular_points(s.wall().into(), cursor(s), TOL);
        for (f, name) in [
            (&tan as &dyn Fn(&Scene) -> Vec<snap::SnapPoint>, "TAN"),
            (&per, "PER"),
        ] {
            let l: Vec<Point2> = f(&here).iter().map(|s| s.point).collect();
            let s: Vec<Point2> = f(&there).iter().map(|s| there.local(s.point)).collect();
            assert_same_points(&l, &s, &format!("{site}: {name}"));
        }
        assert_same_point(
            here.vault().closest_point(cursor(&here)),
            there.local(there.vault().closest_point(cursor(&there))),
            &format!("{site}: nearest"),
        );

        // Bounds of an arc crossing a quadrant.
        let (b0, b1) = (here.door_swing().bounds(), there.door_swing().bounds());
        assert_same_point(
            b0.min(),
            there.local(b1.min()),
            &format!("{site}: bounds min"),
        );
        assert_same_point(
            b0.max(),
            there.local(b1.max()),
            &format!("{site}: bounds max"),
        );
    }
}
