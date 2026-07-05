//! The scenario editor: collapsible groups of egui controls that mutate a [`ScenarioParams`] in place.
//! Pure UI over the plain-data spec — a `ComboBox` picks each enum variant (constructing that variant's
//! defaults on change), `DragValue`s edit its numbers. The App renders this under the preset picker.

use eframe::egui;

use crate::params::{
    Axis, BodySpec, OrientSpec, PathSpec, ScenarioParams, ScheduleSpec, TimingSpec, VibrationSpec,
};
use generate::{AtmoConfig, PhaseModelKind, UldmConfig};

fn num(ui: &mut egui::Ui, label: &str, val: &mut f64, speed: f64) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(val).speed(speed));
    });
}

fn vec3(ui: &mut egui::Ui, label: &str, a: &mut [f64; 3], speed: f64) {
    ui.horizontal(|ui| {
        ui.label(label);
        for x in a.iter_mut() {
            ui.add(egui::DragValue::new(x).speed(speed));
        }
    });
}

/// Render the whole editor. Returns nothing — it edits `p` directly; the App's Run reads the result.
pub fn scenario_editor(ui: &mut egui::Ui, p: &mut ScenarioParams) {
    body_group(ui, p);
    motion_group(ui, p);
    orient_group(ui, p);
    array_group(ui, p);
    schedule_group(ui, p);
    fields_group(ui, p);
    uldm_group(ui, p);
    noise_group(ui, p);
    advanced_group(ui, p);
}

fn body_group(ui: &mut egui::Ui, p: &mut ScenarioParams) {
    egui::CollapsingHeader::new("Body")
        .default_open(true)
        .show(ui, |ui| {
            let name = match p.body {
                BodySpec::Sphere { .. } => "Sphere",
                BodySpec::Cuboid { .. } => "Cuboid",
                BodySpec::Cylinder { .. } => "Cylinder",
                BodySpec::Shell { .. } => "Shell",
                BodySpec::Scaffold => "Scaffold",
                BodySpec::MeshDemo => "Mesh demo",
                BodySpec::MeshFile { .. } => "Mesh file",
            };
            egui::ComboBox::from_id_salt("body_kind")
                .selected_text(name)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(matches!(p.body, BodySpec::Sphere { .. }), "Sphere")
                        .clicked()
                    {
                        p.body = BodySpec::Sphere { r: 0.3 };
                    }
                    if ui
                        .selectable_label(matches!(p.body, BodySpec::Cuboid { .. }), "Cuboid")
                        .clicked()
                    {
                        p.body = BodySpec::Cuboid {
                            half: [0.3, 0.2, 0.15],
                        };
                    }
                    if ui
                        .selectable_label(matches!(p.body, BodySpec::Cylinder { .. }), "Cylinder")
                        .clicked()
                    {
                        p.body = BodySpec::Cylinder {
                            r: 0.3,
                            half_l: 0.6,
                        };
                    }
                    if ui
                        .selectable_label(matches!(p.body, BodySpec::Shell { .. }), "Shell")
                        .clicked()
                    {
                        p.body = BodySpec::Shell {
                            r: 0.4,
                            thickness: 0.1,
                        };
                    }
                    if ui
                        .selectable_label(matches!(p.body, BodySpec::Scaffold), "Scaffold")
                        .clicked()
                    {
                        p.body = BodySpec::Scaffold;
                    }
                    if ui
                        .selectable_label(matches!(p.body, BodySpec::MeshDemo), "Mesh demo")
                        .clicked()
                    {
                        p.body = BodySpec::MeshDemo;
                    }
                    if ui
                        .selectable_label(matches!(p.body, BodySpec::MeshFile { .. }), "Mesh file")
                        .clicked()
                    {
                        p.body = BodySpec::MeshFile {
                            path: String::new(),
                            scale: 1.0,
                        };
                    }
                });

            match &mut p.body {
                BodySpec::Sphere { r } => num(ui, "radius", r, 0.01),
                BodySpec::Cuboid { half } => vec3(ui, "half-extents", half, 0.01),
                BodySpec::Cylinder { r, half_l } => {
                    num(ui, "radius", r, 0.01);
                    num(ui, "half-length", half_l, 0.01);
                }
                BodySpec::Shell { r, thickness } => {
                    num(ui, "radius", r, 0.01);
                    num(ui, "thickness", thickness, 0.005);
                }
                BodySpec::Scaffold | BodySpec::MeshDemo => {
                    ui.weak("fixed geometry (edit mass / pitch below)");
                }
                BodySpec::MeshFile { path, scale } => {
                    ui.horizontal(|ui| {
                        ui.label("path");
                        ui.text_edit_singleline(path);
                    });
                    num(ui, "scale (m/unit)", scale, 0.01);
                    ui.weak("STL / OBJ / glTF; scale is mandatory");
                }
            }
            num(ui, "mass (kg)", &mut p.mass, 1.0);
            ui.horizontal(|ui| {
                ui.label("voxel pitch");
                ui.add(
                    egui::DragValue::new(&mut p.pitch)
                        .speed(0.005)
                        .range(0.02..=1.0),
                );
            });
        });
}

