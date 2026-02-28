mod analysis;
mod colors;
mod llamacpp;
mod settings;
mod ui_main;
mod ui_settings;
mod worker;

use eframe::egui;

use crate::settings::Settings;
use crate::worker::{WorkerCommand, WorkerManager, WorkerMessage};

struct PerplexApp {
    settings: Settings,
    show_settings: bool,
    settings_path_buffer: String,
    input_text: String,
    analysis_result: Option<analysis::AnalysisResult>,
    error_message: Option<String>,
    token_count: Option<usize>,
    worker: WorkerManager,
}

impl Default for PerplexApp {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            show_settings: false,
            settings_path_buffer: String::new(),
            input_text: String::new(),
            analysis_result: None,
            error_message: None,
            token_count: None,
            worker: WorkerManager::default(),
        }
    }
}

impl PerplexApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let _ = env_logger::try_init();

        let mut app = Self::default();
        app.settings = Settings::load();

        if let Some(path) = app.settings.model_path.clone() {
            app.load_model(path);
        }
        app
    }

    fn select_model(&mut self) {
        let file = rfd::FileDialog::new()
            .add_filter("GGUF Model", &["gguf"])
            .set_title("Select a GGUF Model")
            .pick_file();

        if let Some(path) = file {
            let path_str = path.to_string_lossy().to_string();
            self.load_model(path_str);
        }
    }

    fn load_model(&mut self, path: String) {
        self.settings.model_path = Some(path.clone());
        if let Err(e) = self.settings.save() {
            log::warn!("Failed to save settings: {}", e);
        }

        self.error_message = None;
        self.analysis_result = None;
        self.token_count = None;

        self.worker.load_model(path);
    }

    fn start_analysis(&mut self) {
        let text = self.input_text.clone();
        self.error_message = None;

        if let Err(e) = self.worker.send_command(WorkerCommand::Analyze(text)) {
            self.error_message = Some(e);
        }
    }

    fn process_worker_messages(&mut self) {
        for msg in self.worker.poll_messages() {
            match msg {
                WorkerMessage::ModelLoaded => {
                    log::info!("Model loaded and ready");
                    if !self.input_text.is_empty() {
                        let _ = self
                            .worker
                            .send_command(WorkerCommand::Tokenize(self.input_text.clone()));
                    }
                }
                WorkerMessage::TokenCount(count) => {
                    self.token_count = Some(count);
                }
                WorkerMessage::Completed(result) => {
                    self.analysis_result = Some(result);
                }
                WorkerMessage::Error(error) => {
                    self.error_message = Some(error);
                }
                // Worker-level state (is_loading, is_analyzing, progress) is
                // already updated inside WorkerManager::poll_messages.
                WorkerMessage::Started | WorkerMessage::Progress { .. } => {}
            }
        }
    }

    fn can_analyze(&self) -> bool {
        self.settings.model_path.is_some()
            && !self.input_text.is_empty()
            && self.worker.is_ready()
            && !self.worker.is_analyzing
    }
}

impl eframe::App for PerplexApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_worker_messages();

        if self.worker.is_analyzing || self.worker.is_loading {
            ctx.request_repaint();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::Frame::none().inner_margin(20.0).show(ui, |ui| {
                if ui_main::render_header(
                    ui,
                    self.settings.model_path.as_deref(),
                    self.worker.is_loading,
                ) {
                    self.show_settings = true;
                    // Initialize buffer with current path when opening
                    self.settings_path_buffer =
                        self.settings.model_path.clone().unwrap_or_default();
                }

                ui.add_space(12.0);

                if ui_main::render_model_panel(ui, self.settings.model_path.is_some()) {
                    self.select_model();
                }

                let available = ui.available_height();
                let has_results = self.analysis_result.is_some();

                let input_height = if has_results {
                    (available * 0.35).max(120.0)
                } else {
                    (available * 0.4).max(150.0)
                };

                if ui_main::render_text_input(
                    ui,
                    &mut self.input_text,
                    !self.worker.is_analyzing,
                    input_height,
                    self.token_count,
                ) {
                    let _ = self
                        .worker
                        .send_command(WorkerCommand::Tokenize(self.input_text.clone()));
                }

                if ui_main::render_controls(
                    ui,
                    self.can_analyze(),
                    self.worker.is_analyzing,
                    self.worker.progress,
                ) {
                    self.start_analysis();
                }

                if let Some(ref error) = self.error_message {
                    ui_main::render_error(ui, error);
                }

                if let Some(ref result) = self.analysis_result {
                    let results_height = ui.available_height();
                    ui_main::render_results(ui, result, results_height);
                } else if !self.worker.is_analyzing {
                    ui_main::render_empty_state(ui, self.settings.model_path.is_some());
                }
            });
        });

        if self.show_settings {
            if let Some(action) = ui_settings::render_settings_window(
                ctx,
                &mut self.show_settings,
                &mut self.settings_path_buffer,
            ) {
                match action {
                    ui_settings::SettingsAction::Browse => {
                        let file = rfd::FileDialog::new()
                            .add_filter("GGUF Model", &["gguf"])
                            .set_title("Select a GGUF Model")
                            .pick_file();

                        if let Some(path) = file {
                            self.settings_path_buffer = path.to_string_lossy().to_string();
                        }
                    }
                    ui_settings::SettingsAction::Save => {
                        if !self.settings_path_buffer.is_empty() {
                            self.load_model(self.settings_path_buffer.clone());
                        } else {
                            // Similar to clear logic but via save empty
                            self.settings.model_path = None;
                            let _ = self.settings.save();
                            self.worker.shutdown();
                            self.analysis_result = None;
                            self.token_count = None;
                        }
                    }
                    ui_settings::SettingsAction::Clear => {
                        self.settings_path_buffer.clear();
                    }
                }
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.worker.shutdown();
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 700.0])
            .with_min_inner_size([600.0, 400.0])
            .with_title("Perplex"),
        ..Default::default()
    };

    eframe::run_native(
        "Perplex",
        options,
        Box::new(|cc| Ok(Box::new(PerplexApp::new(cc)))),
    )
}
