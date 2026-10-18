//! `gear_compat` - native Android camera gear compatibility application.
//!
//! Pure Rust stack: Slint (retained-mode, declarative, ahead-of-time compiled
//! UI) + rusqlite (bundled SQLite) local database + serde/serde_json for
//! user-defined custom gear specs + ndk for the Android lifecycle.

pub mod compatibility;
pub mod db;
pub mod models;
pub mod ui;

use std::path::PathBuf;

/// Resolve the directory in which the local SQLite database lives.
///
/// On Android we use the app-private files dir (`Context.getFilesDir()`,
/// i.e. `/data/data/<package>/files`), which requires no storage permission.
/// On desktop (development / unit tests) we fall back to a local `./data`.
pub fn data_dir() -> PathBuf {
    #[cfg(target_os = "android")]
    {
        // The path is deterministic from the package name declared in
        // Cargo.toml `[package.metadata.android] package`. `ndk-context` is
        // probed in `main.rs` to prove the NativeActivity is attached.
        PathBuf::from("/data/data/com.gearcompat.rust/files")
    }
    #[cfg(not(target_os = "android"))]
    {
        PathBuf::from("./data")
    }
}
