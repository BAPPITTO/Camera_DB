//! Build script for `gear_compat` (Android, Rust + embedded Kùzu C++ engine).
//!
//! Two responsibilities, in strict order:
//!
//!   1. Ahead-of-time compilation of `ui/app-window.slint` into Rust
//!      (zero markup parsing at runtime -> flat frame times on 165 Hz panels).
//!
//!   2. Cross-compilation of the Kùzu C++ static library via CMake + the NDK
//!      toolchain file, followed by *pre-link artifact verification* with
//!      llvm-nm so a malformed archive aborts here with a readable message
//!      instead of failing in rustc's link step ten minutes later.
//!
//! The CMake path only runs when cross-compiling to Android (cargo-ndk sets
//! ANDROID_NDK_HOME / ANDROID_ABI / ANDROID_PLATFORM and CARGO_TARGET_OS is
//! android). Host builds (`cargo test`) skip it; they never link libkuzu.

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Exact symbols build.rs requires inside libkuzu_c_api.a BEFORE rustc is
/// told to link it. Keep in sync with extern "C" block in src/graph/mod.rs
/// and the declarations in external/kuzu_shim/include/kuzu_shim.h.
const REQUIRED_KUZU_SYMBOLS: &[&str] = &[
    "kuzu_database_open",
    "kuzu_database_close",
    "kuzu_connection_create",
    "kuzu_connection_destroy",
    "kuzu_connection_query",
    "kuzu_connection_num_query_results",
    "kuzu_free_string",
    "kuzu_version",
];

fn main() {
    compile_slint_ui();

    if is_android_target() {
        let static_lib = build_kuzu_with_cmake();
        verify_exported_symbols(&static_lib);
        emit_link_directives(&static_lib);
    } else {
        // Host/dev builds must still re-run when the native sources change,
        // so switching between host and Android builds stays correct.
        println!("cargo:rerun-if-changed=external/kuzu_shim");
    }
}

// -----------------------------------------------------------------------------
// 1. Slint AOT compilation
// -----------------------------------------------------------------------------

fn compile_slint_ui() {
    println!("cargo:rerun-if-changed=ui/app-window.slint");
    println!("cargo:rerun-if-changed=ui");

    let compiler = slint_build::CompilerConfiguration::new().with_style("material".into());

    if let Err(e) = slint_build::compile_with_config("ui/app-window.slint", compiler) {
        panic!("Slint UI compilation failed: {e}");
    }
}

// -----------------------------------------------------------------------------
// 2. Environment plumbing
// -----------------------------------------------------------------------------

fn var_or_fail(key: &str) -> String {
    match env::var(key) {
        Ok(v) if !v.trim().is_empty() => v,
        _ => fail(&format!(
            "environment variable `{key}` is not set.\n\n\
             If you are building for Android, invoke this crate through cargo-ndk, \
             which injects ANDROID_NDK_HOME / ANDROID_ABI / ANDROID_PLATFORM:\n\n\
             \x20   cargo ndk -t arm64-v8a -o app/src/main/jniLibs build --release\n\n\
             A bare `cargo build --target aarch64-linux-android` does NOT set them. \
             Alternatively export them manually, e.g.:\n\n\
             \x20   export ANDROID_NDK_HOME=$HOME/Android/Sdk/ndk/26.1.10909125\n\
             \x20   export ANDROID_ABI=arm64-v8a\n\
             \x20   export ANDROID_PLATFORM=android-24"
        )),
    }
}

fn fail(msg: &str) -> ! {
    // panic in a build script = nonzero exit + the message surfaced by cargo.
    panic!("\n=== gear_compat build.rs FAILURE ===\n{msg}\n=====================================\n");
}

fn is_android_target() -> bool {
    // TARGET is always set by cargo during build-script execution.
    env::var("CARGO_CFG_TARGET_OS").map(|os| os == "android").unwrap_or(false)
        || env::var("TARGET")
            .map(|t| t.ends_with("-linux-android") || t.ends_with("-linux-androideabi"))
            .unwrap_or(false)
}

// -----------------------------------------------------------------------------
// 3. CMake invocation
// -----------------------------------------------------------------------------

