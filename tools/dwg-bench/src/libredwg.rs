//! LibreDWG through `shim/shim.c`: the adapter a product integration would
//! need too, kept as small as the C API allows.

use std::ffi::{CStr, CString, c_char, c_int};
use std::path::Path;
use std::time::Instant;

use crate::summary::Summary;

unsafe extern "C" {
    fn lr_read_json(path: *const c_char, error: *mut c_int) -> *mut c_char;
    fn lr_free(s: *mut c_char);
    fn lr_write(input: *const c_char, output: *const c_char, version: *const c_char) -> c_int;
    fn lr_critical() -> c_int;
}

fn c_path(path: &Path) -> Result<CString, String> {
    CString::new(path.to_string_lossy().as_bytes()).map_err(|e| e.to_string())
}

pub fn read(path: &Path) -> Result<Summary, String> {
    let p = c_path(path)?;
    let mut error: c_int = 0;
    // SAFETY: `p` is a valid NUL-terminated string; the shim returns either
    // NULL or a malloc'd NUL-terminated string that we free with lr_free.
    let json = unsafe { lr_read_json(p.as_ptr(), &mut error) };
    if json.is_null() {
        return Err(format!("read error 0x{error:x}"));
    }
    // SAFETY: non-null, NUL-terminated, owned by us until lr_free.
    let text = unsafe { CStr::from_ptr(json) }
        .to_string_lossy()
        .into_owned();
    // SAFETY: returned by lr_read_json, freed once.
    unsafe { lr_free(json) };
    serde_json::from_str(&text).map_err(|e| format!("summary: {e}"))
}

/// Writes `output`: DWG as R2000, the version LibreDWG writes reliably,
/// unless the name asks for another (`….r2018.dwg`); DXF as R2000.
/// Returns the write time in milliseconds.
pub fn write(input: &Path, output: &Path) -> Result<f64, String> {
    let name = output.to_string_lossy();
    let version = if name.contains(".r2018.") {
        "r2018"
    } else {
        "r2000"
    };
    let (i, o) = (c_path(input)?, c_path(output)?);
    let v = CString::new(version).expect("no NUL");
    let start = Instant::now();
    // SAFETY: all three are valid NUL-terminated strings.
    let error = unsafe { lr_write(i.as_ptr(), o.as_ptr(), v.as_ptr()) };
    let millis = start.elapsed().as_secs_f64() * 1e3;
    // SAFETY: a constant from the library.
    let critical = unsafe { lr_critical() };
    if error >= critical {
        return Err(format!("write error 0x{error:x} as {version}"));
    }
    if !output.is_file() {
        return Err(format!("no file written as {version}"));
    }
    Ok(millis)
}
