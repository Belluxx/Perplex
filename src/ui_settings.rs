use egui::RichText;

#[derive(PartialEq)]
pub enum SettingsAction {
    BrowseA,
    BrowseB,
    Save,
    ClearA,
    ClearB,
}

pub fn render_settings_window(
    ctx: &egui::Context,
    open: &mut bool,
    path_buffer_a: &mut String,
    path_buffer_b: &mut String,
) -> Option<SettingsAction> {
    let mut action = None;

    egui::Window::new("Settings")
        .open(open)
        .min_size([400.0, 280.0])
        .show(ctx, |ui| {
            ui.heading("Model Configuration");
            ui.add_space(10.0);

            render_model_group(
                ui,
                "Model A",
                path_buffer_a,
                &mut action,
                SettingsAction::BrowseA,
                SettingsAction::ClearA,
            );

            ui.add_space(8.0);

            render_model_group(
                ui,
                "Model B",
                path_buffer_b,
                &mut action,
                SettingsAction::BrowseB,
                SettingsAction::ClearB,
            );

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
    browse_action: SettingsAction,
    clear_action: SettingsAction,
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
                *action = Some(browse_action);
            }
            if !path_buffer.is_empty() && ui.button("❌ Clear").clicked() {
                *action = Some(clear_action);
            }
        });
    });
}
