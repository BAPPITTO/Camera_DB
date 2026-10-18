//! Pure business logic for body/lens/flash compatibility evaluation.
//!
//! No UI, no database access - everything here is deterministic and unit
//! tested. The UI layer feeds structs loaded from `db` into these functions.

use crate::models::{trim_num, CameraBody, Flash, Lens, SensorType};

/// Hot-shoe trigger voltage above which we warn about damage risk to the
/// camera's front-circuit electronics.
pub const TRIGGER_VOLTAGE_LIMIT_V: f64 = 6.0;

/// Sony APS-C crop factor used for effective focal length math.
pub const CROP_FACTOR: f64 = 1.5;

/// Fraction of the full-frame image circle actually used by an APS-C sensor.
pub const APS_C_AREA_RATIO: f64 = SensorType::APS_C_AREA_RATIO as f64;

/// Result status strings, kept in sync with [`CompatibilityStatus::as_str`].
#[allow(dead_code)]
pub const STATUS_NATIVE_FULL_FRAME: &str = "NATIVE_FULL_FRAME";
#[allow(dead_code)]
pub const STATUS_NATIVE_APS_C: &str = "NATIVE_APS_C";
#[allow(dead_code)]
pub const STATUS_NATIVE_CROP: &str = "NATIVE_CROP";
#[allow(dead_code)]
pub const STATUS_CROP_MODE: &str = "CROP_MODE";
#[allow(dead_code)]
pub const STATUS_HIGH_VOLTAGE_RISK: &str = "HIGH_VOLTAGE_RISK";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatibilityStatus {
    /// Full-frame lens on a full-frame body: native coverage, no caveats.
    NativeFullFrame,
    /// APS-C lens on an APS-C body: native coverage.
    NativeApsC,
    /// Full-frame lens on an APS-C body: usable natively (the body simply uses
    /// the centre of the image circle) with the standard 1.5x equivalent.
    NativeCrop,
    /// APS-C ("E") lens mounted on a full-frame body: the body must engage its
    /// crop mode or the frame will show heavy vignetting/black corners.
    CropMode,
    /// Unreachable for valid schema data; kept so callers can pattern-match
    /// exhaustively if they construct models manually.
    UNKNOWN,
}

impl CompatibilityStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            CompatibilityStatus::NativeFullFrame => "NATIVE_FULL_FRAME",
            CompatibilityStatus::NativeApsC => "NATIVE_APS_C",
            CompatibilityStatus::NativeCrop => "NATIVE_CROP",
            CompatibilityStatus::CropMode => "CROP_MODE",
            CompatibilityStatus::UNKNOWN => "UNKNOWN",
        }
    }

    pub fn is_optimal(self) -> bool {
        matches!(
            self,
            CompatibilityStatus::NativeFullFrame | CompatibilityStatus::NativeApsC
        )
    }

    pub fn label(self) -> &'static str {
        match self {
            CompatibilityStatus::NativeFullFrame => "Native full-frame",
            CompatibilityStatus::NativeApsC => "Native APS-C",
            CompatibilityStatus::NativeCrop => "Native crop (FE glass on APS-C body)",
            CompatibilityStatus::CropMode => "Crop mode required",
            CompatibilityStatus::UNKNOWN => "Unknown pairing",
        }
    }
}

/// Result of evaluating one body + one lens pairing.
#[derive(Debug, Clone, PartialEq)]
pub struct CompatibilityResult {
    pub status: CompatibilityStatus,
    /// 35mm-equivalent focal length at the wide end.
    pub effective_focal_length_min: f64,
    /// 35mm-equivalent focal length at the tele end.
    pub effective_focal_length_max: f64,
    /// Resolution actually delivered by this combination.
    pub effective_megapixels: f64,
    /// True when the combination forces a resolution penalty.
    pub resolution_penalty: bool,
    /// Human-readable explanation lines for the UI.
    pub notes: Vec<String>,
}

impl CompatibilityResult {
    /// Single-focal (prime) convenience accessor matching the spec wording.
    pub fn effective_focal_length(&self) -> f64 {
        self.effective_focal_length_min
    }

    pub fn focal_range_label(&self) -> String {
        if (self.effective_focal_length_max - self.effective_focal_length_min).abs()
            > f64::EPSILON
        {
            format!(
                "{}-{} mm eqv.",
                trim_num(self.effective_focal_length_min),
                trim_num(self.effective_focal_length_max)
            )
        } else {
            format!("{} mm eqv.", trim_num(self.effective_focal_length_min))
        }
    }
}

