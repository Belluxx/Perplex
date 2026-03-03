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
    settings_path_buffer_a: String,
    settings_path_buffer_b: String,
    input_text: String,
    result_a: Option<analysis::AnalysisResult>,
    result_b: Option<analysis::AnalysisResult>,
    error_message: Option<String>,
    token_count: Option<usize>,
    worker_a: WorkerManager,
    worker_b: WorkerManager,
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
        }
    }
}

impl PerplexApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let _ = env_logger::try_init();

        let mut app = Self::default();
        app.settings = Settings::load();

        if let Some(path) = app.settings.model_path_a.clone() {
            app.load_model_a(path);
        }
        if let Some(path) = app.settings.model_path_b.clone() {
            app.load_model_b(path);
        }
        app
    }

    fn select_model_a(&mut self) {
        if let Some(path) = pick_gguf_model() {
            self.load_model_a(path);
        }
    }

    fn select_model_b(&mut self) {
        if let Some(path) = pick_gguf_model() {
            self.load_model_b(path);
        }
    }

    fn load_model_a(&mut self, path: String) {
        self.settings.model_path_a = Some(path.clone());
        self.save_settings();
        self.error_message = None;
        self.result_a = None;
        self.worker_a.load_model(path);
    }

    fn load_model_b(&mut self, path: String) {
        self.settings.model_path_b = Some(path.clone());
        self.save_settings();
        self.error_message = None;
        self.result_b = None;
        self.worker_b.load_model(path);
    }

    fn clear_model_a(&mut self) {
        self.settings.model_path_a = None;
        self.save_settings();
        self.worker_a.shutdown();
        self.result_a = None;
    }

    fn clear_model_b(&mut self) {
        self.settings.model_path_b = None;
        self.save_settings();
        self.worker_b.shutdown();
        self.result_b = None;
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

        if self.worker_a.is_ready() {
            if let Err(e) = self
                .worker_a
                .send_command(WorkerCommand::Analyze(text.clone()))
            {
                self.append_error(format!("Model A: {}", e));
            }
        }
        if self.worker_b.is_ready() {
            if let Err(e) = self.worker_b.send_command(WorkerCommand::Analyze(text)) {
                self.append_error(format!("Model B: {}", e));
            }
        }
    }

    fn process_worker_messages(&mut self) {
        for msg in self.worker_a.poll_messages() {
            match msg {
                WorkerMessage::ModelLoaded => {
                    log::info!("Model A loaded and ready");
                    if !self.input_text.is_empty() {
                        let _ = self
                            .worker_a
                            .send_command(WorkerCommand::Tokenize(self.input_text.clone()));
                    }
                }
                WorkerMessage::TokenCount(count) => {
                    self.token_count = Some(count);
                }
                WorkerMessage::Completed(result) => {
                    self.result_a = Some(result);
                }
                WorkerMessage::Error(error) => {
                    self.append_error(format!("Model A: {}", error));
                }
                WorkerMessage::Started | WorkerMessage::Progress { .. } => {}
            }
        }

        for msg in self.worker_b.poll_messages() {
            match msg {
                WorkerMessage::ModelLoaded => {
                    log::info!("Model B loaded and ready");
                }
                WorkerMessage::Completed(result) => {
                    self.result_b = Some(result);
                }
                WorkerMessage::Error(error) => {
                    self.append_error(format!("Model B: {}", error));
                }
                WorkerMessage::TokenCount(_)
                | WorkerMessage::Started
                | WorkerMessage::Progress { .. } => {}
            }
        }
    }

    fn can_analyze(&self) -> bool {
        let any_ready = self.worker_a.is_ready() || self.worker_b.is_ready();
        let any_analyzing = self.worker_a.is_analyzing || self.worker_b.is_analyzing;
        any_ready && !self.input_text.is_empty() && !any_analyzing
    }

    fn is_analyzing(&self) -> bool {
        self.worker_a.is_analyzing || self.worker_b.is_analyzing
    }

    fn is_loading(&self) -> bool {
        self.worker_a.is_loading || self.worker_b.is_loading
    }

    fn model_name(path: Option<&str>) -> Option<&str> {
        path.and_then(|p| std::path::Path::new(p).file_name().and_then(|n| n.to_str()))
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
                    self.settings_path_buffer_a =
                        self.settings.model_path_a.clone().unwrap_or_default();
                    self.settings_path_buffer_b =
                        self.settings.model_path_b.clone().unwrap_or_default();
                }

                ui.add_space(12.0);

                let (clicked_a, clicked_b) = ui_main::render_model_panel(
                    ui,
                    self.settings.model_path_a.is_some(),
                    self.settings.model_path_b.is_some(),
                );
                if clicked_a {
                    self.select_model_a();
                }
                if clicked_b {
                    self.select_model_b();
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
                        Self::model_name(self.settings.model_path_a.as_deref()),
                        Self::model_name(self.settings.model_path_b.as_deref()),
                        ui.available_height(),
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
                    ui_settings::SettingsAction::BrowseA => {
                        if let Some(path) = pick_gguf_model() {
                            self.settings_path_buffer_a = path;
                        }
                    }
                    ui_settings::SettingsAction::BrowseB => {
                        if let Some(path) = pick_gguf_model() {
                            self.settings_path_buffer_b = path;
                        }
                    }
                    ui_settings::SettingsAction::Save => {
                        let path_a = self.settings_path_buffer_a.clone();
                        let path_b = self.settings_path_buffer_b.clone();

                        if !path_a.is_empty() {
                            if self.settings.model_path_a.as_deref() != Some(&path_a) {
                                self.load_model_a(path_a);
                            }
                        } else {
                            self.clear_model_a();
                        }

                        if !path_b.is_empty() {
                            if self.settings.model_path_b.as_deref() != Some(&path_b) {
                                self.load_model_b(path_b);
                            }
                        } else {
                            self.clear_model_b();
                        }
                    }
                    ui_settings::SettingsAction::ClearA => {
                        self.settings_path_buffer_a.clear();
                    }
                    ui_settings::SettingsAction::ClearB => {
                        self.settings_path_buffer_b.clear();
                    }
                }
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.worker_a.shutdown();
        self.worker_b.shutdown();
    }
}

fn pick_gguf_model() -> Option<String> {
    rfd::FileDialog::new()
        .add_filter("GGUF Model", &["gguf"])
        .set_title("Select a GGUF Model")
        .pick_file()
        .map(|p| p.to_string_lossy().to_string())
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
