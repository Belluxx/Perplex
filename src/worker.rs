use std::sync::mpsc::{self, TryRecvError};
use std::thread;

use crate::analysis::AnalysisResult;

#[derive(Debug)]
pub enum WorkerMessage {
    ModelLoaded,
    ModelUnloaded,
    Started,
    Progress { current: usize, total: usize },
    Completed(AnalysisResult),
    TokenCount(usize),
    /// Loading a model failed; the worker is left without a model.
    LoadError(String),
    Error(String),
}

#[derive(Debug)]
pub enum WorkerCommand {
    LoadModel(String),
    UnloadModel,
    Analyze(String),
    Tokenize(String),
    Shutdown,
}

/// Manages a persistent background worker thread for LLM operations.
///
/// The worker thread is spawned once and kept alive for the duration of
/// the manager. Model loading and unloading are handled via commands,
/// allowing future JIT model swapping without restarting threads.
pub struct WorkerManager {
    tx: Option<mpsc::Sender<WorkerCommand>>,
    rx: Option<mpsc::Receiver<WorkerMessage>>,
    handle: Option<thread::JoinHandle<()>>,
    pub is_loading: bool,
    pub is_analyzing: bool,
    pub progress: Option<f32>,
    pub has_model: bool,
    pub requested_path: Option<String>,
}

impl WorkerManager {
    /// Creates a new manager and spawns its persistent worker thread.
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (msg_tx, msg_rx) = mpsc::channel();

        let handle = thread::spawn(move || {
            crate::llamacpp::run_worker(cmd_rx, msg_tx);
        });

        Self {
            tx: Some(cmd_tx),
            rx: Some(msg_rx),
            handle: Some(handle),
            is_loading: false,
            is_analyzing: false,
            progress: None,
            has_model: false,
            requested_path: None,
        }
    }

    /// Sends a LoadModel command to the worker thread.
    pub fn load_model(&mut self, path: String) {
        self.is_loading = true;
        self.is_analyzing = false;
        self.progress = None;
        self.requested_path = Some(path.clone());

        if let Some(ref tx) = self.tx {
            let _ = tx.send(WorkerCommand::LoadModel(path));
        }
    }

    /// Sends an UnloadModel command to the worker thread.
    pub fn unload_model(&mut self) {
        if let Some(ref tx) = self.tx {
            let _ = tx.send(WorkerCommand::UnloadModel);
        }
        self.has_model = false;
        self.requested_path = None;
    }

    /// Sends a command to the worker thread. Returns an error if no worker is active.
    pub fn send_command(&self, cmd: WorkerCommand) -> Result<(), String> {
        if let Some(ref tx) = self.tx {
            tx.send(cmd)
                .map_err(|e| format!("Failed to send command: {}", e))
        } else {
            Err("No worker available".to_string())
        }
    }

    /// Drains all pending messages from the worker, updating internal state
    /// (`is_loading`, `is_analyzing`, `progress`, `has_model`) as it goes.
    ///
    /// Returns the messages so the application can react to them
    /// (e.g. storing results, displaying errors).
    pub fn poll_messages(&mut self) -> Vec<WorkerMessage> {
        let mut messages = Vec::new();
        let mut disconnected = false;

        if let Some(ref rx) = self.rx {
            loop {
                match rx.try_recv() {
                    Ok(msg) => {
                        match &msg {
                            WorkerMessage::ModelLoaded => {
                                self.is_loading = false;
                                self.has_model = true;
                            }
                            WorkerMessage::ModelUnloaded => {
                                self.has_model = false;
                            }
                            WorkerMessage::Started => {
                                self.is_analyzing = true;
                                self.progress = Some(0.0);
                            }
                            WorkerMessage::Progress { current, total } => {
                                self.progress = Some(*current as f32 / (*total).max(1) as f32);
                            }
                            WorkerMessage::Completed(_) => {
                                self.is_analyzing = false;
                                self.progress = None;
                            }
                            WorkerMessage::LoadError(_) => {
                                // The analyzer drops any previous model before
                                // loading, so a failed load leaves the worker
                                // without a model.
                                self.is_loading = false;
                                self.has_model = false;
                                self.requested_path = None;
                                self.progress = None;
                            }
                            WorkerMessage::Error(_) => {
                                self.is_analyzing = false;
                                self.is_loading = false;
                                self.progress = None;
                            }
                            WorkerMessage::TokenCount(_) => {}
                        }
                        messages.push(msg);
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        // Worker thread is gone (likely panicked)
                        self.is_loading = false;
                        self.is_analyzing = false;
                        self.progress = None;
                        self.has_model = false;
                        self.requested_path = None;
                        messages.push(WorkerMessage::Error(
                            "Worker thread terminated unexpectedly".to_string(),
                        ));
                        disconnected = true;
                        break;
                    }
                }
            }
        }

        if disconnected {
            self.rx = None;
            self.tx = None;
        }

        messages
    }

    /// Returns `true` if a model is loaded and the worker is idle.
    pub fn is_ready(&self) -> bool {
        self.has_model && !self.is_loading
    }

    /// Sends a shutdown command and joins the worker thread.
    pub fn shutdown(&mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(WorkerCommand::Shutdown);
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        self.rx = None;
        self.has_model = false;
        self.requested_path = None;
    }
}
