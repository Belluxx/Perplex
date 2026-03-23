use egui::RichText;

use crate::settings::PreloadMode;
use crate::ModelSlot;

pub enum SettingsAction {
    Browse(ModelSlot),
    Save,
    Clear(ModelSlot),
}

pub fn render_settings_window(
    ctx: &egui::Context,
    open: &mut bool,
    path_buffer_a: &mut String,
    path_buffer_b: &mut String,
    preload_mode: &mut PreloadMode,
) -> Option<SettingsAction> {
    let mut action = None;

    egui::Window::new("Settings")
        .open(open)
        .min_size([400.0, 280.0])
        .show(ctx, |ui| {
            ui.heading("Model Configuration");
            ui.add_space(10.0);

            render_model_group(ui, "Model A", path_buffer_a, &mut action, ModelSlot::A);

            ui.add_space(8.0);

            render_model_group(ui, "Model B", path_buffer_b, &mut action, ModelSlot::B);

            ui.add_space(12.0);

            ui.heading("Loading Mode");
            ui.add_space(6.0);

            egui::ComboBox::from_id_salt("preload_mode")
                .selected_text(preload_mode.to_string())
                .width(280.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        preload_mode,
                        PreloadMode::PreloadAll,
                        PreloadMode::PreloadAll.to_string(),
                    );
                    ui.selectable_value(
                        preload_mode,
                        PreloadMode::PreloadSingle,
                        PreloadMode::PreloadSingle.to_string(),
                    );
                    ui.selectable_value(
                        preload_mode,
                        PreloadMode::NoPreload,
                        PreloadMode::NoPreload.to_string(),
                    );
                });

            ui.add_space(12.0);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("💾 Save").clicked() {
                    action = Some(SettingsAction::Save);
                }
            });
        });

    action
}

fn render_model_group(
    ui: &mut egui::Ui,
    label: &str,
    path_buffer: &mut String,
    action: &mut Option<SettingsAction>,
    slot: ModelSlot,
) {
    ui.group(|ui| {
        ui.label(RichText::new(label).strong());
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(path_buffer)
                    .hint_text("Path to .gguf model file")
                    .desired_width(f32::INFINITY),
            );
        });

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            if ui.button("📂 Browse…").clicked() {
                *action = Some(SettingsAction::Browse(slot));
            }
            if !path_buffer.is_empty() && ui.button("❌ Clear").clicked() {
                *action = Some(SettingsAction::Clear(slot));
            }
        });
    });
}
