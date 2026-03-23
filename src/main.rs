mod analysis;
mod colors;
mod llamacpp;
mod settings;
mod ui_main;
mod ui_settings;
mod ui_tokens;
mod worker;

use eframe::egui;

use crate::settings::{PreloadMode, Settings};
use crate::ui_main::{UnifiedColorMode, ViewMode};
use crate::worker::{WorkerCommand, WorkerManager};

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

/// Tracks the sequential JIT analysis when models run one at a time.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum JitPhase {
    Idle,
    /// Model A is being loaded / analyzed.
    RunningA,
    /// Model A finished; unloading before starting B.
    TransitionAtoB,
    /// Model B is being loaded / analyzed.
    RunningB,
    /// Model B finished; unloading.
    CleanupB,
}

struct PerplexApp {
    settings: Settings,
    show_settings: bool,
    settings_path_buffer_a: String,
    settings_path_buffer_b: String,
    settings_preload_buffer: PreloadMode,
    input_text: String,
    result_a: Option<analysis::AnalysisResult>,
    result_b: Option<analysis::AnalysisResult>,
    error_message: Option<String>,
    token_count_a: Option<usize>,
    token_count_b: Option<usize>,
    worker_a: WorkerManager,
    worker_b: WorkerManager,
    view_mode: ViewMode,
    unified_color_mode: UnifiedColorMode,
    jit_phase: JitPhase,
    jit_pending_text: String,
}

impl Default for PerplexApp {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            show_settings: false,
            settings_path_buffer_a: String::new(),
            settings_path_buffer_b: String::new(),
            settings_preload_buffer: PreloadMode::PreloadSingle,
            input_text: String::new(),
            result_a: None,
            result_b: None,
            error_message: None,
            token_count_a: None,
            token_count_b: None,
            worker_a: WorkerManager::new(),
            worker_b: WorkerManager::new(),
            view_mode: ViewMode::Split,
            unified_color_mode: UnifiedColorMode::AvgRank,
            jit_phase: JitPhase::Idle,
            jit_pending_text: String::new(),
        }
    }
}

