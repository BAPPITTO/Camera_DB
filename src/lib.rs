//! `gear_compat` - native Android camera gear compatibility application.
//!
//! Pure Rust stack: egui/eframe UI, rusqlite (bundled SQLite) local database,
//! serde/serde_json for user-defined custom gear specs, ndk for lifecycle.

pub mod compatibility;
pub mod db;
pub mod models;
pub mod ui;

use std::cell::RefCell;
use std::path::PathBuf;

thread_local! {
    /// App-private files directory supplied by the Android activity at
    /// startup (equivalent to `Context.getFilesDir()`). Set in `main.rs`.
    pub static ANDROID_DATA_DIR: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// Publish the Android app files dir so every module can locate the DB.
pub fn set_android_data_dir(path: PathBuf) {
    ANDROID_DATA_DIR.with(|c| *c.borrow_mut() = Some(path));
}

/// Resolve the directory in which the local SQLite database lives.
///
/// On Android we use the app-private files dir reported by the native
/// activity, so no storage permission is required. On desktop (development /
/// unit tests) we fall back to a local `./data` directory.
pub fn data_dir() -> PathBuf {
    #[cfg(target_os = "android")]
    {
        ANDROID_DATA_DIR
            .with(|c| c.borrow().clone())
            .unwrap_or_else(|| PathBuf::from("/data/data/com.gearcompat.rust/files"))
    }
    #[cfg(not(target_os = "android"))]
    {
        PathBuf::from("./data")
    }
}
