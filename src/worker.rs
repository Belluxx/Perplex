use std::sync::mpsc;
use std::thread;

use crate::analysis::AnalysisResult;

#[derive(Debug)]
pub enum WorkerMessage {
    ModelLoaded,
    Started,
    Progress { current: usize, total: usize },
    Completed(AnalysisResult),
    TokenCount(usize),
    Error(String),
}

#[derive(Debug)]
pub enum WorkerCommand {
    Analyze(String),
    Tokenize(String),
    Shutdown,
}

/// Manages the background worker thread that loads and runs the LLM.
///
/// Owns the communication channels and thread handle, and tracks
/// worker-level state (loading, analyzing, progress). The application
/// layer polls messages via [`poll_messages`] and reacts to them.
pub struct WorkerManager {
    tx: Option<mpsc::Sender<WorkerCommand>>,
    rx: Option<mpsc::Receiver<WorkerMessage>>,
    handle: Option<thread::JoinHandle<()>>,
    pub is_loading: bool,
    pub is_analyzing: bool,
    pub progress: Option<f32>,
}

impl Default for WorkerManager {
    fn default() -> Self {
        Self {
            tx: None,
            rx: None,
            handle: None,
            is_loading: false,
            is_analyzing: false,
            progress: None,
        }
    }
}

impl WorkerManager {
    /// Shuts down any existing worker, then spawns a new one for the given model path.
    pub fn load_model(&mut self, path: String) {
        self.shutdown();

        self.is_loading = true;
        self.is_analyzing = false;
        self.progress = None;

        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (msg_tx, msg_rx) = mpsc::channel();

        self.tx = Some(cmd_tx);
        self.rx = Some(msg_rx);

        let handle = thread::spawn(move || {
            crate::llamacpp::run_analysis_worker(path, cmd_rx, msg_tx);
        });

        self.handle = Some(handle);
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
    /// (`is_loading`, `is_analyzing`, `progress`) as it goes.
    ///
    /// Returns the messages so the application can react to them
    /// (e.g. storing results, displaying errors).
    pub fn poll_messages(&mut self) -> Vec<WorkerMessage> {
        let mut messages = Vec::new();

        if let Some(ref rx) = self.rx {
            while let Ok(msg) = rx.try_recv() {
                match &msg {
                    WorkerMessage::ModelLoaded => {
                        self.is_loading = false;
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
                    WorkerMessage::Error(_) => {
                        self.is_analyzing = false;
                        self.is_loading = false;
                        self.progress = None;
                    }
                    WorkerMessage::TokenCount(_) => {}
                }
                messages.push(msg);
            }
        }

        messages
    }

    /// Returns `true` if a worker is connected and the model has finished loading.
    pub fn is_ready(&self) -> bool {
        self.tx.is_some() && !self.is_loading
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
    }
}
