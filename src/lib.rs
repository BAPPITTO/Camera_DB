//! `gear_compat` - native Android camera gear compatibility application.
//!
//! Pure Rust stack: Slint (retained-mode, declarative, ahead-of-time compiled
//! UI) + rusqlite (bundled SQLite) local database + serde/serde_json for
//! user-defined custom gear specs + ndk for the Android lifecycle.

pub mod compatibility;
pub mod db;
pub mod graph;
pub mod models;
pub mod ui;

use std::path::PathBuf;

/// Resolve the directory in which the local SQLite database lives.
///
/// On Android we ask the actual JVM `Context` for its app-private files dir
/// via `ndk-context` (`Context.getFilesDir()`), which requires no storage
/// permission and stays correct regardless of what the package name is or
/// where the OS decides to mount app storage. If — and only if — the native
/// activity is not yet attached (e.g. a headless instrumentation harness),
/// we fall back to the deterministic path derived from the package declared
/// in Cargo.toml. On desktop (development / unit tests) we use `./data`.
pub fn data_dir() -> PathBuf {
    #[cfg(target_os = "android")]
    {
        match ndk_context::android_context().internal_data_path() {
            Some(c_path) => {
                let mut dir = PathBuf::from(c_path);
                dir.push("files");
                dir
            }
            // Activity not attached yet: deterministic fallback matching
            // `[package.metadata.android] package` in Cargo.toml.
            None => PathBuf::from("/data/data/com.gearcompat.rust/files"),
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        PathBuf::from("./data")
    }
}
