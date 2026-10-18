//! Entry point.
//!
//! * On Android, Slint's `backend-android-activity-05` feature exports the
//!   `android_main` symbol itself (NativeActivity loads it from `libmain.so`,
//!   as declared by the `android.app.lib_name` meta-data in the manifest).
//!   The backend owns the event loop, the ANativeWindow surface and touch
//!   input; we only provide a `#[no_mangle] extern "C" fn slint_main()` that
//!   it calls once the platform is up.
//! * On desktop the same code runs under Slint's default winit + FemtoVG
//!   (OpenGL) platform, so `cargo run` gives an instant dev window.

use gear_compat_lib::db::Database;
use gear_compat_lib::ui::create_app_window;
use slint::ComponentHandle;

/// Open the local SQLite database (creating + seeding it on first launch) and
/// start the Slint event loop. This body runs identically on both platforms.
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = gear_compat_lib::data_dir();
    log::info!("opening local database in {}", data_dir.display());

    // First launch creates + seeds the SQLite file; later launches reuse it.
    let db = Database::open_in_dir(&data_dir)?;

    let ui = create_app_window(db)?;
    ui.run()?; // blocks until the last window closes / activity is destroyed
    Ok(())
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn slint_main() {
    // Route every log line into logcat so crashes/DB errors are visible via
    // `adb logcat -s gear_compat`.
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Debug)
            .with_tag("gear_compat"),
    );
    // Prove the NativeActivity glue is attached before touching storage.
    let ctx = ndk_context::android_context();
    log::info!(
        "android context: manifest={}, class_loader={}",
        ctx.manifest_path(),
        ctx.class_loader()
    );

    if let Err(e) = run() {
        log::error!("fatal: {e}");
    }
}

#[cfg(not(target_os = "android"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Desktop logging goes to stderr through Slint's own `log` integration;
    // no extra logger crate is pulled into the APK dependency tree.
    if let Err(e) = run() {
        eprintln!("gear_compat: fatal error: {e}");
        return Err(e);
    }
    Ok(())
}
