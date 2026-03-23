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

    fn index(self) -> usize {
        match self {
            ModelSlot::A => 0,
            ModelSlot::B => 1,
        }
    }

    fn label(self) -> &'static str {
        match self {
            ModelSlot::A => "Model A",
            ModelSlot::B => "Model B",
        }
    }
}

/// Per-slot state: each model slot owns its worker, results, and UI buffers.
struct SlotState {
    worker: WorkerManager,
    result: Option<analysis::AnalysisResult>,
    token_count: Option<usize>,
    settings_path_buffer: String,
}

impl Default for SlotState {
    fn default() -> Self {
        Self {
            worker: WorkerManager::new(),
            result: None,
            token_count: None,
            settings_path_buffer: String::new(),
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
    settings_preload_buffer: PreloadMode,
    input_text: String,
    slots: [SlotState; 2],
    error_message: Option<String>,
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
            settings_preload_buffer: PreloadMode::PreloadSingle,
            input_text: String::new(),
            slots: Default::default(),
            error_message: None,
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

    fn select_model(&mut self, slot: ModelSlot) {
        if let Some(path) = pick_gguf_model() {
            self.set_model(slot, path);
        }
    }

    fn set_model(&mut self, slot: ModelSlot, path: String) {
        *self.model_path_mut(slot) = Some(path);
        self.save_settings();
        self.error_message = None;
        self.slots[slot.index()].result = None;

        self.apply_preload_policy();
    }

    fn clear_model(&mut self, slot: ModelSlot) {
        *self.model_path_mut(slot) = None;
        self.save_settings();
        let s = &mut self.slots[slot.index()];
        s.worker.unload_model();
        s.result = None;
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
            self.slots[0].result = None;
            self.slots[1].result = None;

            self.jit_phase = JitPhase::RunningA;
            let path = self.settings.model_path_a.clone().unwrap();
            let a = &mut self.slots[ModelSlot::A.index()];
            if !a.worker.has_model {
                a.worker.load_model(path);
            }
            // Queued after LoadModel — runs once loading completes.
            let _ = a.worker.send_command(WorkerCommand::Analyze(text));
        } else {
            // Single model or parallel: send analyze to each ready/configured slot.
            // If a model isn't loaded yet, load it first.
            for slot in ModelSlot::ALL {
                if let Some(path) = self.model_path(slot).cloned() {
                    let s = &mut self.slots[slot.index()];
                    if !s.worker.has_model && !s.worker.is_loading {
                        s.worker.load_model(path);
                    }
                    let _ = s.worker.send_command(WorkerCommand::Analyze(text.clone()));
                }
            }
        }
    }

    fn process_worker_messages(&mut self) {
        let input_text = self.input_text.clone();

        for slot in ModelSlot::ALL {
            let messages = self.slots[slot.index()].worker.poll_messages();
            for msg in messages {
                match msg {
                    worker::WorkerMessage::ModelLoaded => {
                        log::info!("{} loaded and ready", slot.label());
                        if self.jit_phase == JitPhase::Idle && !input_text.is_empty() {
                            let _ = self.slots[slot.index()]
                                .worker
                                .send_command(WorkerCommand::Tokenize(input_text.clone()));
                        }
                    }
                    worker::WorkerMessage::ModelUnloaded => {
                        log::info!("{} unloaded", slot.label());
                        self.slots[slot.index()].token_count = None;
                        self.advance_jit_on_unload(slot);
                    }
                    worker::WorkerMessage::TokenCount(count) => {
                        self.slots[slot.index()].token_count = Some(count);
                    }
                    worker::WorkerMessage::Completed(result) => {
                        self.slots[slot.index()].result = Some(result);
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
                self.slots[ModelSlot::A.index()].worker.unload_model();
                self.jit_phase = JitPhase::TransitionAtoB;
            }
            (JitPhase::RunningB, ModelSlot::B) => {
                self.slots[ModelSlot::B.index()].worker.unload_model();
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
                    let b = &mut self.slots[ModelSlot::B.index()];
                    b.worker.load_model(path);
                    let _ = b
                        .worker
                        .send_command(WorkerCommand::Analyze(self.jit_pending_text.clone()));
                } else {
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
            let has = self.slots[slot.index()].worker.has_model;
            let loading = self.slots[slot.index()].worker.is_loading;
            if should && !has && !loading {
                if let Some(path) = self.model_path(slot).cloned() {
                    self.slots[slot.index()].worker.load_model(path);
                }
            } else if !should && has {
                self.slots[slot.index()].worker.unload_model();
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
        self.slots.iter().any(|s| s.worker.is_analyzing || s.worker.is_loading)
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
                    self.slots[0].worker.is_loading,
                    self.slots[1].worker.is_loading,
                );
                if header.settings {
                    self.show_settings = true;
                    for slot in ModelSlot::ALL {
                        self.slots[slot.index()].settings_path_buffer =
                            self.model_path(slot).cloned().unwrap_or_default();
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
                let has_results = self.slots[0].result.is_some() || self.slots[1].result.is_some();
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
                    self.slots[0].token_count,
                    self.slots[1].token_count,
                ) {
                    // Live token counts when models are preloaded.
                    let updated_text = self.input_text.clone();
                    for slot in ModelSlot::ALL {
                        let s = &mut self.slots[slot.index()];
                        if s.worker.is_ready() {
                            let _ = s.worker.send_command(WorkerCommand::Tokenize(
                                updated_text.clone(),
                            ));
                        }
                    }
                }

                if ui_main::render_controls(
                    ui,
                    self.can_analyze(),
                    self.is_busy(),
                    self.slots[0].worker.progress,
                    self.slots[1].worker.progress,
                ) {
                    self.start_analysis();
                }

                if let Some(ref error) = self.error_message {
                    ui_main::render_error(ui, error);
                }

                // Re-check after start_analysis may have cleared results.
                let has_results = self.slots[0].result.is_some() || self.slots[1].result.is_some();
                if has_results {
                    ui_main::render_results(
                        ui,
                        self.slots[0].result.as_ref(),
                        self.slots[1].result.as_ref(),
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
            let [slot_a, slot_b] = &mut self.slots;
            let action = ui_settings::render_settings_window(
                ctx,
                &mut self.show_settings,
                &mut slot_a.settings_path_buffer,
                &mut slot_b.settings_path_buffer,
                &mut self.settings_preload_buffer,
            );
            if let Some(action) = action {
                match action {
                    ui_settings::SettingsAction::Browse(slot) => {
                        if let Some(path) = pick_gguf_model() {
                            self.slots[slot.index()].settings_path_buffer = path;
                        }
                    }
                    ui_settings::SettingsAction::Save => {
                        self.show_settings = false;

                        self.settings.preload_mode = self.settings_preload_buffer;

                        for slot in ModelSlot::ALL {
                            let buf = self.slots[slot.index()].settings_path_buffer.clone();
                            if !buf.is_empty() {
                                if self.model_path_mut(slot).as_deref() != Some(&buf) {
                                    *self.model_path_mut(slot) = Some(buf);
                                    self.slots[slot.index()].result = None;
                                }
                            } else {
                                if self.model_path(slot).is_some() {
                                    self.slots[slot.index()].worker.unload_model();
                                }
                                *self.model_path_mut(slot) = None;
                                self.slots[slot.index()].result = None;
                            }
                        }

                        self.apply_preload_policy();
                        self.save_settings();
                    }
                    ui_settings::SettingsAction::Clear(slot) => {
                        self.slots[slot.index()].settings_path_buffer.clear();
                    }
                }
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        for s in &mut self.slots {
            s.worker.shutdown();
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
