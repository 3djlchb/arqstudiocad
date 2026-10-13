//! What one library makes of one drawing, in terms every library can give.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The outcome of reading a drawing.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Summary {
    /// File-format version as the library names it (`AC1032`, `R_2018`…).
    pub version: String,
    /// Entities of every space and block.
    pub entities: usize,
    /// Entities in model space.
    pub model: usize,
    /// Entities by DXF type name.
    pub types: BTreeMap<String, usize>,
    /// Non-graphical objects by DXF class name (dynamic-block parameters,
    /// actions and grips, evaluation graphs, associative and constraint
    /// objects, dictionaries…).
    pub objects: BTreeMap<String, usize>,
    /// Layer table records.
    pub layers: usize,
    /// Block records, model and paper space included.
    pub blocks: usize,
    /// Fingerprints of geometry, to compare coordinates across libraries:
    /// `[count, sum]` of LINE end coordinates, of CIRCLE centers and radii,
    /// and of INSERT insertion points.
    pub lines: Fingerprint,
    pub circles: Fingerprint,
    pub inserts: Fingerprint,
    /// 3D solids, regions, bodies and surfaces: `count` of them and `sum`
    /// of the bytes of their ACIS geometry (SAT text or SAB binary).
    #[serde(default)]
    pub solids: Fingerprint,
    /// How many of those carry ACIS geometry at all.
    #[serde(default)]
    pub solids_with_data: usize,
    /// Entities carrying extended data (XDATA): where applications such as
    /// BIM tools keep their own properties.
    #[serde(default)]
    pub xdata_entities: usize,
    /// Read only by recovering: the strict read failed.
    pub recovered: bool,
    /// Diagnostics the library reported (skipped or damaged records).
    pub diagnostics: usize,
}

/// A count and a sum of coordinates.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Fingerprint {
    pub count: usize,
    pub sum: f64,
}

impl Fingerprint {
    pub fn add(&mut self, values: &[f64]) {
        self.count += 1;
        self.sum += values.iter().filter(|v| v.is_finite()).sum::<f64>();
    }
}

/// One job's result, printed by the child process as one JSON line.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct JobResult {
    pub library: String,
    pub operation: String,
    pub input: String,
    pub output: Option<String>,
    pub ok: bool,
    pub error: Option<String>,
    pub summary: Option<Summary>,
    /// Wall time of the operation, milliseconds.
    pub millis: f64,
    /// Peak resident memory of the process, KiB (Linux `VmHWM`).
    pub peak_kib: Option<u64>,
}
