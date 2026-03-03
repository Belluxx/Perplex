mod analysis;
mod colors;
mod llamacpp;
mod settings;
mod ui_main;
mod ui_settings;
mod ui_tokens;
mod worker;

use eframe::egui;

use crate::settings::Settings;
use crate::ui_main::{UnifiedColorMode, ViewMode};
use crate::worker::{WorkerCommand, WorkerManager, WorkerMessage};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ModelSlot {
    A,
    B,
}

impl ModelSlot {
    const ALL: [ModelSlot; 2] = [ModelSlot::A, ModelSlot::B];

    fn label(self) -> &'static str {
        match self {
            ModelSlot::A => "Model A",
            ModelSlot::B => "Model B",
        }
    }
}

struct PerplexApp {
    settings: Settings,
    show_settings: bool,
    settings_path_buffer_a: String,
    settings_path_buffer_b: String,
    input_text: String,
    result_a: Option<analysis::AnalysisResult>,
    result_b: Option<analysis::AnalysisResult>,
    error_message: Option<String>,
    token_count: Option<usize>,
    worker_a: WorkerManager,
    worker_b: WorkerManager,
    view_mode: ViewMode,
    unified_color_mode: UnifiedColorMode,
}

impl Default for PerplexApp {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            show_settings: false,
            settings_path_buffer_a: String::new(),
            settings_path_buffer_b: String::new(),
            input_text: String::new(),
            result_a: None,
            result_b: None,
            error_message: None,
            token_count: None,
            worker_a: WorkerManager::default(),
            worker_b: WorkerManager::default(),
            view_mode: ViewMode::Split,
            unified_color_mode: UnifiedColorMode::AvgRank,
        }
    }
}

impl PerplexApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let _ = env_logger::try_init();

        let mut app = Self::default();
        app.settings = Settings::load();

        for slot in ModelSlot::ALL {
            if let Some(path) = app.model_path_mut(slot).clone() {
                app.load_model(slot, path);
            }
        }
        app
    }

    fn model_path_mut(&mut self, slot: ModelSlot) -> &mut Option<String> {
        match slot {
            ModelSlot::A => &mut self.settings.model_path_a,
            ModelSlot::B => &mut self.settings.model_path_b,
        }
    }

    fn result_mut(&mut self, slot: ModelSlot) -> &mut Option<analysis::AnalysisResult> {
        match slot {
            ModelSlot::A => &mut self.result_a,
            ModelSlot::B => &mut self.result_b,
        }
    }

    fn worker_mut(&mut self, slot: ModelSlot) -> &mut WorkerManager {
        match slot {
            ModelSlot::A => &mut self.worker_a,
            ModelSlot::B => &mut self.worker_b,
        }
    }

    fn settings_path_buffer_mut(&mut self, slot: ModelSlot) -> &mut String {
        match slot {
            ModelSlot::A => &mut self.settings_path_buffer_a,
            ModelSlot::B => &mut self.settings_path_buffer_b,
        }
    }

    fn select_model(&mut self, slot: ModelSlot) {
        if let Some(path) = pick_gguf_model() {
            self.load_model(slot, path);
        }
    }

    fn load_model(&mut self, slot: ModelSlot, path: String) {
        *self.model_path_mut(slot) = Some(path.clone());
        self.save_settings();
        self.error_message = None;
        *self.result_mut(slot) = None;
        self.worker_mut(slot).load_model(path);
    }

    fn clear_model(&mut self, slot: ModelSlot) {
        *self.model_path_mut(slot) = None;
        self.save_settings();
        self.worker_mut(slot).shutdown();
        *self.result_mut(slot) = None;
    }

    fn save_settings(&self) {
        if let Err(e) = self.settings.save() {
            log::warn!("Failed to save settings: {}", e);
        }
    }

    fn append_error(&mut self, msg: String) {
        if let Some(ref mut existing) = self.error_message {
            existing.push('\n');
            existing.push_str(&msg);
        } else {
            self.error_message = Some(msg);
        }
    }

    fn start_analysis(&mut self) {
        let text = self.input_text.clone();
        self.error_message = None;

        for slot in ModelSlot::ALL {
            if self.worker_mut(slot).is_ready() {
                if let Err(e) = self
                    .worker_mut(slot)
                    .send_command(WorkerCommand::Analyze(text.clone()))
                {
                    self.append_error(format!("{}: {}", slot.label(), e));
                }
            }
        }
    }

    fn process_worker_messages(&mut self) {
        let input_text = self.input_text.clone();
        for slot in ModelSlot::ALL {
            let messages = self.worker_mut(slot).poll_messages();
            for msg in messages {
                match msg {
                    WorkerMessage::ModelLoaded => {
                        log::info!("{} loaded and ready", slot.label());
                        // Auto-tokenize when primary model (A) finishes loading
                        if slot == ModelSlot::A && !input_text.is_empty() {
                            let _ = self
                                .worker_mut(slot)
                                .send_command(WorkerCommand::Tokenize(input_text.clone()));
                        }
                    }
                    WorkerMessage::TokenCount(count) => {
                        self.token_count = Some(count);
                    }
                    WorkerMessage::Completed(result) => {
                        *self.result_mut(slot) = Some(result);
                    }
                    WorkerMessage::Error(error) => {
                        self.append_error(format!("{}: {}", slot.label(), error));
                    }
                    WorkerMessage::Started | WorkerMessage::Progress { .. } => {}
                }
            }
        }
    }

    fn can_analyze(&self) -> bool {
        let any_ready = self.worker_a.is_ready() || self.worker_b.is_ready();
        !self.input_text.is_empty() && any_ready && !self.is_analyzing()
    }

    fn is_analyzing(&self) -> bool {
        self.worker_a.is_analyzing || self.worker_b.is_analyzing
    }

    fn is_loading(&self) -> bool {
        self.worker_a.is_loading || self.worker_b.is_loading
    }
}

