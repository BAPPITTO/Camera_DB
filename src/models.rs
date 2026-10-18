//! Domain model structs mirroring the SQLite schema exactly.
//!
//! Column <-> field mapping is documented per struct so `db/schema.rs` and
//! `db/mod.rs` row-mappers stay in sync with the DDL.

use serde::{Deserialize, Serialize};

/// `sensor_type` / `target_sensor` enum values enforced by SQL CHECK
/// constraints (`CHECK(sensor_type IN ('APS_C','FULL_FRAME'))`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SensorType {
    #[serde(rename = "APS_C")]
    ApsC,
    #[serde(rename = "FULL_FRAME")]
    FullFrame,
}

impl SensorType {
    pub fn as_str(self) -> &'static str {
        match self {
            SensorType::ApsC => "APS_C",
            SensorType::FullFrame => "FULL_FRAME",
        }
    }

    /// Parse a raw DB value; unknown values default to APS-C (safest, since
    /// an APS-C body never over-scans a full-frame image circle).
    pub fn from_db(value: &str) -> Self {
        match value {
            "FULL_FRAME" => SensorType::FullFrame,
            _ => SensorType::ApsC,
        }
    }

    pub fn crop_factor(self) -> f32 {
        match self {
            SensorType::FullFrame => 1.0,
            SensorType::ApsC => 1.5,
        }
    }

    /// Sony APS-C sensors cover ~44.4% of the full-frame image area.
    pub const APS_C_AREA_RATIO: f32 = 0.444;
}

impl std::fmt::Display for SensorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SensorType::ApsC => write!(f, "APS-C"),
            SensorType::FullFrame => write!(f, "Full Frame"),
        }
    }
}

/// Lens mount designation. `E` = APS-C-recommended optics, `FE` = full-frame
/// image circle (also mounts on E bodies).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MountDesignation {
    E,
    FE,
}

impl MountDesignation {
    pub fn as_str(self) -> &'static str {
        match self {
            MountDesignation::E => "E",
            MountDesignation::FE => "FE",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "FE" => MountDesignation::FE,
            _ => MountDesignation::E,
        }
    }
}

impl std::fmt::Display for MountDesignation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Table `camera_bodies`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraBody {
    pub id: i64,
    /// e.g. `ILCE-7M4`
    pub model_name: String,
    /// e.g. `Alpha 7 IV`
    pub marketing_name: String,
    pub sensor_type: SensorType,
    pub base_megapixels: f64,
    /// Megapixels reported when the body runs its own APS-C crop mode.
    pub aps_crop_megapixels: Option<f64>,
    pub has_ibis: bool,
    pub release_year: Option<i64>,
}

/// Table `lenses`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lens {
    pub id: i64,
    /// e.g. `SEL1655`
    pub model_name: String,
    /// e.g. `E PZ 16-50mm F3.5-5.6 OSS`
    pub marketing_name: String,
    pub mount_designation: MountDesignation,
    pub target_sensor: SensorType,
    pub focal_length_min: f64,
    pub focal_length_max: f64,
    /// Widest aperture at the wide end (T-stop for variable-aperture zooms).
    pub max_aperture_min: f64,
    pub has_oss: bool,
    pub weight_grams: Option<i64>,
}

impl Lens {
    /// True for zooms (`focal_length_min != focal_length_max`).
    pub fn is_zoom(&self) -> bool {
        (self.focal_length_max - self.focal_length_min).abs() > f64::EPSILON
    }

    pub fn focal_label(&self) -> String {
        if self.is_zoom() {
            format!("{}-{}mm", trim_num(self.focal_length_min), trim_num(self.focal_length_max))
        } else {
            format!("{}mm", trim_num(self.focal_length_min))
        }
    }
}

/// Table `flashes`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Flash {
    pub id: i64,
    /// e.g. `HVL-F46RM`
    pub model_name: String,
    /// Guide number at ISO 100 / m.
    pub guide_number: f64,
    /// Hot-shoe trigger voltage in volts. Values above 6.0 V are flagged as a
    /// risk to legacy electronics.
    pub trigger_voltage: f64,
    pub ttl_compatible: bool,
}

/// Gear categories allowed by `custom_gear.gear_type`'s CHECK constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GearType {
    #[serde(rename = "SD_CARD")]
    SdCard,
    #[serde(rename = "BATTERY")]
    Battery,
    #[serde(rename = "FILTER")]
    Filter,
}

impl GearType {
    pub const ALL: [GearType; 3] = [GearType::SdCard, GearType::Battery, GearType::Filter];

    pub fn as_str(self) -> &'static str {
        match self {
            GearType::SdCard => "SD_CARD",
            GearType::Battery => "BATTERY",
            GearType::Filter => "FILTER",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            GearType::SdCard => "SD Card",
            GearType::Battery => "Battery",
            GearType::Filter => "Filter",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "SD_CARD" => Some(GearType::SdCard),
            "BATTERY" => Some(GearType::Battery),
            "FILTER" => Some(GearType::Filter),
            _ => None,
        }
    }
}

/// Table `custom_gear`. `specs_json` holds arbitrary user JSON such as
/// `{"read_speed": 300, "write_speed": 260, "capacity_gb": 128}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomGear {
    pub id: i64,
    pub user_name: String,
    pub gear_type: GearType,
    pub specs_json: String,
}

impl CustomGear {
    /// Build a new row, validating that `specs` serializes to valid JSON.
    pub fn new(user_name: &str, gear_type: GearType, specs: &serde_json::Value) -> Self {
        Self {
            id: -1,
            user_name: user_name.to_string(),
            gear_type,
            specs_json: serde_json::to_string(specs).unwrap_or_else(|_| "{}".to_string()),
        }
    }

    pub fn parsed_specs(&self) -> serde_json::Value {
        serde_json::from_str(&self.specs_json).unwrap_or(serde_json::Value::Null)
    }

    /// Pretty-printed spec JSON for display in the UI.
    pub fn pretty_specs(&self) -> String {
        let v = self.parsed_specs();
        serde_json::to_string_pretty(&v).unwrap_or_else(|_| self.specs_json.clone())
    }
}

/// Format a focal length / MP value without trailing `.0`.
pub fn trim_num(v: f64) -> String {
    if (v - v.round()).abs() < f64::EPSILON {
        format!("{}", v.round() as i64)
    } else {
        format!("{:.1}", v)
    }
}
