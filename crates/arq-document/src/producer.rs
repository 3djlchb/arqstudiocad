//! The program a drawing comes from (ADR 0045 D1), and how ours names
//! itself in the drawings it saves.
//!
//! A DWG of 2004 or later names the program that wrote it in its AppInfo
//! section; the codec reads it into `CadDocument::app_info`. Its product
//! information names AutoCAD and the ODA's Teigha as the public samples
//! show; the other producers are known by the names they give themselves,
//! to confirm against their own drawings (ADR 0045 D4). A DXF says nothing
//! the samples have shown to tell producers apart, so it is unknown.

use std::sync::RwLock;

use acadrust::AppInfo;

/// The program a drawing was last saved by.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Producer {
    /// Autodesk's AutoCAD, or a program it licenses.
    AutoCad,
    /// ProgeCAD or another IntelliCAD.
    IntelliCad,
    /// Bricsys's BricsCAD.
    BricsCad,
    /// Graebert's ARES Commander.
    AresCommander,
    /// Gstarsoft's GstarCAD.
    GstarCad,
    /// ACCA's Edificius.
    Edificius,
    /// Nemetschek's Vectorworks.
    Vectorworks,
    /// The Open Design Alliance's libraries, naming no program of their own.
    Oda,
    /// This program.
    Ours,
    /// Another program, as it names itself.
    Other(String),
    /// Nothing in the file says which.
    Unknown,
}

impl Producer {
    /// A short stable key, for the Python API and the forms table.
    pub fn key(&self) -> &str {
        match self {
            Self::AutoCad => "autocad",
            Self::IntelliCad => "intellicad",
            Self::BricsCad => "bricscad",
            Self::AresCommander => "ares",
            Self::GstarCad => "gstarcad",
            Self::Edificius => "edificius",
            Self::Vectorworks => "vectorworks",
            Self::Oda => "oda",
            Self::Ours => "ours",
            Self::Other(_) => "other",
            Self::Unknown => "unknown",
        }
    }

    /// The producer a file's AppInfo names; unknown without one.
    pub fn of(info: Option<&AppInfo>) -> Self {
        let Some(info) = info else {
            return Self::Unknown;
        };
        let product = info.product_attribute("name").unwrap_or_default();
        let ours = writer().map(|w| w.product_attribute("name").unwrap_or_default());
        if ours.is_some_and(|name| !name.is_empty() && name == product) {
            return Self::Ours;
        }
        // Everything the section says, for the names a program may give.
        let said = format!("{product} {} {}", info.version, info.comment).to_lowercase();
        let has = |words: &[&str]| words.iter().any(|w| said.contains(w));
        if has(&["autocad", "autodesk"]) {
            Self::AutoCad
        } else if has(&["progecad", "intellicad"]) {
            Self::IntelliCad
        } else if has(&["bricscad", "bricsys"]) {
            Self::BricsCad
        } else if has(&["ares commander", "graebert"])
            || product
                .split_whitespace()
                .next()
                .is_some_and(|w| w.eq_ignore_ascii_case("ares"))
        {
            Self::AresCommander
        } else if has(&["gstarcad", "gstarsoft"]) {
            Self::GstarCad
        } else if has(&["edificius"]) {
            Self::Edificius
        } else if has(&["vectorworks"]) {
            Self::Vectorworks
        } else if has(&["teigha", "open design alliance"]) {
            Self::Oda
        } else if !product.is_empty() {
            Self::Other(product)
        } else {
            Self::Unknown
        }
    }
}

/// How ours names itself in the DWG it saves, once the application has
/// said (`set_writer`); until then the codec's own.
static WRITER: RwLock<Option<AppInfo>> = RwLock::new(None);

/// Names this program, `name` at `version`, in every DWG saved from now on:
/// its AppInfo section says so, so a drawing saved here is known as ours
/// when read again. The name comes from the application (its `product`
/// module), never spelled out here.
pub fn set_writer(name: &str, version: &str) {
    let info = AppInfo {
        name: "AppInfoDataList".to_owned(),
        version: version.to_owned(),
        comment: format!("Saved by {name} {version}"),
        product: format!(
            "<ProductInformation name=\"{name}\" build_version=\"{version}\" \
             registry_version=\"{version}\" install_id_string=\"{name}\" \
             registry_localeID=\"1033\"/>"
        ),
    };
    *WRITER.write().unwrap_or_else(|e| e.into_inner()) = Some(info);
}

/// The AppInfo ours writes, when the application has named it.
pub(crate) fn writer() -> Option<AppInfo> {
    WRITER.read().unwrap_or_else(|e| e.into_inner()).clone()
}

#[cfg(test)]
#[path = "producer_tests.rs"]
mod tests;
