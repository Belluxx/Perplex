mod colors;
mod llamacpp;
mod ui;
mod utils;

use eframe::egui;
use std::fs;
use std::path::Path;
use std::sync::mpsc;
use std::thread;

use crate::utils::{AnalysisResult, WorkerCommand, WorkerMessage};

const CONFIG_FILE: &str = ".perplex_model_config";

fn save_model_path(path: &str) {
    if let Err(e) = fs::write(CONFIG_FILE, path) {
        log::warn!("Failed to save model path: {}", e);
    }
}

fn load_last_model_path() -> Option<String> {
    if let Ok(path) = fs::read_to_string(CONFIG_FILE) {
        let path = path.trim().to_string();
        if Path::new(&path).exists() {
            return Some(path);
        }
    }
    None
}

struct PerplexApp {
    model_path: Option<String>,

    input_text: String,

    analysis_result: Option<AnalysisResult>,

    error_message: Option<String>,

    is_loading_model: bool,

    is_analyzing: bool,

    progress: Option<f32>,

    worker_tx: Option<mpsc::Sender<WorkerCommand>>,

    worker_rx: Option<mpsc::Receiver<WorkerMessage>>,

    worker_handle: Option<thread::JoinHandle<()>>,
}

impl Default for PerplexApp {
    fn default() -> Self {
        Self {
            model_path: None,
            input_text: String::new(),
            analysis_result: None,
            error_message: None,
            is_loading_model: false,
            is_analyzing: false,
            progress: None,
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
        if let Some(path) = load_last_model_path() {
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
        save_model_path(&path);
        self.shutdown_worker();

        self.is_loading_model = true;
        self.error_message = None;
        self.analysis_result = None;

        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (msg_tx, msg_rx) = mpsc::channel();

        self.worker_tx = Some(cmd_tx);
        self.worker_rx = Some(msg_rx);

        let model_path = path.clone();
        self.model_path = Some(path);

        let handle = thread::spawn(move || {
            llamacpp::run_analysis_worker(model_path, cmd_rx, msg_tx);
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
        self.model_path.is_some()
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
                ui::render_header(ui, self.model_path.as_deref(), self.is_loading_model);

                ui.add_space(12.0);

                if ui::render_model_panel(ui, self.model_path.is_some()) {
                    self.select_model();
                }

               
               
                let available = ui.available_height();
                let has_results = self.analysis_result.is_some();

               
               
                let input_height = if has_results {
                    (available * 0.35).max(120.0)
                } else {
                    (available * 0.4).max(150.0)
                };

                ui::render_text_input(ui, &mut self.input_text, !self.is_analyzing, input_height);

                if ui::render_controls(ui, self.can_analyze(), self.is_analyzing, self.progress) {
                    self.start_analysis();
                }

                if let Some(ref error) = self.error_message {
                    ui::render_error(ui, error);
                }

                if let Some(ref result) = self.analysis_result {
                    let results_height = ui.available_height();
                    ui::render_results(ui, result, results_height);
                } else if !self.is_analyzing {
                    ui::render_empty_state(ui, self.model_path.is_some());
                }
            });
        });
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
