use egui::RichText;

#[derive(PartialEq)]
pub enum SettingsAction {
    Browse,
    Save,
    Clear,
}

pub fn render_settings_window(
    ctx: &egui::Context,
    open: &mut bool,
    path_buffer: &mut String,
) -> Option<SettingsAction> {
    let mut action = None;
    egui::Window::new("Settings")
        .open(open)
        .min_size([350.0, 200.0])
        .show(ctx, |ui| {
            ui.heading("General Settings");
            ui.add_space(10.0);

            ui.group(|ui| {
                ui.label(RichText::new("Model Configuration").strong());
                ui.add_space(8.0);

                ui.label("Model Path:");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(path_buffer)
                            .hint_text("Path to .gguf model file")
                            .desired_width(f32::INFINITY),
                    );
                });

                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if ui.button("📂 Browse...").clicked() {
                        action = Some(SettingsAction::Browse);
                    }

                    if !path_buffer.is_empty() {
                        if ui.button("❌ Clear").clicked() {
                            action = Some(SettingsAction::Clear);
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("💾 Save").clicked() {
                            action = Some(SettingsAction::Save);
                        }
                    });
                });
            });
        });
    action
}
