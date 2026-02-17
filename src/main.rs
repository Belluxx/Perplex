mod colors;
mod llamacpp;
mod settings;
mod ui_main;
mod ui_settings;
mod utils;

use eframe::egui;

use std::sync::mpsc;
use std::thread;

use crate::settings::Settings;
use crate::utils::{AnalysisResult, WorkerCommand, WorkerMessage};

struct PerplexApp {
    settings: Settings,
    show_settings: bool,
    settings_path_buffer: String,

    input_text: String,

    analysis_result: Option<AnalysisResult>,

    error_message: Option<String>,

    is_loading_model: bool,

    is_analyzing: bool,

    progress: Option<f32>,

    token_count: Option<usize>,

    worker_tx: Option<mpsc::Sender<WorkerCommand>>,

    worker_rx: Option<mpsc::Receiver<WorkerMessage>>,

    worker_handle: Option<thread::JoinHandle<()>>,
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
            is_loading_model: false,
            is_analyzing: false,
            progress: None,
            token_count: None,
            worker_tx: None,
            worker_rx: None,
            worker_handle: None,
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
        self.shutdown_worker();

        self.is_loading_model = true;
        self.error_message = None;
        self.analysis_result = None;

        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (msg_tx, msg_rx) = mpsc::channel();

        self.worker_tx = Some(cmd_tx);
        self.worker_rx = Some(msg_rx);

        let handle = thread::spawn(move || {
            llamacpp::run_analysis_worker(path, cmd_rx, msg_tx);
        });

        self.worker_handle = Some(handle);
    }

    fn start_analysis(&mut self) {
        if let Some(ref tx) = self.worker_tx {
            self.is_analyzing = true;
            self.progress = Some(0.0);
            self.error_message = None;

            let text = self.input_text.clone();
            if let Err(e) = tx.send(WorkerCommand::Analyze(text)) {
                self.error_message = Some(format!("Failed to send command: {}", e));
                self.is_analyzing = false;
            }
        }
    }

    fn process_worker_messages(&mut self) {
        if let Some(ref rx) = self.worker_rx {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    WorkerMessage::ModelLoaded => {
                        self.is_loading_model = false;
                        log::info!("Model loaded and ready");
                    }
                    WorkerMessage::Started => {
                        self.is_analyzing = true;
                        self.progress = Some(0.0);
                    }
                    WorkerMessage::Progress { current, total } => {
                        self.progress = Some(current as f32 / total.max(1) as f32);
                    }
                    WorkerMessage::TokenCount(count) => {
                        self.token_count = Some(count);
                    }
                    WorkerMessage::Completed(result) => {
                        self.analysis_result = Some(result);
                        self.is_analyzing = false;
                        self.progress = None;
                    }
                    WorkerMessage::Error(error) => {
                        self.error_message = Some(error);
                        self.is_analyzing = false;
                        self.is_loading_model = false;
                        self.progress = None;
                    }
                }
            }
        }
    }

    fn shutdown_worker(&mut self) {
        if let Some(tx) = self.worker_tx.take() {
            let _ = tx.send(WorkerCommand::Shutdown);
        }
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
        self.worker_rx = None;
    }

    fn can_analyze(&self) -> bool {
        self.settings.model_path.is_some()
            && !self.input_text.is_empty()
            && !self.is_loading_model
            && self.worker_tx.is_some()
    }
}

impl eframe::App for PerplexApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_worker_messages();

        if self.is_analyzing || self.is_loading_model {
            ctx.request_repaint();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::Frame::none().inner_margin(20.0).show(ui, |ui| {
                if ui_main::render_header(
                    ui,
                    self.settings.model_path.as_deref(),
                    self.is_loading_model,
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
                    !self.is_analyzing,
                    input_height,
                    self.token_count,
                ) {
                    if let Some(ref tx) = self.worker_tx {
                        let _ = tx.send(WorkerCommand::Tokenize(self.input_text.clone()));
                    }
                }

                if ui_main::render_controls(
                    ui,
                    self.can_analyze(),
                    self.is_analyzing,
                    self.progress,
                ) {
                    self.start_analysis();
                }

                if let Some(ref error) = self.error_message {
                    ui_main::render_error(ui, error);
                }

                if let Some(ref result) = self.analysis_result {
                    let results_height = ui.available_height();
                    ui_main::render_results(ui, result, results_height);
                } else if !self.is_analyzing {
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
                            self.shutdown_worker();
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
        self.shutdown_worker();
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
