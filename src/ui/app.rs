//! `eframe::App` implementation - the whole visible surface of the app.

use eframe::egui::{self, Color32, RichText, ScrollArea, Ui};

use crate::compatibility::{
    calculate_compatibility, evaluate_flash, CompatibilityResult, CompatibilityStatus,
    FlashSafetyStatus,
};
use crate::db::Database;
use crate::models::{trim_num, CameraBody, CustomGear, Flash, GearType, Lens, SensorType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Compatibility,
    Flashes,
    CustomGear,
    About,
}

/// Transient UI state for the "add custom gear" form.
struct NewGearForm {
    name: String,
    gear_type: GearType,
    /// Raw JSON typed by the user (validated with serde_json before insert).
    specs: String,
    error: Option<String>,
}

pub struct GearCompatApp {
    db: Database,
    bodies: Vec<CameraBody>,
    lenses: Vec<Lens>,
    flashes: Vec<Flash>,
    custom_gear: Vec<CustomGear>,

    tab: Tab,

    selected_body_id: Option<i64>,
    selected_lens_id: Option<i64>,

    new_gear: NewGearForm,
    status_line: String,
}

impl GearCompatApp {
    pub fn new(db: Database) -> Self {
        let bodies = db.all_bodies().unwrap_or_default();
        let lenses = db.all_lenses().unwrap_or_default();
        let flashes = db.all_flashes().unwrap_or_default();
        let custom_gear = db.all_custom_gear().unwrap_or_default();

        let (n_bodies, n_lenses, n_flashes) = (bodies.len(), lenses.len(), flashes.len());
        let selected_body_id = bodies.first().map(|b| b.id);
        // Default to an FE lens so the interesting crop-mode maths is visible
        // immediately after first launch.
        let selected_lens_id = lenses
            .iter()
            .find(|l| l.target_sensor == SensorType::FullFrame)
            .or(lenses.first())
            .map(|l| l.id);

        Self {
            db,
            bodies,
            lenses,
            flashes,
            custom_gear,
            tab: Tab::Compatibility,
            selected_body_id,
            selected_lens_id,
            new_gear: NewGearForm {
                name: String::new(),
                gear_type: GearType::SdCard,
                specs: "{\"read_speed\": 300, \"write_speed\": 260, \"capacity_gb\": 128}".to_string(),
                error: None,
            },
            status_line: format!(
                "{} bodies / {} lenses / {} flashes loaded from local SQLite",
                n_bodies,
                n_lenses,
                n_flashes
            ),
        }
    }

    fn reload_gear(&mut self) {
        self.custom_gear = self.db.all_custom_gear().unwrap_or_default();
    }

    // -----------------------------------------------------------------------
    // Tab: compatibility checker
    // -----------------------------------------------------------------------

