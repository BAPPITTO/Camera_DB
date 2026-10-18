<p align="center">
  <img alt="license" src="https://img.shields.io/badge/license-MIT-blue.svg">
  <img alt="rust" src="https://img.shields.io/badge/language-Rust%202021-orange.svg">
  <img alt="android" src="https://img.shields.io/badge/platform-Android%20API%2024%2B-green.svg">
  <a href="https://github.com/your-org/gear-compat-android/releases/latest"><img alt="release" src="https://img.shields.io/github/v/release/your-org/gear-compat-android?include_prereleases"></a>
</p>

# Gear Compat — Sony E-mount Compatibility, in Pure Rust

**Gear Compat** is a native Android application that answers one question
instantly: *"Can I mount this lens on this body — and what actually happens to
my focal length, resolution, and stabilisation when I do?"* It also screens
hot-shoe flashes for dangerous trigger voltages.

There is **no backend and no network permission**. The master database of camera
bodies, lenses and flashes — plus your own custom gear (SD cards, batteries,
filters stored as JSON) — lives in an **embedded SQLite database** inside the
app's private storage. The entire stack is Rust: `egui`/`eframe` renders the UI
natively (no WebView), `rusqlite` (bundled) owns the data, and `cargo-apk` /
`cargo-ndk` cross-compile it straight to the APK.

## Features

* **Body ↔ Lens compatibility engine** with explicit status codes:
  * `CROP_MODE` — APS-C ("E") lens on a full-frame body → effective focal
    length × 1.5, effective megapixels × 0.444, vignetting warning.
  * `NATIVE_CROP` — full-frame ("FE") lens on an APS-C body → focal length × 1.5,
    full sensor readout.
  * `NATIVE_FULL_FRAME` / `NATIVE_APS_C` — matched pairings at 1.0×.
* **Flash safety check**: any unit with `trigger_voltage > 6.0 V` is flagged
  `HIGH_VOLTAGE_RISK` so you never cook a legacy flash into a modern hot shoe.
* **IBIS vs OSS notes** per combination, zoom ranges, crop-mode MP figures.
* **Custom gear manager**: save SD cards / batteries / filters with arbitrary
  JSON specs, e.g. `{"read_speed": 300, "write_speed": 260, "capacity_gb": 128}`.
* **Seeded offline corpus** (Alpha 7R V/7 IV/7 III/7C II, 6700/6600/6400/5100,
  E & FE glass, HVL/Godox flashes) created on first launch; user edits survive
  upgrades (idempotent `INSERT OR IGNORE` seeding).
* **CI/CD**: GitHub Actions compiles signed-installable APKs for `arm64-v8a` +
  `armeabi-v7a`, publishes them to GitHub Releases on `v*` tags, and attaches a
  source zip to the same release.

## Architecture

```
┌─────────────────────────────────────────────┐
│  egui / eframe UI            src/ui/*       │
├─────────────────────────────────────────────┤
│  Business logic (pure)       src/compatibility.rs
├─────────────────────────────────────────────┤
│  SQLite access               src/db/*       │
│  schema · seed · CRUD        rusqlite bundled
└─────────────────────────────────────────────┘
        android_logger · ndk · ndk-context
```

Tables: `camera_bodies`, `lenses`, `flashes`, `custom_gear` — enum columns are
enforced with SQL `CHECK` constraints (`APS_C|FULL_FRAME`, `E|FE`,
`SD_CARD|BATTERY|FILTER`).

## Building from Source

### Requirements

* **Rust** ≥ 1.77 (edition 2021) — [rustup.rs](https://rustup.rs)
* **Android NDK r26b** (26.1.10909125) with the platform toolchains
* **JDK 17** + Android SDK (platform 33, build-tools 33.0.2) for Gradle packaging
* Cargo subcommands: `cargo install cargo-apk cargo-ndk`
* Add the targets:
  ```sh
  rustup target add aarch64-linux-android armv7-linux-androideabi
  export ANDROID_NDK_HOME=$HOME/Android/Sdk/ndk/26.1.10909125   # adjust path
  ```

### Desktop preview (fastest iteration)

```sh
cargo run              # opens an egui window; DB at ./data/gear_compat.db
cargo test             # schema, seeding and compatibility-rule unit tests
```

### Android APK — cargo-apk path

```sh
cargo apk build --release
# Output: target/aarch64-linux-android/release/apk/gear_compat.apk
adb install -r target/aarch64-linux-android/release/apk/gear_compat.apk
```

### Android APK — cargo-ndk + Gradle path

```sh
cargo ndk -t arm64-v8a -t armeabi-v7a -o app/src/main/jniLibs build --release
./gradlew :app:assembleRelease
# Output: app/build/outputs/apk/release/app-release.apk
```

Both flows run automatically in CI (`.github/workflows/build-apk.yml`); grab
the freshest artifact from [GitHub Releases](../../releases).

### Troubleshooting

| Symptom | Fix |
| --- | --- |
| `linker ... not found` | `ANDROID_NDK_HOME` points at a valid r26b install? |
| `couldn't read ... sqlite3.c` | `rusqlite` `bundle` feature needs clang from NDK r26+ on PATH |
| Black screen on device | Device lacks GLES 2.0 — unsupported renderer, see manifest `<uses-feature>` |
| Crash debugging | `adb logcat -s gear_compat` (android_logger captures everything) |

## Repository Layout

```
Cargo.toml                crate + [package.metadata.android] config
.cargo/config.toml        Android linker flags
src/main.rs               entry point (NativeActivity / desktop)
src/lib.rs                module wiring + data-dir resolution
src/models.rs             schema-mirroring structs & enums
src/compatibility.rs      crop-factor / MP / trigger-voltage rules + tests
src/db/{mod.rs,schema.rs} rusqlite layer, DDL, idempotent seed corpus
src/ui/{mod.rs,app.rs}    egui tabs: Compatibility · Flashes · My Gear · About
app/                      AndroidManifest.xml + minimal Gradle project
assets/                   optional OTA seed placeholder
.github/workflows/        build-apk.yml · package-source.yml
.github/ISSUE_TEMPLATE/   body/lens/flash data request forms (typed)
```

## Contributing

Data corrections go through the typed issue templates (body / lens / flash) —
see [CONTRIBUTING.md](CONTRIBUTING.md) for the sourcing rules, especially the
citation requirement for trigger voltages. Code PRs must pass
`cargo fmt`, `cargo clippy -D warnings`, and `cargo test`.

## License

Released under the [MIT License](LICENSE). Guide numbers, sensor sizes and
model names are facts contributed by the community; verify critical values
before trusting your hardware to them.
