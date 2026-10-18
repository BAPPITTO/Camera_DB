//! SQLite DDL and idempotent seed data for the master gear database.
//!
//! All statements are `CREATE TABLE IF NOT EXISTS` / `INSERT OR IGNORE` so
//! `init_schema` + `seed` can run on every app launch without destroying user
//! edits.

use rusqlite::Connection;

use crate::models::{CameraBody, Flash, Lens, MountDesignation, SensorType};

/// Current schema version, tracked in `PRAGMA user_version`.
pub const SCHEMA_VERSION: i64 = 1;

// ---------------------------------------------------------------------------
// DDL
// ---------------------------------------------------------------------------

pub const SQL_CREATE_CAMERA_BODIES: &str = "
CREATE TABLE IF NOT EXISTS camera_bodies (
    id                  INTEGER PRIMARY KEY,
    model_name          TEXT    NOT NULL UNIQUE,
    marketing_name      TEXT    NOT NULL,
    sensor_type         TEXT    NOT NULL CHECK (sensor_type IN ('APS_C', 'FULL_FRAME')),
    base_megapixels     REAL    NOT NULL,
    aps_crop_megapixels REAL,
    has_ibis            INTEGER NOT NULL DEFAULT 0,
    release_year        INTEGER
);
";

pub const SQL_CREATE_LENSES: &str = "
CREATE TABLE IF NOT EXISTS lenses (
    id                INTEGER PRIMARY KEY,
    model_name        TEXT    NOT NULL UNIQUE,
    marketing_name    TEXT    NOT NULL,
    mount_designation TEXT    NOT NULL CHECK (mount_designation IN ('E', 'FE')),
    target_sensor     TEXT    NOT NULL CHECK (target_sensor IN ('APS_C', 'FULL_FRAME')),
    focal_length_min  REAL    NOT NULL,
    focal_length_max  REAL    NOT NULL,
    max_aperture_min  REAL    NOT NULL,
    has_oss           INTEGER NOT NULL DEFAULT 0,
    weight_grams      INTEGER
);
";

pub const SQL_CREATE_FLASHES: &str = "
CREATE TABLE IF NOT EXISTS flashes (
    id              INTEGER PRIMARY KEY,
    model_name      TEXT    NOT NULL UNIQUE,
    guide_number    REAL    NOT NULL,
    trigger_voltage REAL    NOT NULL,
    ttl_compatible  INTEGER NOT NULL DEFAULT 0
);
";

pub const SQL_CREATE_CUSTOM_GEAR: &str = "
CREATE TABLE IF NOT EXISTS custom_gear (
    id         INTEGER PRIMARY KEY,
    user_name  TEXT    NOT NULL,
    gear_type  TEXT    NOT NULL CHECK (gear_type IN ('SD_CARD', 'BATTERY', 'FILTER')),
    specs_json TEXT    NOT NULL DEFAULT '{}'
);
";

/// Auxiliary indexes for the list queries issued by the UI.
pub const SQL_CREATE_INDEXES: &str = "
CREATE INDEX IF NOT EXISTS idx_bodies_sensor      ON camera_bodies (sensor_type);
CREATE INDEX IF NOT EXISTS idx_lenses_mount       ON lenses (mount_designation);
CREATE INDEX IF NOT EXISTS idx_lenses_target      ON lenses (target_sensor);
CREATE INDEX IF NOT EXISTS idx_custom_gear_type   ON custom_gear (gear_type);
";

