//! Builds the LibreDWG adapter when the `libredwg` feature is on.
//!
//! LIBREDWG_DIR: installed prefix (lib/libredwg.so).
//! LIBREDWG_SRC: source tree (include/ and src/, for the DXF writer's
//! internal Bit_Chain).

fn main() {
    #[cfg(feature = "libredwg")]
    {
        let prefix = std::env::var("LIBREDWG_DIR").expect("set LIBREDWG_DIR");
        let source = std::env::var("LIBREDWG_SRC").expect("set LIBREDWG_SRC");
        cc::Build::new()
            .file("shim/shim.c")
            .include(format!("{source}/include"))
            .include(format!("{source}/src"))
            .include(&source)
            .warnings(false)
            .compile("dwgbench_shim");
        println!("cargo:rustc-link-search=native={prefix}/lib");
        println!("cargo:rustc-link-lib=dylib=redwg");
        println!("cargo:rustc-link-arg=-Wl,-rpath,{prefix}/lib");
        println!("cargo:rerun-if-changed=shim/shim.c");
        println!("cargo:rerun-if-env-changed=LIBREDWG_DIR");
    }
}