fn motion_group(ui: &mut egui::Ui, p: &mut ScenarioParams) {
    egui::CollapsingHeader::new("Placement & motion").show(ui, |ui| {
        vec3(ui, "placement", &mut p.placement, 0.1);

        let pname = match p.path {
            PathSpec::Static => "Static",
            PathSpec::LinearPass { .. } => "Linear pass",
            PathSpec::Oscillation { .. } => "Oscillation",
            PathSpec::Circular { .. } => "Circular",
        };
        egui::ComboBox::from_id_salt("path_kind")
            .selected_text(pname)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(matches!(p.path, PathSpec::Static), "Static")
                    .clicked()
                {
                    p.path = PathSpec::Static;
                }
                if ui
                    .selectable_label(matches!(p.path, PathSpec::LinearPass { .. }), "Linear pass")
                    .clicked()
                {
                    p.path = PathSpec::LinearPass {
                        a: [0.0, 0.0, -10.0],
                        b: [0.0, 0.0, 10.0],
                    };
                }
                if ui
                    .selectable_label(
                        matches!(p.path, PathSpec::Oscillation { .. }),
                        "Oscillation",
                    )
                    .clicked()
                {
                    p.path = PathSpec::Oscillation {
                        axis: [0.0, 0.0, 1.0],
                        amp: 1.0,
                        freq: 0.1,
                        phase: 0.0,
                    };
                }
                if ui
                    .selectable_label(matches!(p.path, PathSpec::Circular { .. }), "Circular")
                    .clicked()
                {
                    p.path = PathSpec::Circular {
                        radius: 3.0,
                        freq: 0.05,
                    };
                }
            });
        match &mut p.path {
            PathSpec::Static => {}
            PathSpec::LinearPass { a, b } => {
                vec3(ui, "from", a, 0.1);
                vec3(ui, "to", b, 0.1);
            }
            PathSpec::Oscillation {
                axis,
                amp,
                freq,
                phase,
            } => {
                vec3(ui, "axis", axis, 0.1);
                num(ui, "amplitude", amp, 0.05);
                num(ui, "frequency", freq, 0.005);
                num(ui, "phase", phase, 0.05);
            }
            PathSpec::Circular { radius, freq } => {
                num(ui, "radius", radius, 0.1);
                num(ui, "frequency", freq, 0.005);
            }
        }

        let tname = match p.timing {
            TimingSpec::Uniform { .. } => "Uniform",
            TimingSpec::Eased { .. } => "Eased",
        };
        egui::ComboBox::from_id_salt("timing_kind")
            .selected_text(tname)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(matches!(p.timing, TimingSpec::Uniform { .. }), "Uniform")
                    .clicked()
                {
                    p.timing = TimingSpec::Uniform { rate: 1.0 };
                }
                if ui
                    .selectable_label(matches!(p.timing, TimingSpec::Eased { .. }), "Eased")
                    .clicked()
                {
                    p.timing = TimingSpec::Eased {
                        rate: 1.0,
                        accel: 0.0,
                    };
                }
            });
        match &mut p.timing {
            TimingSpec::Uniform { rate } => num(ui, "rate", rate, 0.01),
            TimingSpec::Eased { rate, accel } => {
                num(ui, "rate", rate, 0.01);
                num(ui, "accel", accel, 0.01);
            }
        }
    });
}

