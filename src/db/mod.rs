//! Embedded SQLite database layer: open/migrate/seed + typed CRUD.
//!
//! `Database` owns a single `rusqlite::Connection` guarded by a `Mutex`.
//! Slint dispatches all callbacks on the Android UI thread, so in practice
//! access is already serialised and the mutex is defensive-only — it exists
//! to keep the type `Sync` for any future background-task queries (e.g. the
//! Kùzu graph indexer) without reworking every call site.

pub mod schema;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::models::{CameraBody, CustomGear, Flash, GearType, Lens, MountDesignation, SensorType};

/// Filename inside the app-private files dir.
pub const DB_FILE_NAME: &str = "gear_compat.db";

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("could not create data directory {0:?}: {1}")]
    Io(PathBuf, #[source] std::io::Error),
    #[error("invalid JSON in custom_gear.specs_json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("database is poisoned (a previous write panicked)")]
    Poisoned,
}

pub type DbResult<T> = Result<T, DbError>;

// ---------------------------------------------------------------------------
// Row mappers
// ---------------------------------------------------------------------------

fn body_from_row(row: &Row<'_>) -> rusqlite::Result<CameraBody> {
    Ok(CameraBody {
        id: row.get("id")?,
        model_name: row.get("model_name")?,
        marketing_name: row.get("marketing_name")?,
        sensor_type: SensorType::from_db(&row.get::<_, String>("sensor_type")?),
        base_megapixels: row.get("base_megapixels")?,
        aps_crop_megapixels: row.get("aps_crop_megapixels")?,
        has_ibis: row.get::<_, i64>("has_ibis")? != 0,
        release_year: row.get("release_year")?,
    })
}

fn lens_from_row(row: &Row<'_>) -> rusqlite::Result<Lens> {
    Ok(Lens {
        id: row.get("id")?,
        model_name: row.get("model_name")?,
        marketing_name: row.get("marketing_name")?,
        mount_designation: MountDesignation::from_db(&row.get::<_, String>("mount_designation")?),
        target_sensor: SensorType::from_db(&row.get::<_, String>("target_sensor")?),
        focal_length_min: row.get("focal_length_min")?,
        focal_length_max: row.get("focal_length_max")?,
        max_aperture_min: row.get("max_aperture_min")?,
        has_oss: row.get::<_, i64>("has_oss")? != 0,
        weight_grams: row.get("weight_grams")?,
    })
}

fn flash_from_row(row: &Row<'_>) -> rusqlite::Result<Flash> {
    Ok(Flash {
        id: row.get("id")?,
        model_name: row.get("model_name")?,
        guide_number: row.get("guide_number")?,
        trigger_voltage: row.get("trigger_voltage")?,
        ttl_compatible: row.get::<_, i64>("ttl_compatible")? != 0,
    })
}

fn gear_from_row(row: &Row<'_>) -> rusqlite::Result<CustomGear> {
    let raw_type: String = row.get("gear_type")?;
    let gear_type = GearType::from_db(&raw_type).unwrap_or(GearType::Filter);
    Ok(CustomGear {
        id: row.get("id")?,
        user_name: row.get("user_name")?,
        gear_type,
        specs_json: row.get("specs_json")?,
    })
}

// ---------------------------------------------------------------------------
// Database handle
// ---------------------------------------------------------------------------

pub struct Database {
    conn: Mutex<Connection>,
    path: PathBuf,
}

impl Database {
    /// Open (creating if necessary) the DB at `dir`/`gear_compat.db`, run the
    /// schema migration and seed the master gear corpus. Called on first
    /// launch and on every subsequent launch (all steps are idempotent).
    pub fn open_in_dir(dir: &Path) -> DbResult<Self> {
        std::fs::create_dir_all(dir).map_err(|e| DbError::Io(dir.to_path_buf(), e))?;
        Self::open(&dir.join(DB_FILE_NAME))
    }