    fn ui_compatibility(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label("Camera body:");
            egui::ComboBox::from_id_source("body_combo")
                .selected_text(self.body_label())
                .show_ui(ui, |ui| {
                    for b in &self.bodies {
                        ui.selectable_value(
                            &mut self.selected_body_id,
                            Some(b.id),
                            format!("{} ({})", b.marketing_name, b.sensor_type),
                        );
                    }
                });
        });

        ui.horizontal(|ui| {
            ui.label("Lens:");
            egui::ComboBox::from_id_source("lens_combo")
                .selected_text(self.lens_label())
                .show_ui(ui, |ui| {
                    for l in &self.lenses {
                        ui.selectable_value(
                            &mut self.selected_lens_id,
                            Some(l.id),
                            format!("{} [{}]", l.marketing_name, l.mount_designation),
                        );
                    }
                });
        });

        ui.separator();

        let body = self.selected_body().cloned();
        let lens = self.selected_lens().cloned();
        match (body, lens) {
            (Some(b), Some(l)) => {
                let result = calculate_compatibility(&b, &l);
                render_compat_result(ui, &result);
            }
            _ => {
                ui.colored_label(Color32::YELLOW, "Select a body and a lens to evaluate.");
            }
        }
    }

    // -----------------------------------------------------------------------
    // Tab: flash safety
    // -----------------------------------------------------------------------

    fn ui_flashes(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Hot-shoe trigger voltage safety").strong());
        ui.label("Any unit above 6.0 V is flagged HIGH_VOLTAGE_RISK.");
        ui.separator();

        let flashes = self.flashes.clone();
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for f in &flashes {
                    flash_row(ui, f);
                    ui.add_space(6.0);
                }
            });
    }

    // -----------------------------------------------------------------------
    // Tab: custom gear CRUD
    // -----------------------------------------------------------------------

    fn ui_custom_gear(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Your custom gear (stored locally as JSON)").strong());

        // List + delete ------------------------------------------------------
        let mut to_delete: Option<i64> = None;
        {
            let items = self.custom_gear.clone();
            ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                for g in &items {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&g.user_name).strong());
                            ui.label(format!("[{}]", g.gear_type.label()));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button("Delete").clicked() {
                                    to_delete = Some(g.id);
                                }
                            });
                        });
                        ui.monospace(g.pretty_specs());
                    });
                    ui.add_space(4.0);
                }
            });
        }
        if let Some(id) = to_delete {
            match self.db.delete_custom_gear(id) {
                Ok(true) => self.set_status("Custom gear entry deleted."),
                Ok(false) => self.set_status("Nothing to delete (row already gone)."),
                Err(e) => self.set_status(format!("Delete failed: {e}")),
            }
            self.reload_gear();
        }

        ui.separator();

        // Add form -------------------------------------------------------------
        let mut name_buf = std::mem::take(&mut self.new_gear.name);
        let mut specs_buf = std::mem::take(&mut self.new_gear.specs);
        let mut type_now = self.new_gear.gear_type;

        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.add_sized([220.0, 22.0], egui::TextEdit::singleline(&mut name_buf));
        });
        ui.horizontal(|ui| {
            ui.label("Type:");
            for t in GearType::ALL {
                if ui.selectable_label(type_now == t, t.label()).clicked() {
                    type_now = t;
                }
            }
        });
        ui.label("Specs JSON:");
        ui.add_sized(
            [ui.available_width().max(240.0), 64.0],
            egui::TextEdit::multiline(&mut specs_buf).monospace(),
        );

        let add_clicked = ui.button("Save to database").clicked();

        self.new_gear.name = name_buf;
        self.new_gear.specs = specs_buf;
        self.new_gear.gear_type = type_now;

        if add_clicked {
            if self.new_gear.name.trim().is_empty() {
                self.new_gear.error = Some("Name must not be empty.".to_string());
            } else {
                match serde_json::from_str::<serde_json::Value>(&self.new_gear.specs) {
                    Ok(value) => {
                        let item =
                            CustomGear::new(self.new_gear.name.trim(), self.new_gear.gear_type, &value);
                        match self.db.insert_custom_gear(&item) {
                            Ok(id) => {
                                self.set_status(format!("Saved custom gear #{id}: {}", item.user_name));
                                self.new_gear.error = None;
                                self.new_gear.name.clear();
                                self.reload_gear();
                            }
                            Err(e) => self.new_gear.error = Some(format!("{e}")),
                        }
                    }
                    Err(e) => self.new_gear.error = Some(format!("Invalid JSON: {e}")),
                }
            }
        }

        if let Some(err) = &self.new_gear.error {
            ui.colored_label(Color32::from_rgb(230, 60, 60), err.clone());
        }
    }

    // -----------------------------------------------------------------------
    // Tab: about / diagnostics
    // -----------------------------------------------------------------------

    fn ui_about(&mut self, ui: &mut Ui) {
        let (b, l, f, c) = self.db.table_counts().unwrap_or((0, 0, 0, 0));
        ui.label(RichText::new("Gear Compat").heading());
        ui.label(
            "Sony E-mount compatibility reference. 100% offline: all data lives in an \
             embedded SQLite database on this device.",
        );
        ui.add_space(8.0);
        egui::Grid::new("about_grid")
            .num_columns(2)
            .spacing([16.0, 4.0])
            .show(ui, |ui| {
                ui.label("Database path:");
                ui.monospace(self.db.path().display().to_string());
                ui.end_row();
                ui.label("camera_bodies:");
                ui.monospace(b.to_string());
                ui.end_row();
                ui.label("lenses:");
                ui.monospace(l.to_string());
                ui.end_row();
                ui.label("flashes:");
                ui.monospace(f.to_string());
                ui.end_row();
                ui.label("custom_gear:");
                ui.monospace(c.to_string());
                ui.end_row();
            });
        ui.add_space(8.0);
        ui.label(format!(
            "Rust edition 2021 • egui/eframe v{} • rusqlite bundled SQLite",
            env!("CARGO_PKG_VERSION")
        ));
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn set_status(&mut self, msg: impl Into<String>) {
        self.status_line = msg.into();
    }

    fn selected_body(&self) -> Option<&CameraBody> {
        self.bodies.iter().find(|b| Some(b.id) == self.selected_body_id)
    }

    fn selected_lens(&self) -> Option<&Lens> {
        self.lenses.iter().find(|l| Some(l.id) == self.selected_lens_id)
    }

    fn body_label(&self) -> String {
        match self.selected_body() {
            Some(b) => format!(
                "({}, {}, {} MP)",
                b.marketing_name,
                b.sensor_type,
                trim_num(b.base_megapixels)
            ),
            None => "- select -".to_string(),
        }
    }

    fn lens_label(&self) -> String {
        match self.selected_lens() {
            Some(l) => format!(
                "{} [{}] {}",
                l.marketing_name,
                l.mount_designation,
                l.focal_label()
            ),
            None => "- select -".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Stateless render helpers (free functions keep the borrow checker happy)
// ---------------------------------------------------------------------------

fn render_compat_result(ui: &mut Ui, r: &CompatibilityResult) {
    let color = match r.status {
        CompatibilityStatus::NativeFullFrame | CompatibilityStatus::NativeApsC => {
            Color32::from_rgb(80, 200, 120)
        }
        CompatibilityStatus::NativeCrop => Color32::from_rgb(90, 170, 255),
        CompatibilityStatus::CropMode => Color32::from_rgb(255, 170, 60),
        CompatibilityStatus::UNKNOWN => Color32::GRAY,
    };

    ui.label(
        RichText::new(r.status.as_str())
            .strong()
            .size(20.0)
            .color(color),
    );
    ui.label(r.status.label());

    egui::Grid::new("compat_grid")
        .num_columns(2)
        .spacing([16.0, 6.0])
        .show(ui, |ui| {
            ui.label("Effective focal length:");
            ui.monospace(r.focal_range_label());
            ui.end_row();

            ui.label("Effective resolution:");
            ui.monospace(format!("{:.1} MP", r.effective_megapixels));
            ui.end_row();

            ui.label("Resolution penalty:");
            ui.monospace(if r.resolution_penalty { "YES" } else { "no" });
            ui.end_row();
        });

    ui.add_space(8.0);
    ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
        for note in &r.notes {
            ui.label(format!("• {note}"));
        }
    });
}

fn flash_row(ui: &mut Ui, f: &Flash) {
    let safety = evaluate_flash(f);
    let (badge, color) = match safety.status {
        FlashSafetyStatus::Safe => ("OK", Color32::from_rgb(80, 200, 120)),
        FlashSafetyStatus::HighVoltageRisk => ("!!", Color32::from_rgb(230, 60, 60)),
    };

    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(badge).strong().color(color));
            ui.label(RichText::new(&f.model_name).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.monospace(format!("{:.1} V", f.trigger_voltage));
            });
        });
        ui.label(format!(
            "GN {} m @ ISO 100 • TTL: {}",
            trim_num(f.guide_number),
            if f.ttl_compatible { "yes" } else { "no" }
        ));
        if safety.status == FlashSafetyStatus::HighVoltageRisk {
            ui.colored_label(
                Color32::from_rgb(230, 60, 60),
                RichText::new(safety.message.as_str()).strong(),
            );
        } else {
            ui.label(safety.message.as_str());
        }
    });
}

impl eframe::App for GearCompatApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Gear Compat");
                ui.separator();
                if ui
                    .selectable_label(self.tab == Tab::Compatibility, "Compatibility")
                    .clicked()
                {
                    self.tab = Tab::Compatibility;
                }
                if ui.selectable_label(self.tab == Tab::Flashes, "Flashes").clicked() {
                    self.tab = Tab::Flashes;
                }
                if ui
                    .selectable_label(self.tab == Tab::CustomGear, "My Gear")
                    .clicked()
                {
                    self.tab = Tab::CustomGear;
                }
                if ui.selectable_label(self.tab == Tab::About, "About").clicked() {
                    self.tab = Tab::About;
                }
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&self.status_line).weak().size(12.0));
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| match self.tab {
            Tab::Compatibility => self.ui_compatibility(ui),
            Tab::Flashes => self.ui_flashes(ui),
            Tab::CustomGear => self.ui_custom_gear(ui),
            Tab::About => self.ui_about(ui),
        });
    }
}
