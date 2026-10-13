//! AcadRust and opencadcodec, measured with the same code: the module is
//! written once and instantiated for each crate, whose APIs are the same
//! (opencadcodec is a fork of AcadRust).

macro_rules! codec_library {
    ($module:ident, $krate:ident) => {
        pub mod $module {
            use std::path::Path;

            use $krate::types::DxfVersion;
            use $krate::{
                CadDocument, DwgReadOptions, DwgReader, DwgWriter, DxfReader,
                DxfReaderConfiguration, DxfWriter, EntityType,
            };

            use crate::summary::Summary;

            /// An object's DXF class name, upper case as in the file.
            fn object_name(o: &$krate::objects::ObjectType) -> String {
                use $krate::objects::ObjectType as O;
                match o {
                    O::DynamicBlock(d) => d.dxf_name.to_uppercase(),
                    O::Associative(a) => a.dxf_name.to_uppercase(),
                    O::BlockVisibilityParameter(_) => "BLOCKVISIBILITYPARAMETER".to_owned(),
                    O::Unknown { type_name, .. } => type_name.to_uppercase(),
                    O::ProxyObject(_) => "ACAD_PROXY_OBJECT".to_owned(),
                    other => {
                        let debug = format!("{other:?}");
                        let end = debug.find(['(', ' ', '{']).unwrap_or(debug.len());
                        debug[..end].to_uppercase()
                    }
                }
            }

            fn is_dwg(path: &Path) -> bool {
                path.extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("dwg"))
            }

            fn read_with(path: &Path, failsafe: bool) -> Result<CadDocument, String> {
                if is_dwg(path) {
                    let options = if failsafe {
                        DwgReadOptions::failsafe()
                    } else {
                        DwgReadOptions::default()
                    };
                    DwgReader::from_file_with_options(path, options)
                        .and_then(|mut r| r.read())
                        .map_err(|e| e.to_string())
                } else {
                    let config = DxfReaderConfiguration {
                        failsafe,
                        ..DxfReaderConfiguration::default()
                    };
                    DxfReader::from_file(path)
                        .map(|r| r.with_configuration(config))
                        .and_then(|r| r.read())
                        .map_err(|e| e.to_string())
                }
            }

            /// Strict read; if it fails, a failsafe (recovering) read.
            pub fn read(path: &Path) -> Result<(CadDocument, bool), String> {
                match read_with(path, false) {
                    Ok(doc) => Ok((doc, false)),
                    Err(strict) => read_with(path, true)
                        .map(|doc| (doc, true))
                        .map_err(|recovering| format!("{strict} (recovering: {recovering})")),
                }
            }

            pub fn summarize(doc: &CadDocument, recovered: bool) -> Summary {
                let mut s = Summary {
                    version: format!("{:?}", doc.version),
                    model: doc.model_space_entities().count(),
                    layers: doc.layers.len(),
                    blocks: doc.block_records.len(),
                    recovered,
                    diagnostics: doc.notifications.len(),
                    ..Summary::default()
                };
                for o in doc.objects.values() {
                    *s.objects.entry(object_name(o)).or_default() += 1;
                }
                for e in doc.entities() {
                    s.entities += 1;
                    if !e.common().extended_data.is_empty() {
                        s.xdata_entities += 1;
                    }
                    let acis = match e {
                        EntityType::Solid3D(x) => Some(&x.acis_data),
                        EntityType::Region(x) => Some(&x.acis_data),
                        EntityType::Body(x) => Some(&x.acis_data),
                        EntityType::Surface(x) => Some(&x.acis_data),
                        _ => None,
                    };
                    if let Some(a) = acis {
                        let bytes = if a.is_binary {
                            a.sab_data.len()
                        } else {
                            a.sat_data.len()
                        };
                        s.solids.count += 1;
                        s.solids.sum += bytes as f64;
                        if bytes > 0 {
                            s.solids_with_data += 1;
                        }
                    }
                    *s.types
                        .entry(e.as_entity().entity_type().to_owned())
                        .or_default() += 1;
                    match e {
                        EntityType::Line(l) => {
                            s.lines.add(&[l.start.x, l.start.y, l.end.x, l.end.y])
                        }
                        EntityType::Circle(c) => s.circles.add(&[c.center.x, c.center.y, c.radius]),
                        EntityType::Insert(i) => {
                            s.inserts.add(&[i.insert_point.x, i.insert_point.y])
                        }
                        _ => {}
                    }
                }
                s
            }

            /// Writes DWG as AutoCAD 2018 (AC1032), the target the product
            /// saves; DXF in the same version.
            pub fn write(doc: &CadDocument, out: &Path) -> Result<(), String> {
                let mut modern = doc.clone();
                // DWGBENCH_KEEP_VERSION=1 saves in the drawing's own version
                // (R12 and unknown still go to 2018, which the codec writes).
                let keep = std::env::var_os("DWGBENCH_KEEP_VERSION").is_some()
                    && !matches!(doc.version, DxfVersion::AC1009 | DxfVersion::Unknown);
                if !keep {
                    modern.version = DxfVersion::AC1032;
                }
                if is_dwg(out) {
                    DwgWriter::write_to_file(out, &modern).map_err(|e| e.to_string())
                } else {
                    DxfWriter::new(&modern)
                        .write_to_file(out)
                        .map_err(|e| e.to_string())
                }
            }
        }
    };
}

codec_library!(acadrust_lib, acadrust);
codec_library!(opencadcodec_lib, opencadcodec);
