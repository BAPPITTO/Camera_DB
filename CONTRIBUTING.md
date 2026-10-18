# Contributing to Gear Compat

Thanks for helping make the Sony E-mount compatibility data trustworthy. There
are two kinds of contributions: **data corrections** (most common) and **code**.

---

## Data corrections & additions

All gear records live in the embedded SQLite database seeded from
`src/db/schema.rs`. We never accept "trust me" numbers - every figure needs a
citable source.

### 1. Pick the right issue template

Open a new issue and choose exactly one typed template (blank issues are
disabled):

| Template | Use for |
| --- | --- |
| 📷 Camera Body Request / Correction (`body_request.yml`) | New/changed `camera_bodies` rows |
| 🔭 Lens Request / Correction (`lens_request.yml`) | New/changed `lenses` rows |
| ⚡ Flash Request / Correction (`flash_request.yml`) | New/changed `flashes` rows |

Fill **every** required field. For corrections, include the current value, the
proposed value, and the source URL (official spec sheet preferred).

### 2. Field rules (mirrored by SQL CHECK constraints)

* `sensor_type` / `target_sensor`: only `APS_C` or `FULL_FRAME`.
* `mount_designation`: only `E` (APS-C optics) or `FE` (full-frame circle).
* `gear_type` for custom gear: `SD_CARD`, `BATTERY`, or `FILTER`.
* Megapixels, focal lengths and apertures are decimals; weight is integer grams.

### 3. Trigger voltage = safety-critical

The app raises a **HIGH_VOLTAGE_RISK** warning above **6.0 V**. For flash
submissions:

* Provide a measured value or a datasheet reference - never an estimate.
* If you believe an existing entry's voltage is wrong and could damage
  hardware, use the *Security Report* contact link instead of a public issue.

### 4. What happens next

A maintainer applies approved changes to the seed arrays in
`src/db/schema.rs` (and bumps `SCHEMA_VERSION` if the schema itself changes).
The change ships in the next tagged release built by CI.

---

## Code contributions

1. `git clone` + create a branch off `main` (`feat/...`, `fix/...`, `data/...`).
2. Keep the stack pure Rust: egui/eframe UI, rusqlite persistence, no backend,
   no webview, no JNI Java code.
3. Run before pushing:
   ```sh
   cargo fmt --all
   cargo clippy --all-targets -- -D warnings
   cargo test          # schema, seed idempotency, compatibility maths
   cargo run           # desktop dev build (./data/gear_compat.db)
   ```
4. Compatibility-rule changes must update `src/compatibility.rs` **and** its
   unit tests in the same commit (crop factor 1.5x, 0.444 area ratio, 6.0 V
   threshold are contractual behaviours).
5. Open a PR against `main`; CI builds the APK for both ABIs and the diff is
   reviewed for data-source citations.

Please don't commit `target/`, generated `.so` files, `.db` files, or signing
keystores - they're already in `.gitignore`.