    pub fn open(path: &Path) -> DbResult<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;
        schema::init_schema(&conn)?;
        schema::seed(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
            path: path.to_path_buf(),
        })
    }

    pub fn open_in_memory() -> DbResult<Self> {
        let conn = Connection::open_in_memory()?;
        schema::init_schema(&conn)?;
        schema::seed(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
            path: PathBuf::from(":memory:"),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn lock(&self) -> DbResult<std::sync::MutexGuard<'_, Connection>> {
        self.conn.lock().map_err(|_| DbError::Poisoned)
    }

    // -- camera bodies ------------------------------------------------------

    pub fn all_bodies(&self) -> DbResult<Vec<CameraBody>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, model_name, marketing_name, sensor_type, base_megapixels,
                    aps_crop_megapixels, has_ibis, release_year
             FROM camera_bodies
             ORDER BY release_year DESC, marketing_name ASC",
        )?;
        let rows = stmt.query_map([], body_from_row)?;
        let mut out = Vec::with_capacity(rows.size_hint().0);
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn body_by_id(&self, id: i64) -> DbResult<Option<CameraBody>> {
        let conn = self.lock()?;
        Ok(conn
            .query_row(
                "SELECT id, model_name, marketing_name, sensor_type, base_megapixels,
                        aps_crop_megapixels, has_ibis, release_year
                 FROM camera_bodies WHERE id = ?1",
                params![id],
                body_from_row,
            )
            .optional()?)
    }

    // -- lenses -------------------------------------------------------------

    pub fn all_lenses(&self) -> DbResult<Vec<Lens>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, model_name, marketing_name, mount_designation, target_sensor,
                    focal_length_min, focal_length_max, max_aperture_min, has_oss, weight_grams
             FROM lenses
             ORDER BY focal_length_min ASC, focal_length_max ASC",
        )?;
        let rows = stmt.query_map([], lens_from_row)?;
        let mut out = Vec::with_capacity(rows.size_hint().0);
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn lens_by_id(&self, id: i64) -> DbResult<Option<Lens>> {
        let conn = self.lock()?;
        Ok(conn
            .query_row(
                "SELECT id, model_name, marketing_name, mount_designation, target_sensor,
                        focal_length_min, focal_length_max, max_aperture_min, has_oss, weight_grams
                 FROM lenses WHERE id = ?1",
                params![id],
                lens_from_row,
            )
            .optional()?)
    }

    // -- flashes ------------------------------------------------------------

    pub fn all_flashes(&self) -> DbResult<Vec<Flash>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, model_name, guide_number, trigger_voltage, ttl_compatible
             FROM flashes ORDER BY guide_number DESC",
        )?;
        let rows = stmt.query_map([], flash_from_row)?;
        let mut out = Vec::with_capacity(rows.size_hint().0);
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    // -- custom gear (user-defined) ----------------------------------------

    pub fn all_custom_gear(&self) -> DbResult<Vec<CustomGear>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, user_name, gear_type, specs_json
             FROM custom_gear ORDER BY gear_type ASC, user_name ASC",
        )?;
        let rows = stmt.query_map([], gear_from_row)?;
        let mut out = Vec::with_capacity(rows.size_hint().0);
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Insert a custom gear item, validating that `specs_json` parses as JSON
    /// before touching the DB. Returns the new row id.
    pub fn insert_custom_gear(&self, item: &CustomGear) -> DbResult<i64> {
        serde_json::from_str::<serde_json::Value>(&item.specs_json)?;
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO custom_gear (user_name, gear_type, specs_json) VALUES (?1, ?2, ?3)",
            params![item.user_name, item.gear_type.as_str(), item.specs_json],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn update_custom_gear(&self, item: &CustomGear) -> DbResult<()> {
        serde_json::from_str::<serde_json::Value>(&item.specs_json)?;
        let conn = self.lock()?;
        conn.execute(
            "UPDATE custom_gear SET user_name = ?1, gear_type = ?2, specs_json = ?3
             WHERE id = ?4",
            params![item.user_name, item.gear_type.as_str(), item.specs_json, item.id],
        )?;
        Ok(())
    }

    pub fn delete_custom_gear(&self, id: i64) -> DbResult<bool> {
        let conn = self.lock()?;
        Ok(conn.execute("DELETE FROM custom_gear WHERE id = ?1", params![id])? > 0)
    }

    /// Counts per table, shown in the Settings pane.
    pub fn table_counts(&self) -> DbResult<(i64, i64, i64, i64)> {
        let conn = self.lock()?;
        let one = |sql: &str| -> DbResult<i64> {
            Ok(conn.query_row(sql, [], |r| r.get::<_, i64>(0))?)
        };
        Ok((
            one("SELECT COUNT(*) FROM camera_bodies")?,
            one("SELECT COUNT(*) FROM lenses")?,
            one("SELECT COUNT(*) FROM flashes")?,
            one("SELECT COUNT(*) FROM custom_gear")?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::GearType;
    use serde_json::json;

    #[test]
    fn crud_roundtrip_for_custom_gear() {
        let db = Database::open_in_memory().unwrap();

        let item = CustomGear::new(
            "My TOUGH G128",
            GearType::SdCard,
            &json!({"read_speed": 300, "write_speed": 260, "capacity_gb": 128}),
        );
        let id = db.insert_custom_gear(&item).unwrap();
        assert!(id > 0);

        let all = db.all_custom_gear().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].user_name, "My TOUGH G128");
        assert_eq!(all[0].gear_type, GearType::SdCard);
        let specs = all[0].parsed_specs();
        assert_eq!(specs["read_speed"], 300);
        assert_eq!(specs["capacity_gb"], 128);

        let mut edited = all[0].clone();
        edited.id = id;
        edited.user_name = "TOUGH G128 v2".into();
        edited.specs_json = json!({"read_speed": 275, "write_speed": 260, "capacity_gb": 128})
            .to_string();
        db.update_custom_gear(&edited).unwrap();
        assert_eq!(db.all_custom_gear().unwrap()[0].user_name, "TOUGH G128 v2");

        assert!(db.delete_custom_gear(id).unwrap());
        assert!(db.all_custom_gear().unwrap().is_empty());
    }

    #[test]
    fn invalid_specs_json_is_rejected() {
        let db = Database::open_in_memory().unwrap();
        let bad = CustomGear {
            id: -1,
            user_name: "broken".into(),
            gear_type: GearType::Battery,
            specs_json: "{not json".into(),
        };
        assert!(matches!(db.insert_custom_gear(&bad), Err(DbError::Json(_))));
    }

    #[test]
    fn seed_queries_work() {
        let db = Database::open_in_memory().unwrap();
        assert_eq!(db.all_bodies().unwrap().len(), schema::seed_camera_bodies().len());
        assert_eq!(db.all_lenses().unwrap().len(), schema::seed_lenses().len());
        assert_eq!(db.all_flashes().unwrap().len(), schema::seed_flashes().len());
        let a7iv = db.body_by_id(2).unwrap().unwrap();
        assert_eq!(a7iv.marketing_name, "Alpha 7 IV");
        assert_eq!(a7iv.sensor_type, SensorType::FullFrame);
        let (b, l, f, c) = db.table_counts().unwrap();
        assert_eq!((b, l, f, c), (8, 8, 5, 0));
    }
}