/// Evaluate a body/lens pairing per the documented rules:
///
/// * FF body + APS-C lens  -> `CROP_MODE`, focal x1.5, MP x0.444.
/// * APS-C body + FF lens  -> `NATIVE_CROP`, focal x1.5.
/// * matched sensors       -> native, focal x1.0.
pub fn calculate_compatibility(body: &CameraBody, lens: &Lens) -> CompatibilityResult {
    let mut notes = Vec::new();
    let (status, crop_factor, effective_megapixels, resolution_penalty) =
        match (body.sensor_type, lens.target_sensor) {
            (SensorType::FullFrame, SensorType::ApsC) => {
                notes.push(format!(
                    "'{}' projects an APS-C image circle onto a {} sensor: enable Super Bit Rate / crop mode or expect severe corner vignetting.",
                    lens.marketing_name, body.marketing_name
                ));
                notes.push(format!(
                    "Resolution drops to ~{:.1}% of the native {} MP sensor.",
                    APS_C_AREA_RATIO * 100.0,
                    trim_num(body.base_megapixels)
                ));
                (
                    CompatibilityStatus::CropMode,
                    CROP_FACTOR,
                    body.base_megapixels * APS_C_AREA_RATIO,
                    true,
                )
            }
            (SensorType::ApsC, SensorType::FullFrame) => {
                notes.push(format!(
                    "'{}' covers the '{}' sensor natively; use the centre of the image circle with a {:.1}x equivalent field of view.",
                    lens.marketing_name, body.marketing_name, CROP_FACTOR
                ));
                (
                    CompatibilityStatus::NativeCrop,
                    CROP_FACTOR,
                    body.base_megapixels,
                    false,
                )
            }
            (SensorType::FullFrame, SensorType::FullFrame) => {
                notes.push("Full-frame coverage across the whole sensor.".to_string());
                (
                    CompatibilityStatus::NativeFullFrame,
                    1.0,
                    body.base_megapixels,
                    false,
                )
            }
            (SensorType::ApsC, SensorType::ApsC) => {
                notes.push("Native APS-C pairing.".to_string());
                (CompatibilityStatus::NativeApsC, 1.0, body.base_megapixels, false)
            }
        };

    if let Some(crop_mp) = body.aps_crop_megapixels {
        if status == CompatibilityStatus::CropMode {
            notes.push(format!(
                "Manufacturer spec for this crop mode is {} MP; the {:.2}x-area estimate gives {:.1} MP.",
                trim_num(crop_mp),
                APS_C_AREA_RATIO,
                effective_megapixels
            ));
        }
    }

    if lens.has_oss && !body.has_ibis {
        notes.push(
            "Optical Stabilisation comes from the lens (no IBIS in this body).".to_string(),
        );
    } else if !lens.has_oss && body.has_ibis {
        notes.push(
            "Stabilisation is handled by the body's IBIS (lens has no OSS).".to_string(),
        );
    } else if !lens.has_oss && !body.has_ibis {
        notes.push(
            "No stabilisation available: mind the 1/videoframe rule for handheld video."
                .to_string(),
        );
    }

    if lens.mount_designation == crate::models::MountDesignation::E
        && body.sensor_type == SensorType::FullFrame
    {
        notes.push(
            "Mount designation 'E' indicates APS-C-recommended optics on an FE-capable body."
                .to_string(),
        );
    }

    CompatibilityResult {
        status,
        effective_focal_length_min: lens.focal_length_min * crop_factor,
        effective_focal_length_max: lens.focal_length_max * crop_factor,
        effective_megapixels,
        resolution_penalty,
        notes,
    }
}

// ---------------------------------------------------------------------------
// Flash safety
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlashSafetyStatus {
    Safe,
    HighVoltageRisk,
}

impl FlashSafetyStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            FlashSafetyStatus::Safe => "SAFE",
            FlashSafetyStatus::HighVoltageRisk => "HIGH_VOLTAGE_RISK",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlashSafetyResult {
    pub status: FlashSafetyStatus,
    pub trigger_voltage: f64,
    pub ttl_compatible: bool,
    pub message: String,
}