fn orient_group(ui: &mut egui::Ui, p: &mut ScenarioParams) {
    egui::CollapsingHeader::new("Orientation").show(ui, |ui| {
        let oname = match p.orient {
            OrientSpec::Fixed => "Fixed",
            OrientSpec::FreeRotation { .. } => "Free rotation",
            OrientSpec::Libration { .. } => "Libration",
        };
        egui::ComboBox::from_id_salt("orient_kind")
            .selected_text(oname)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(matches!(p.orient, OrientSpec::Fixed), "Fixed")
                    .clicked()
                {
                    p.orient = OrientSpec::Fixed;
                }
                if ui
                    .selectable_label(
                        matches!(p.orient, OrientSpec::FreeRotation { .. }),
                        "Free rotation",
                    )
                    .clicked()
                {
                    p.orient = OrientSpec::FreeRotation {
                        omega0: [0.02, 3.0, 0.01],
                    };
                }
                if ui
                    .selectable_label(
                        matches!(p.orient, OrientSpec::Libration { .. }),
                        "Libration",
                    )
                    .clicked()
                {
                    p.orient = OrientSpec::Libration {
                        axis: [0.0, 1.0, 0.0],
                        pivot_distance: 2.0,
                        theta0: 0.6,
                        thetadot0: 0.0,
                    };
                }
            });
        match &mut p.orient {
            OrientSpec::Fixed => {}
            OrientSpec::FreeRotation { omega0 } => vec3(ui, "ω₀", omega0, 0.05),
            OrientSpec::Libration {
                axis,
                pivot_distance,
                theta0,
                thetadot0,
            } => {
                vec3(ui, "axis", axis, 0.1);
                num(ui, "pivot distance", pivot_distance, 0.05);
                num(ui, "θ₀", theta0, 0.02);
                num(ui, "θ̇₀", thetadot0, 0.02);
            }
        }
    });
}

fn array_group(ui: &mut egui::Ui, p: &mut ScenarioParams) {
    egui::CollapsingHeader::new("Detector array").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label("count");
            ui.add(egui::DragValue::new(&mut p.detectors.count).range(1..=8));
        });
        num(ui, "spacing", &mut p.detectors.spacing, 0.1);
        ui.horizontal(|ui| {
            ui.label("axis");
            ui.selectable_value(&mut p.detectors.axis, Axis::X, "X");
            ui.selectable_value(&mut p.detectors.axis, Axis::Y, "Y");
            ui.selectable_value(&mut p.detectors.axis, Axis::Z, "Z");
        });
    });
}

fn schedule_group(ui: &mut egui::Ui, p: &mut ScenarioParams) {
    egui::CollapsingHeader::new("Schedule").show(ui, |ui| {
        let sname = match p.schedule {
            ScheduleSpec::Uniform { .. } => "Uniform",
            ScheduleSpec::Gappy { .. } => "Gappy",
            ScheduleSpec::Jittered { .. } => "Jittered",
        };
        egui::ComboBox::from_id_salt("sched_kind")
            .selected_text(sname)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(
                        matches!(p.schedule, ScheduleSpec::Uniform { .. }),
                        "Uniform",
                    )
                    .clicked()
                {
                    p.schedule = ScheduleSpec::Uniform {
                        cadence: 2.0,
                        n: 64,
                    };
                }
                if ui
                    .selectable_label(matches!(p.schedule, ScheduleSpec::Gappy { .. }), "Gappy")
                    .clicked()
                {
                    p.schedule = ScheduleSpec::Gappy {
                        cadence: 1.0,
                        n: 256,
                        p_drop: 0.2,
                        seed: 11,
                    };
                }
                if ui
                    .selectable_label(
                        matches!(p.schedule, ScheduleSpec::Jittered { .. }),
                        "Jittered",
                    )
                    .clicked()
                {
                    p.schedule = ScheduleSpec::Jittered {
                        cadence: 1.0,
                        n: 256,
                        jitter: 0.3,
                        seed: 5,
                    };
                }
            });
        match &mut p.schedule {
            ScheduleSpec::Uniform { cadence, n } => {
                num(ui, "cadence", cadence, 0.05);
                usize_drag(ui, "cycles", n);
            }
            ScheduleSpec::Gappy {
                cadence,
                n,
                p_drop,
                seed,
            } => {
                num(ui, "cadence", cadence, 0.05);
                usize_drag(ui, "cycles", n);
                num(ui, "drop prob", p_drop, 0.01);
                u64_drag(ui, "seed", seed);
            }
            ScheduleSpec::Jittered {
                cadence,
                n,
                jitter,
                seed,
            } => {
                num(ui, "cadence", cadence, 0.05);
                usize_drag(ui, "cycles", n);
                num(ui, "jitter", jitter, 0.01);
                u64_drag(ui, "seed", seed);
            }
        }
        let mut contaminated = p.contamination.is_some();
        if ui
            .checkbox(&mut contaminated, "contamination mask")
            .changed()
        {
            p.contamination = contaminated.then_some((0.1, 3));
        }
        if let Some((frac, seed)) = &mut p.contamination {
            num(ui, "fraction", frac, 0.01);
            u64_drag(ui, "seed", seed);
        }
    });
}