/// Create every table if it does not already exist and stamp the version.
pub fn init_schema(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    conn.execute_batch(SQL_CREATE_CAMERA_BODIES)?;
    conn.execute_batch(SQL_CREATE_LENSES)?;
    conn.execute_batch(SQL_CREATE_FLASHES)?;
    conn.execute_batch(SQL_CREATE_CUSTOM_GEAR)?;
    conn.execute_batch(SQL_CREATE_INDEXES)?;
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Seed data (Sony E-mount corpus)
// ---------------------------------------------------------------------------

pub fn seed_camera_bodies() -> Vec<CameraBody> {
    vec![
    CameraBody { id: 1, model_name: "ILCE-7RM5".to_string(), marketing_name: "Alpha 7R V".to_string(), sensor_type: SensorType::FullFrame, base_megapixels: 61.0, aps_crop_megapixels: Some(26.0), has_ibis: true, release_year: Some(2022) },
    CameraBody { id: 2, model_name: "ILCE-7M4".to_string(), marketing_name: "Alpha 7 IV".to_string(), sensor_type: SensorType::FullFrame, base_megapixels: 33.0, aps_crop_megapixels: Some(14.0), has_ibis: true, release_year: Some(2021) },
    CameraBody { id: 3, model_name: "ILCE-7M3".to_string(), marketing_name: "Alpha 7 III".to_string(), sensor_type: SensorType::FullFrame, base_megapixels: 24.2, aps_crop_megapixels: Some(10.0), has_ibis: true, release_year: Some(2018) },
    CameraBody { id: 4, model_name: "ILCE-7CM2".to_string(), marketing_name: "Alpha 7C II".to_string(), sensor_type: SensorType::FullFrame, base_megapixels: 33.0, aps_crop_megapixels: Some(14.0), has_ibis: true, release_year: Some(2023) },
    CameraBody { id: 5, model_name: "ILCE-6700".to_string(), marketing_name: "Alpha 6700".to_string(), sensor_type: SensorType::ApsC, base_megapixels: 26.0, aps_crop_megapixels: None, has_ibis: true, release_year: Some(2022) },
    CameraBody { id: 6, model_name: "ILCE-6600".to_string(), marketing_name: "Alpha 6600".to_string(), sensor_type: SensorType::ApsC, base_megapixels: 24.2, aps_crop_megapixels: None, has_ibis: false, release_year: Some(2019) },
    CameraBody { id: 7, model_name: "ILCE-6400".to_string(), marketing_name: "Alpha 6400".to_string(), sensor_type: SensorType::ApsC, base_megapixels: 24.2, aps_crop_megapixels: None, has_ibis: false, release_year: Some(2019) },
    CameraBody { id: 8, model_name: "ILCE-5100".to_string(), marketing_name: "Alpha 5100".to_string(), sensor_type: SensorType::ApsC, base_megapixels: 24.3, aps_crop_megapixels: None, has_ibis: false, release_year: Some(2014) },
]}

pub fn seed_lenses() -> Vec<Lens> {
    vec![
    Lens { id: 1, model_name: "SEL1655".to_string(), marketing_name: "E PZ 16-50mm F3.5-5.6 OSS".to_string(), mount_designation: MountDesignation::E, target_sensor: SensorType::ApsC, focal_length_min: 16.0, focal_length_max: 50.0, max_aperture_min: 3.5, has_oss: true, weight_grams: Some(116) },
    Lens { id: 2, model_name: "SEL18135".to_string(), marketing_name: "E 18-135mm F3.5-5.6 OSS".to_string(), mount_designation: MountDesignation::E, target_sensor: SensorType::ApsC, focal_length_min: 18.0, focal_length_max: 135.0, max_aperture_min: 3.5, has_oss: true, weight_grams: Some(325) },
    Lens { id: 3, model_name: "SEL1018".to_string(), marketing_name: "E 10-18mm F4 OSS".to_string(), mount_designation: MountDesignation::E, target_sensor: SensorType::ApsC, focal_length_min: 10.0, focal_length_max: 18.0, max_aperture_min: 4.0, has_oss: true, weight_grams: Some(225) },
    Lens { id: 4, model_name: "SEL16F14".to_string(), marketing_name: "FE 16mm F1.4 GM".to_string(), mount_designation: MountDesignation::FE, target_sensor: SensorType::FullFrame, focal_length_min: 16.0, focal_length_max: 16.0, max_aperture_min: 1.4, has_oss: false, weight_grams: Some(409) },
    Lens { id: 5, model_name: "SEL2470GM2".to_string(), marketing_name: "FE 24-70mm F2.8 GM II".to_string(), mount_designation: MountDesignation::FE, target_sensor: SensorType::FullFrame, focal_length_min: 24.0, focal_length_max: 70.0, max_aperture_min: 2.8, has_oss: false, weight_grams: Some(695) },
    Lens { id: 6, model_name: "SEL70200GM2".to_string(), marketing_name: "FE 70-200mm F2.8 GM OSS II".to_string(), mount_designation: MountDesignation::FE, target_sensor: SensorType::FullFrame, focal_length_min: 70.0, focal_length_max: 200.0, max_aperture_min: 2.8, has_oss: true, weight_grams: Some(1045) },
    Lens { id: 7, model_name: "SEL50F18F".to_string(), marketing_name: "FE 50mm F1.8".to_string(), mount_designation: MountDesignation::FE, target_sensor: SensorType::FullFrame, focal_length_min: 50.0, focal_length_max: 50.0, max_aperture_min: 1.8, has_oss: false, weight_grams: Some(186) },
    Lens { id: 8, model_name: "SEL35F18F".to_string(), marketing_name: "FE 35mm F1.8".to_string(), mount_designation: MountDesignation::FE, target_sensor: SensorType::FullFrame, focal_length_min: 35.0, focal_length_max: 35.0, max_aperture_min: 1.8, has_oss: false, weight_grams: Some(280) },
]}

pub fn seed_flashes() -> Vec<Flash> {
    vec![
        // Sony native TTL units, low trigger voltage.
        Flash { id: 1, model_name: "HVL-F46RM".to_string(), guide_number: 46.0, trigger_voltage: 5.0, ttl_compatible: true },
        Flash { id: 2, model_name: "HVL-F60RM2".to_string(), guide_number: 60.0, trigger_voltage: 5.0, ttl_compatible: true },
        Flash { id: 3, model_name: "HVL-F28RM".to_string(), guide_number: 28.0, trigger_voltage: 5.0, ttl_compatible: true },
        // Third-party legacy unit with a high-voltage hot shoe - used to exercise
        // the HIGH_VOLTAGE_RISK path in `compatibility.rs`.
        Flash { id: 4, model_name: "Metronix 56 (legacy)".to_string(), guide_number: 56.0, trigger_voltage: 350.0, ttl_compatible: false },
        Flash { id: 5, model_name: "Godox V860II-S".to_string(), guide_number: 60.0, trigger_voltage: 5.4, ttl_compatible: true },
    ]
}

/// Insert the master gear corpus. Idempotent via a table-level guard: if a
/// table already holds ANY row we skip seeding it entirely. Seeding by
/// `model_name` UNIQUE alone would re-insert the canonical row (with a fresh
/// autoincrement id) whenever a user corrects/renames a master row — leaving
/// duplicate gear with different ids. Master-data corrections belong in the
/// issue templates / seed snapshot, not in the live DB, so once seeded the
/// corpus is left exactly as the user has it until they clear app data.
pub fn seed(conn: &Connection) -> Result<(), rusqlite::Error> {
    let mut stmt_body = conn.prepare_cached(
        "INSERT INTO camera_bodies
            (model_name, marketing_name, sensor_type, base_megapixels,
             aps_crop_megapixels, has_ibis, release_year)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    if table_empty(conn, "camera_bodies")? {
        for b in &seed_camera_bodies() {
            stmt_body.execute(rusqlite::params![
                b.model_name,
                b.marketing_name,
                b.sensor_type.as_str(),
                b.base_megapixels,
                b.aps_crop_megapixels,
                b.has_ibis as i64,
                b.release_year,
            ])?;
        }
    }

    let mut stmt_lens = conn.prepare_cached(
        "INSERT INTO lenses
            (model_name, marketing_name, mount_designation, target_sensor,
             focal_length_min, focal_length_max, max_aperture_min, has_oss, weight_grams)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?;
    if table_empty(conn, "lenses")? {
        for l in &seed_lenses() {
            stmt_lens.execute(rusqlite::params![
                l.model_name,
                l.marketing_name,
                l.mount_designation.as_str(),
                l.target_sensor.as_str(),
                l.focal_length_min,
                l.focal_length_max,
                l.max_aperture_min,
                l.has_oss as i64,
                l.weight_grams,
            ])?;
        }
    }

    let mut stmt_flash = conn.prepare_cached(
        "INSERT INTO flashes
            (model_name, guide_number, trigger_voltage, ttl_compatible)
         VALUES (?1, ?2, ?3, ?4)",
    )?;
    if table_empty(conn, "flashes")? {
        for f in &seed_flashes() {
            stmt_flash.execute(rusqlite::params![
                f.model_name,
                f.guide_number,
                f.trigger_voltage,
                f.ttl_compatible as i64,
            ])?;
        }
    }

    Ok(())
}

/// True when `table` contains zero rows. Used to make [`seed`] a one-shot
/// bootstrap instead of a per-launch upsert.
fn table_empty(conn: &Connection, table: &'static str) -> Result<bool, rusqlite::Error> {
    // `table` is a hard-coded literal from this module only — never user
    // input — so interpolation into the SQL is safe here.
    let sql = format!("SELECT EXISTS (SELECT 1 FROM {table} LIMIT 1)");
    let has_rows: bool = conn.query_row(&sql, [], |row| row.get(0))?;
    Ok(!has_rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_and_seed_are_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        seed(&conn).unwrap();
        // Second run must not fail nor duplicate rows.
        init_schema(&conn).unwrap();
        seed(&conn).unwrap();

        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM camera_bodies", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, seed_camera_bodies().len() as i64);

        let v: String = conn
            .query_row(
                "SELECT sensor_type FROM camera_bodies WHERE model_name='ILCE-6700'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(v, "APS_C");
    }

    #[test]
    fn check_constraints_reject_bad_enum_values() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        let res = conn.execute(
            "INSERT INTO camera_bodies (model_name, marketing_name, sensor_type, base_megapixels, has_ibis)
             VALUES ('X','X','MEDIUM_FORMAT',100,0)",
            [],
        );
        assert!(res.is_err(), "CHECK constraint must reject MEDIUM_FORMAT");

        let res2 = conn.execute(
            "INSERT INTO lenses (model_name, marketing_name, mount_designation, target_sensor,
                                 focal_length_min, focal_length_max, max_aperture_min, has_oss)
             VALUES ('Y','Y','G','FULL_FRAME',50,50,1.4,0)",
            [],
        );
        assert!(res2.is_err(), "CHECK constraint must reject G mount");
    }
}