/// Configure + build the Kùzu static library, returning the absolute path of
/// the generated `libkuzu_c_api.a`.
fn build_kuzu_with_cmake() -> PathBuf {
    let ndk_home = var_or_fail("ANDROID_NDK_HOME");
    let abi = var_or_fail("ANDROID_ABI");
    let platform = var_or_fail("ANDROID_PLATFORM");

    let out_dir = PathBuf::from(var_or_fail_simple("OUT_DIR"));
    let build_dir = out_dir.join("kuzu-build");
    let src_dir = PathBuf::from(var_or_fail_simple("CARGO_MANIFEST_DIR")).join("external/kuzu_shim");

    let toolchain_file =
        Path::new(&ndk_home).join("build").join("cmake").join("android.toolchain.cmake");
    if !toolchain_file.exists() {
        fail(&format!(
            "NDK toolchain file not found at `{}`.\n\
             ANDROID_NDK_HOME points at an incomplete or wrong-version NDK. \
             This pipeline requires r26b (see .github/workflows/build-apk.yml).",
            toolchain_file.display()
        ));
    }

    for dir in [&src_dir, &build_dir] {
        if !dir.exists() && dir == &src_dir {
            fail(&format!(
                "Kuzu CMake source tree missing at `{}`.",
                dir.display()
            ));
        }
    }
    std::fs::create_dir_all(&build_dir)
        .unwrap_or_else(|e| fail(&format!("cannot create {}: {e}", build_dir.display())));

    // Exact CMake invocation. Generator: Ninja when present (CI has it),
    // otherwise Unix Makefiles. cmake-rs equivalent flags spelled out below.
    let generator = if which("ninja").is_some() { "Ninja" } else { "Unix Makefiles" };

    let configure_status = Command::new("cmake")
        .current_dir(&build_dir)
        .arg("-G")
        .arg(generator)
        .arg(format!("-DCMAKE_TOOLCHAIN_FILE={}", toolchain_file.display()))
        .arg(format!("-DANDROID_ABI={abi}"))
        .arg(format!("-DANDROID_PLATFORM={platform}"))
        .arg("-DANDROID_STL=c++_shared")
        .arg("-DCMAKE_BUILD_TYPE=Release")
        .arg("-DCMAKE_CXX_STANDARD=20")
        .arg("-DCMAKE_CXX_STANDARD_REQUIRED=ON")
        .arg("-DKUZU_ENABLE_STATIC_ON_WINDOWS=OFF")
        .arg("-DCMAKE_POSITION_INDEPENDENT_CODE=ON")
        .arg(&src_dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .unwrap_or_else(|e| {
            fail(&format!(
                "failed to execute `cmake` for the configure step: {e}.\n\
                 Install CMake >= 3.20 and ensure it is on PATH."
            ))
        });
    if !configure_status.success() {
        fail("CMake CONFIGURE step for libkuzu_c_api failed (see output above).");
    }

    let jobs = num_cpus_hint();
    let build_status = Command::new("cmake")
        .current_dir(&build_dir)
        .args(["--build", ".", "--config", "Release", "-j"])
        .arg(jobs.to_string())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .unwrap_or_else(|e| fail(&format!("failed to execute `cmake --build`: {e}")));
    if !build_status.success() {
        fail("CMake BUILD step for libkuzu_c_api failed (see output above).");
    }

    // Locate the archive. Ninja/Makefile generators drop it directly in the
    // build root; multi-config generators nest under Release/.
    let candidates = [
        build_dir.join("libkuzu_c_api.a"),
        build_dir.join("Release").join("libkuzu_c_api.a"),
        build_dir.join("lib").join("libkuzu_c_api.a"),
    ];
    candidates
        .into_iter()
        .find(|p| p.exists())
        .unwrap_or_else(|| {
            fail(&format!(
                "CMake reported success but no libkuzu_c_api.a was found under {}.\n\
                 Expected exactly one of:\n  {}\n",
                build_dir.display(),
                ["libkuzu_c_api.a", "Release/libkuzu_c_api.a", "lib/libkuzu_c_api.a"]
                    .join("\n  ")
            ))
        })
}

fn var_or_fail_simple(key: &str) -> String {
    env::var(key).unwrap_or_else(|_| fail(&format!("`{key}` must be set during build-script execution")))
}

fn num_cpus_hint() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).min(16)
}

fn which(bin: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .map(|dir| dir.join(bin))
            .find(|full| full.is_file())
    })
}

// -----------------------------------------------------------------------------
// 4. Pre-link linkage verification (llvm-nm / llvm-objdump from the NDK)
// -----------------------------------------------------------------------------