impl PerplexApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let _ = env_logger::try_init();

        let mut app = Self::default();
        app.settings = Settings::load();

        app.apply_preload_policy();
        app
    }

    fn model_path(&self, slot: ModelSlot) -> Option<&String> {
        match slot {
            ModelSlot::A => self.settings.model_path_a.as_ref(),
            ModelSlot::B => self.settings.model_path_b.as_ref(),
        }
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
            self.set_model(slot, path);
        }
    }

    fn set_model(&mut self, slot: ModelSlot, path: String) {
        *self.model_path_mut(slot) = Some(path.clone());
        self.save_settings();
        self.error_message = None;
        *self.result_mut(slot) = None;

        self.apply_preload_policy();
    }

    fn clear_model(&mut self, slot: ModelSlot) {
        *self.model_path_mut(slot) = None;
        self.save_settings();
        self.worker_mut(slot).unload_model();
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

        let both_configured = self.settings.model_path_a.is_some()
            && self.settings.model_path_b.is_some();

        if both_configured && !self.is_parallel() {
            // JIT: load → analyze → unload, one model at a time.
            self.jit_pending_text = text.clone();
            self.result_a = None;
            self.result_b = None;

            self.jit_phase = JitPhase::RunningA;
            let path = self.settings.model_path_a.clone().unwrap();
            if !self.worker_a.has_model {
                self.worker_a.load_model(path);
            }
            // Queued after LoadModel — runs once loading completes.
            let _ = self
                .worker_a
                .send_command(WorkerCommand::Analyze(text));
        } else {
            // Single model or parallel: send analyze to each ready/configured slot.
            // If a model isn't loaded yet, load it first.
            for slot in ModelSlot::ALL {
                if self.model_path(slot).is_some() {
                    let worker = self.worker_mut(slot);
                    if !worker.has_model && !worker.is_loading {
                        let path = self.model_path(slot).cloned().unwrap();
                        self.worker_mut(slot).load_model(path);
                    }
                    let _ = self
                        .worker_mut(slot)
                        .send_command(WorkerCommand::Analyze(text.clone()));
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
                    worker::WorkerMessage::ModelLoaded => {
                        log::info!("{} loaded and ready", slot.label());
                        // Auto-tokenize when a preloaded model finishes loading.
                        if self.jit_phase == JitPhase::Idle && !input_text.is_empty() {
                            let _ = self
                                .worker_mut(slot)
                                .send_command(WorkerCommand::Tokenize(input_text.clone()));
                        }
                    }
                    worker::WorkerMessage::ModelUnloaded => {
                        log::info!("{} unloaded", slot.label());
                        match slot {
                            ModelSlot::A => self.token_count_a = None,
                            ModelSlot::B => self.token_count_b = None,
                        }
                        self.advance_jit_on_unload(slot);
                    }
                    worker::WorkerMessage::TokenCount(count) => match slot {
                        ModelSlot::A => self.token_count_a = Some(count),
                        ModelSlot::B => self.token_count_b = Some(count),
                    },
                    worker::WorkerMessage::Completed(result) => {
                        *self.result_mut(slot) = Some(result);
                        self.advance_jit_on_complete(slot);
                    }
                    worker::WorkerMessage::Error(error) => {
                        if self.jit_phase != JitPhase::Idle {
                            self.jit_phase = JitPhase::Idle;
                            self.jit_pending_text.clear();
                        }
                        self.append_error(format!("{}: {}", slot.label(), error));
                    }
                    worker::WorkerMessage::Started | worker::WorkerMessage::Progress { .. } => {}
                }
            }
        }
    }

    /// Called when a slot finishes analysis during a JIT sequence.
    fn advance_jit_on_complete(&mut self, slot: ModelSlot) {
        match (self.jit_phase, slot) {
            (JitPhase::RunningA, ModelSlot::A) => {
                self.worker_a.unload_model();
                self.jit_phase = JitPhase::TransitionAtoB;
            }
            (JitPhase::RunningB, ModelSlot::B) => {
                self.worker_b.unload_model();
                self.jit_phase = JitPhase::CleanupB;
            }
            _ => {}
        }
    }

    /// Called when a slot finishes unloading during a JIT sequence.
    fn advance_jit_on_unload(&mut self, slot: ModelSlot) {
        match (self.jit_phase, slot) {
            (JitPhase::TransitionAtoB, ModelSlot::A) => {
                if let Some(path) = self.settings.model_path_b.clone() {
                    self.jit_phase = JitPhase::RunningB;
                    self.worker_b.load_model(path);
                    let _ = self
                        .worker_b
                        .send_command(WorkerCommand::Analyze(self.jit_pending_text.clone()));
                } else {
                    // No model B configured — JIT done.
                    self.jit_phase = JitPhase::Idle;
                    self.jit_pending_text.clear();
                }
            }
            (JitPhase::CleanupB, ModelSlot::B) => {
                self.jit_phase = JitPhase::Idle;
                self.jit_pending_text.clear();
            }
            _ => {}
        }
    }

    /// Whether a given slot should be preloaded under the current settings.
    fn should_preload(&self, slot: ModelSlot) -> bool {
        if self.model_path(slot).is_none() {
            return false;
        }
        match self.settings.preload_mode {
            PreloadMode::PreloadAll => true,
            PreloadMode::PreloadSingle => {
                // Only preload when exactly one model is configured.
                let count = self.settings.model_path_a.is_some() as u8
                    + self.settings.model_path_b.is_some() as u8;
                count == 1
            }
            PreloadMode::NoPreload => false,
        }
    }

    /// Loads or unloads models to match the current preload policy.
    fn apply_preload_policy(&mut self) {
        for slot in ModelSlot::ALL {
            let should = self.should_preload(slot);
            let worker = self.worker_mut(slot);
            if should && !worker.has_model && !worker.is_loading {
                if let Some(path) = self.model_path(slot).cloned() {
                    self.worker_mut(slot).load_model(path);
                }
            } else if !should && worker.has_model {
                self.worker_mut(slot).unload_model();
            }
        }
    }

    /// Whether preloaded models can serve the analysis (no JIT needed).
    fn is_parallel(&self) -> bool {
        self.settings.preload_mode == PreloadMode::PreloadAll
            && self.settings.model_path_a.is_some()
            && self.settings.model_path_b.is_some()
    }

    fn has_any_model(&self) -> bool {
        self.settings.model_path_a.is_some() || self.settings.model_path_b.is_some()
    }

    fn can_analyze(&self) -> bool {
        !self.input_text.is_empty() && self.has_any_model() && !self.is_busy()
    }

    /// True when any work is in progress (analysis, loading, or JIT sequencing).
    fn is_busy(&self) -> bool {
        self.worker_a.is_analyzing
            || self.worker_b.is_analyzing
            || self.worker_a.is_loading
            || self.worker_b.is_loading
            || self.jit_phase != JitPhase::Idle
    }
}

