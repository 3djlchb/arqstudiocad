//! A DWG file from anyone, opened as the application opens it.
#![no_main]

mod common;

use arq_core::FileFormat;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| common::open_as_the_application_does(data, FileFormat::Dwg));