/// Inspect the freshly built archive with the NDK's llvm-nm (falling back to
/// llvm-objdump, then to a PATH lookup) and assert every required Kùzu C API
/// symbol is defined (T/D/B/R — NOT undefined 'U'). Aborts the build with a
/// descriptive error listing exactly what is missing.
fn verify_exported_symbols(static_lib: &Path) {
    let nm = find_llvm_tool("llvm-nm");
    let objdump = find_llvm_tool("llvm-objdump");

    let mut defined: Vec<String> = Vec::new();

    // Primary: llvm-nm. Defined global text/data symbols show as lowercase
    // letters for local / uppercase T D B R V W for globals we care about.
    if let Some(nm_path) = &nm {
        let output = Command::new(nm_path)
            .arg("--defined-only")
            .arg("--extern-only")
            .arg(static_lib)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap_or_else(|e| {
                fail(&format!("executing `{}` failed: {e}", nm_path.display()))
            });
        if !output.status.success() {
            fail(&format!(
                "`{} --defined-only --extern-only {}` exited with {}:{}",
                nm_path.display(),
                static_lib.display(),
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            // Format: "0000000000000120 T kuzu_database_open" (or with the
            // object-file prefix "foo.cpp.o:0000... T sym" on archives).
            if let Some(sym) = line.split_whitespace().last() {
                defined.push(sym.to_string());
            }
        }
    } else if let Some(od_path) = &objdump {
        // Fallback: llvm-objdump -T works on .so; for .a use -t (symbol table).
        let output = Command::new(od_path)
            .arg("-t")
            .arg(static_lib)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap_or_else(|e| {
                fail(&format!("executing `{}` failed: {e}", od_path.display()))
            });
        if !output.status.success() {
            fail(&format!("`{} -t {}` failed", od_path.display(), static_lib.display()));
        }
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if let Some(sym) = line.split_whitespace().last() {
                defined.push(sym.to_string());
            }
        }
    } else {
        fail(
            "neither llvm-nm nor llvm-objdump could be located.\n\
             They ship inside the NDK at toolchains/llvm/prebuilt/<host>/bin/.\n\
             Ensure ANDROID_NDK_HOME is valid and cargo-ndk is being used.",
        );
    }

    let missing: Vec<&str> = REQUIRED_KUZU_SYMBOLS
        .iter()
        .copied()
        .filter(|sym| !defined.iter().any(|d| d == sym))
        .collect();

    if !missing.is_empty() {
        fail(&format!(
            "ARTIFACT VERIFICATION FAILED: `{}` does not export the required Kùzu C API symbols:\n\n  {}\n\n\
             This means the C++ static library is malformed, was compiled without the C API \
             translation unit, or the ABI filter dropped it. ABORTING BEFORE LINK so rustc does \
             not fail 10 minutes later with a cryptic `undefined reference` wall.\n\n\
             Symbols actually found ({} total): rerun `{}` manually and inspect.",
            static_lib.display(),
            missing.join("\n  "),
            defined.len(),
            nm.map(|p| p.display().to_string())
                .or_else(|| objdump.map(|p| p.display().to_string()))
                .unwrap_or_else(|| "llvm-nm".into()),
        ));
    }

    println!(
        "cargo:warning=libkuzu_c_api.a verified: all {} required symbols exported ({})",
        REQUIRED_KUZU_SYMBOLS.len(),
        static_lib.display()
    );
}

/// Prefer the NDK-bundled tool (correct ELF support, matches the linker that
/// produced the archive), fall back to a system PATH copy.
fn find_llvm_tool(tool: &str) -> Option<PathBuf> {
    if let Ok(ndk_home) = env::var("ANDROID_NDK_HOME") {
        let host_dirs = [
            "linux-x86_64",
            "darwin-x86_64",
            "windows-x86_64",
        ];
        for host in host_dirs {
            let candidate = Path::new(&ndk_home)
                .join("toolchains")
                .join("llvm")
                .join("prebuilt")
                .join(host)
                .join("bin")
                .join(tool);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    which(tool)
}

// -----------------------------------------------------------------------------
// 5. Link directives (emitted ONLY after verification passes)
// -----------------------------------------------------------------------------

fn emit_link_directives(static_lib: &Path) {
    let lib_dir = static_lib.parent().unwrap_or_else(|| {
        fail("computed static library path has no parent directory");
    });

    // Search path + our archive.
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static=kuzu_c_api");

    // Kùzu is C++20 and uses the LLVM libc++ STL. On Android this is
    // `c++_shared`, shipped by the NDK and packaged by cargo-ndk/cargo-apk
    // into jniLibs. Declared explicitly because a static C++ archive pulled
    // into a cdylib does NOT get STL linkage inferred.
    println!("cargo:rustc-link-lib=c++_shared");

    // Standard system libs the engine expects when linked into libmain.so.
    println!("cargo:rustc-link-lib=dylib=log");
    println!("cargo:rustc-link-lib=dylib=android");

    // Re-run whenever any native input changes.
    println!("cargo:rerun-if-changed=external/kuzu_shim/CMakeLists.txt");
    println!("cargo:rerun-if-changed=external/kuzu_shim/include");
    println!("cargo:rerun-if-changed=external/kuzu_shim/src");
    println!("cargo:rerun-if-env-changed=ANDROID_NDK_HOME");
    println!("cargo:rerun-if-env-changed=ANDROID_ABI");
    println!("cargo:rerun-if-env-changed=ANDROID_PLATFORM");
}