impl eframe::App for PerplexApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_worker_messages();

        if self.is_busy() {
            ctx.request_repaint();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::Frame::none().inner_margin(20.0).show(ui, |ui| {
                let header = ui_main::render_header(
                    ui,
                    self.settings.model_path_a.as_deref(),
                    self.settings.model_path_b.as_deref(),
                    self.worker_a.is_loading,
                    self.worker_b.is_loading,
                );
                if header.settings {
                    self.show_settings = true;
                    for slot in ModelSlot::ALL {
                        *self.settings_path_buffer_mut(slot) =
                            self.model_path_mut(slot).clone().unwrap_or_default();
                    }
                    self.settings_preload_buffer = self.settings.preload_mode;
                }
                if header.eject_a {
                    self.clear_model(ModelSlot::A);
                }
                if header.eject_b {
                    self.clear_model(ModelSlot::B);
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

                let not_busy = !self.is_busy();
                if ui_main::render_text_input(
                    ui,
                    &mut self.input_text,
                    not_busy,
                    input_height,
                    self.token_count_a,
                    self.token_count_b,
                ) {
                    // Live token counts when models are preloaded.
                    let updated_text = self.input_text.clone();
                    for slot in ModelSlot::ALL {
                        if self.worker_mut(slot).is_ready() {
                            let _ = self
                                .worker_mut(slot)
                                .send_command(WorkerCommand::Tokenize(
                                    updated_text.clone(),
                                ));
                        }
                    }
                }

                if ui_main::render_controls(
                    ui,
                    self.can_analyze(),
                    self.is_busy(),
                    self.worker_a.progress,
                    self.worker_b.progress,
                ) {
                    self.start_analysis();
                }

                if let Some(ref error) = self.error_message {
                    ui_main::render_error(ui, error);
                }

                // Re-check after start_analysis may have cleared results.
                let has_results = self.result_a.is_some() || self.result_b.is_some();
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
                } else if !self.is_busy() {
                    ui_main::render_empty_state(ui, self.has_any_model());
                }
            });
        });

        if self.show_settings {
            if let Some(action) = ui_settings::render_settings_window(
                ctx,
                &mut self.show_settings,
                &mut self.settings_path_buffer_a,
                &mut self.settings_path_buffer_b,
                &mut self.settings_preload_buffer,
            ) {
                match action {
                    ui_settings::SettingsAction::Browse(slot) => {
                        if let Some(path) = pick_gguf_model() {
                            *self.settings_path_buffer_mut(slot) = path;
                        }
                    }
                    ui_settings::SettingsAction::Save => {
                        self.show_settings = false;

                        self.settings.preload_mode = self.settings_preload_buffer;

                        for slot in ModelSlot::ALL {
                            let path = self.settings_path_buffer_mut(slot).clone();
                            if !path.is_empty() {
                                if self.model_path_mut(slot).as_deref() != Some(&path) {
                                    *self.model_path_mut(slot) = Some(path);
                                    *self.result_mut(slot) = None;
                                }
                            } else {
                                if self.model_path(slot).is_some() {
                                    self.worker_mut(slot).unload_model();
                                }
                                *self.model_path_mut(slot) = None;
                                *self.result_mut(slot) = None;
                            }
                        }

                        self.apply_preload_policy();
                        self.save_settings();
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

pub fn model_name_from_path(path: Option<&str>) -> Option<&str> {
    // Remove final .gguf if present
    path.and_then(|p| {
        let path = std::path::Path::new(p);
        path.file_name().and_then(|n| n.to_str()).and_then(|n| {
            if n.ends_with(".gguf") {
                Some(&n[..n.len() - 5])
            } else {
                Some(n)
            }
        })
    })
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