fn fields_group(ui: &mut egui::Ui, p: &mut ScenarioParams) {
    egui::CollapsingHeader::new("Signal fields").show(ui, |ui| {
        ui.checkbox(&mut p.fields.shape, "shape descriptors");
        ui.checkbox(&mut p.fields.decomposition, "channel decomposition");
        ui.checkbox(&mut p.fields.periodogram, "periodogram");
    });
}

fn uldm_group(ui: &mut egui::Ui, p: &mut ScenarioParams) {
    egui::CollapsingHeader::new("ULDM").show(ui, |ui| {
        let mut on = p.uldm.is_some();
        if ui.checkbox(&mut on, "ULDM line").changed() {
            p.uldm = on.then_some(UldmConfig {
                amplitude: 1e-3,
                frequency: 0.1,
                phase: 0.0,
            });
        }
        if let Some(u) = &mut p.uldm {
            num(ui, "amplitude", &mut u.amplitude, 1e-4);
            num(ui, "frequency", &mut u.frequency, 0.005);
            num(ui, "phase", &mut u.phase, 0.05);
        }
    });
}

fn noise_group(ui: &mut egui::Ui, p: &mut ScenarioParams) {
    egui::CollapsingHeader::new("Noise & atmosphere").show(ui, |ui| {
        let mut shot = p.noise.shot.is_some();
        if ui.checkbox(&mut shot, "shot noise").changed() {
            p.noise.shot = shot.then_some(1e-4);
        }
        if let Some(sigma) = &mut p.noise.shot {
            num(ui, "σ (shot)", sigma, 1e-5);
        }
        let mut vib = p.noise.vibration.is_some();
        if ui.checkbox(&mut vib, "vibration residual").changed() {
            p.noise.vibration = vib.then_some(VibrationSpec {
                sigma: 1e-3,
                rho: 0.8,
                rejection: 0.1,
            });
        }
        if let Some(v) = &mut p.noise.vibration {
            num(ui, "σ (vib)", &mut v.sigma, 1e-4);
            num(ui, "ρ (colour)", &mut v.rho, 0.01);
            num(ui, "rejection", &mut v.rejection, 0.01);
        }
        let mut atmo = p.atmo.is_some();
        if ui.checkbox(&mut atmo, "atmospheric GGN").changed() {
            p.atmo = atmo.then_some(AtmoConfig {
                n_modes: 16,
                correlation_length: 50.0,
                amplitude: 1.0,
                sound_speed: 343.0,
            });
        }
        if let Some(a) = &mut p.atmo {
            ui.horizontal(|ui| {
                ui.label("modes");
                ui.add(egui::DragValue::new(&mut a.n_modes).range(1..=128));
            });
            num(ui, "corr length", &mut a.correlation_length, 1.0);
            num(ui, "amplitude", &mut a.amplitude, 0.1);
        }
    });
}

fn advanced_group(ui: &mut egui::Ui, p: &mut ScenarioParams) {
    egui::CollapsingHeader::new("Advanced").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label("phase model");
            ui.selectable_value(
                &mut p.phase_model,
                PhaseModelKind::PropagationIntegral,
                "integral",
            );
            ui.selectable_value(
                &mut p.phase_model,
                PhaseModelKind::QuasiStatic,
                "quasi-static",
            );
        });
        num(ui, "fine dt", &mut p.fine_dt, 0.001);
        u64_drag(ui, "seed", &mut p.seed);
    });
}

fn usize_drag(ui: &mut egui::Ui, label: &str, val: &mut usize) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(val).range(1..=4096));
    });
}

fn u64_drag(ui: &mut egui::Ui, label: &str, val: &mut u64) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(val));
    });
}