impl eframe::App for PerplexApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_worker_messages();

        if self.is_analyzing() || self.is_loading() {
            ctx.request_repaint();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::Frame::none().inner_margin(20.0).show(ui, |ui| {
                if ui_main::render_header(
                    ui,
                    self.settings.model_path_a.as_deref(),
                    self.settings.model_path_b.as_deref(),
                    self.worker_a.is_loading,
                    self.worker_b.is_loading,
                ) {
                    self.show_settings = true;
                    for slot in ModelSlot::ALL {
                        *self.settings_path_buffer_mut(slot) =
                            self.model_path_mut(slot).clone().unwrap_or_default();
                    }
                }

                ui.add_space(12.0);

                let (clicked_a, clicked_b) = ui_main::render_model_panel(
                    ui,
                    self.settings.model_path_a.is_some(),
                    self.settings.model_path_b.is_some(),
                );
                if clicked_a {
                    self.select_model(ModelSlot::A);
                }
                if clicked_b {
                    self.select_model(ModelSlot::B);
                }

                let available = ui.available_height();
                let has_results = self.result_a.is_some() || self.result_b.is_some();
                let input_height = if has_results {
                    (available * 0.25).max(100.0)
                } else {
                    (available * 0.35).max(120.0)
                };

                let not_analyzing = !self.is_analyzing();
                if ui_main::render_text_input(
                    ui,
                    &mut self.input_text,
                    not_analyzing,
                    input_height,
                    self.token_count,
                ) {
                    if self.worker_a.is_ready() {
                        let _ = self
                            .worker_a
                            .send_command(WorkerCommand::Tokenize(self.input_text.clone()));
                    }
                }

                if ui_main::render_controls(
                    ui,
                    self.can_analyze(),
                    self.is_analyzing(),
                    self.worker_a.progress,
                    self.worker_b.progress,
                ) {
                    self.start_analysis();
                }

                if let Some(ref error) = self.error_message {
                    ui_main::render_error(ui, error);
                }

                if has_results {
                    ui_main::render_results(
                        ui,
                        self.result_a.as_ref(),
                        self.result_b.as_ref(),
                        model_name_from_path(self.settings.model_path_a.as_deref()),
                        model_name_from_path(self.settings.model_path_b.as_deref()),
                        ui.available_height(),
                        &mut self.view_mode,
                        &mut self.unified_color_mode,
                    );
                } else if !self.is_analyzing() {
                    ui_main::render_empty_state(
                        ui,
                        self.settings.model_path_a.is_some()
                            || self.settings.model_path_b.is_some(),
                    );
                }
            });
        });

        if self.show_settings {
            if let Some(action) = ui_settings::render_settings_window(
                ctx,
                &mut self.show_settings,
                &mut self.settings_path_buffer_a,
                &mut self.settings_path_buffer_b,
            ) {
                match action {
                    ui_settings::SettingsAction::Browse(slot) => {
                        if let Some(path) = pick_gguf_model() {
                            *self.settings_path_buffer_mut(slot) = path;
                        }
                    }
                    ui_settings::SettingsAction::Save => {
                        self.show_settings = false;

                        for slot in ModelSlot::ALL {
                            let path = self.settings_path_buffer_mut(slot).clone();
                            if !path.is_empty() {
                                if self.model_path_mut(slot).as_deref() != Some(&path) {
                                    self.load_model(slot, path);
                                }
                            } else {
                                self.clear_model(slot);
                            }
                        }
                    }
                    ui_settings::SettingsAction::Clear(slot) => {
                        self.settings_path_buffer_mut(slot).clear();
                    }
                }
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        for slot in ModelSlot::ALL {
            self.worker_mut(slot).shutdown();
        }
    }
}

fn pick_gguf_model() -> Option<String> {
    rfd::FileDialog::new()
        .add_filter("GGUF Model", &["gguf"])
        .set_title("Select a GGUF Model")
        .pick_file()
        .map(|p| p.to_string_lossy().to_string())
}

/// Extracts the file name from an optional model path.
pub fn model_name_from_path(path: Option<&str>) -> Option<&str> {
    path.and_then(|p| std::path::Path::new(p).file_name().and_then(|n| n.to_str()))
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 500.0])
            .with_title("Perplex"),
        ..Default::default()
    };

    eframe::run_native(
        "Perplex",
        options,
        Box::new(|cc| Ok(Box::new(PerplexApp::new(cc)))),
    )
}