/// Trigger-voltage safety check: anything over [`TRIGGER_VOLTAGE_LIMIT_V`] is
/// reported as a critical warning (`HIGH_VOLTAGE_RISK`).
pub fn evaluate_flash(flash: &Flash) -> FlashSafetyResult {
    if flash.trigger_voltage > TRIGGER_VOLTAGE_LIMIT_V {
        FlashSafetyResult {
            status: FlashSafetyStatus::HighVoltageRisk,
            trigger_voltage: flash.trigger_voltage,
            ttl_compatible: flash.ttl_compatible,
            message: format!(
                "CRITICAL: '{}' applies {:.1} V to the hot shoe (> {:.1} V limit). Modern Sony bodies expect low-voltage shoes; use a voltage guard / ISO adapter or do not mount this unit.",
                flash.model_name, flash.trigger_voltage, TRIGGER_VOLTAGE_LIMIT_V
            ),
        }
    } else {
        FlashSafetyResult {
            status: FlashSafetyStatus::Safe,
            trigger_voltage: flash.trigger_voltage,
            ttl_compatible: flash.ttl_compatible,
            message: format!(
                "'{}' is within the safe trigger-voltage envelope ({:.1} V <= {:.1} V).{}",
                flash.model_name,
                flash.trigger_voltage,
                TRIGGER_VOLTAGE_LIMIT_V,
                if flash.ttl_compatible {
                    " TTL metering supported."
                } else {
                    " Manual/off-camera flash only."
                }
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{MountDesignation, SensorType};

    fn ff_body(mp: f64) -> CameraBody {
        CameraBody {
            id: 1,
            model_name: "ILCE-7M3".into(),
            marketing_name: "Alpha 7 III".into(),
            sensor_type: SensorType::FullFrame,
            base_megapixels: mp,
            aps_crop_megapixels: Some(10.0),
            has_ibis: true,
            release_year: Some(2018),
        }
    }

    fn crops_body() -> CameraBody {
        CameraBody {
            id: 5,
            model_name: "ILCE-6700".into(),
            marketing_name: "Alpha 6700".into(),
            sensor_type: SensorType::ApsC,
            base_megapixels: 26.0,
            aps_crop_megapixels: None,
            has_ibis: true,
            release_year: Some(2022),
        }
    }

    fn e_lens(min: f64, max: f64) -> Lens {
        Lens {
            id: 1,
            model_name: "SEL1655".into(),
            marketing_name: "E PZ 16-50mm F3.5-5.6 OSS".into(),
            mount_designation: MountDesignation::E,
            target_sensor: SensorType::ApsC,
            focal_length_min: min,
            focal_length_max: max,
            max_aperture_min: 3.5,
            has_oss: true,
            weight_grams: Some(116),
        }
    }

    fn fe_lens(min: f64, max: f64) -> Lens {
        Lens {
            id: 7,
            model_name: "SEL50F18F".into(),
            marketing_name: "FE 50mm F1.8".into(),
            mount_designation: MountDesignation::FE,
            target_sensor: SensorType::FullFrame,
            focal_length_min: min,
            focal_length_max: max,
            max_aperture_min: 1.8,
            has_oss: false,
            weight_grams: Some(186),
        }
    }

    #[test]
    fn ff_body_with_aps_c_lens_is_crop_mode() {
        let r = calculate_compatibility(&ff_body(24.2), &e_lens(16.0, 50.0));
        assert_eq!(r.status, CompatibilityStatus::CropMode);
        // focal * 1.5
        assert!((r.effective_focal_length_min - 24.0).abs() < 1e-9);
        assert!((r.effective_focal_length_max - 75.0).abs() < 1e-9);
        // megapixels * 0.444
        assert!((r.effective_megapixels - 24.2 * 0.444).abs() < 1e-9);
        assert!(r.resolution_penalty);
    }

    #[test]
    fn aps_c_body_with_fe_lens_is_native_crop() {
        let r = calculate_compatibility(&crops_body(), &fe_lens(50.0, 50.0));
        assert_eq!(r.status, CompatibilityStatus::NativeCrop);
        assert!((r.effective_focal_length() - 75.0).abs() < 1e-9);
        assert!((r.effective_megapixels - 26.0).abs() < 1e-9);
        assert!(!r.resolution_penalty);
    }

    #[test]
    fn matched_sensors_are_native() {
        let a = calculate_compatibility(&ff_body(33.0), &fe_lens(24.0, 70.0));
        assert_eq!(a.status, CompatibilityStatus::NativeFullFrame);
        assert!((a.effective_focal_length_min - 24.0).abs() < 1e-9);

        let b = calculate_compatibility(&crops_body(), &e_lens(16.0, 50.0));
        assert_eq!(b.status, CompatibilityStatus::NativeApsC);
        assert!((b.effective_focal_length_min - 16.0).abs() < 1e-9);
    }

    #[test]
    fn high_trigger_voltage_triggers_critical_warning() {
        let risky = Flash {
            id: 4,
            model_name: "Metronix 56 (legacy)".into(),
            guide_number: 56.0,
            trigger_voltage: 350.0,
            ttl_compatible: false,
        };
        let res = evaluate_flash(&risky);
        assert_eq!(res.status, FlashSafetyStatus::HighVoltageRisk);
        assert!(res.message.contains("CRITICAL"));

        let ok = Flash {
            id: 1,
            model_name: "HVL-F46RM".into(),
            guide_number: 46.0,
            trigger_voltage: 5.0,
            ttl_compatible: true,
        };
        assert_eq!(evaluate_flash(&ok).status, FlashSafetyStatus::Safe);

        // exactly at the limit is still safe; strictly greater is not
        let edge = Flash { trigger_voltage: 6.0, ..ok.clone() };
        assert_eq!(evaluate_flash(&edge).status, FlashSafetyStatus::Safe);
        let over = Flash { trigger_voltage: 6.01, ..ok } ;
        assert_eq!(evaluate_flash(&over).status, FlashSafetyStatus::HighVoltageRisk);
    }
}
