//! Ahead-of-time compilation of `ui/app-window.slint` into Rust.
//!
//! `slint-build` invokes the Slint compiler at build time; the generated code
//! is emitted to `$OUT_DIR/slint_generated.rs` and pulled in with
//! `include!(concat!(env!("OUT_DIR"), "/slint_generated.rs"))` from
//! `src/ui/mod.rs`. No .slint file is parsed or interpreted at runtime, which
//! is exactly what keeps frame times flat on the 165 Hz panel.

fn main() {
    // Re-run when the markup changes.
    println!("cargo:rerun-if-changed=ui/app-window.slint");
    println!("cargo:rerun-if-changed=ui");

    let compiler = slint_build::CompilerConfiguration::new().with_style("material".into());

    if let Err(e) = slint_build::compile_with_config("ui/app-window.slint", compiler) {
        panic!("Slint UI compilation failed: {e}");
    }
}
