use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use std::num::NonZeroU32;
use std::path::Path;
use std::sync::{mpsc, OnceLock};

static LLAMA_BACKEND: OnceLock<LlamaBackend> = OnceLock::new();

fn get_backend() -> &'static LlamaBackend {
    LLAMA_BACKEND.get_or_init(|| {
        log::info!("Initializing Llama backend (one-time)...");
        LlamaBackend::init().expect("Failed to initialize Llama backend")
    })
}

use crate::analysis::{AnalysisResult, AnalyzedToken};
use crate::worker::{WorkerCommand, WorkerMessage};

pub struct LlamaAnalyzer {
    model: Option<LlamaModel>,
}

impl LlamaAnalyzer {
    pub fn new() -> Self {
        Self { model: None }
    }

    pub fn load_model<P: AsRef<Path>>(&mut self, model_path: P) -> Result<(), String> {
        // Drop existing model first to free VRAM before loading the new one.
        self.unload_model();

        let path_str = model_path.as_ref().to_string_lossy().to_string();
        let backend = get_backend();

        log::info!("Loading model from: {}", path_str);

        let model_params = LlamaModelParams::default();

        let model = LlamaModel::load_from_file(backend, model_path, &model_params)
            .map_err(|e| format!("Failed to load model: {}", e))?;

        log::info!("Model loaded");
        self.model = Some(model);
        Ok(())
    }

    pub fn unload_model(&mut self) {
        if self.model.take().is_some() {
            log::info!("Model unloaded, VRAM freed");
        }
    }

    pub fn is_loaded(&self) -> bool {
        self.model.is_some()
    }

    fn model(&self) -> Result<&LlamaModel, String> {
        self.model.as_ref().ok_or_else(|| "No model loaded".to_string())
    }

    pub fn analyze(
        &self,
        text: &str,
        progress_tx: Option<&mpsc::Sender<WorkerMessage>>,
    ) -> Result<AnalysisResult, String> {
        let model = self.model()?;
        let backend = get_backend();
        let start_time = std::time::Instant::now();

        if let Some(tx) = progress_tx {
            let _ = tx.send(WorkerMessage::Progress {
                current: 0,
                total: 1,
            });
        }

        let tokens = model
            .str_to_token(text, llama_cpp_2::model::AddBos::Always)
            .map_err(|e| format!("Failed to tokenize: {}", e))?;

        if tokens.is_empty() {
            return Ok(AnalysisResult {
                tokens: vec![],
                processing_time_ms: start_time.elapsed().as_millis() as u64,
            });
        }

        let total_tokens = tokens.len();
        log::info!("Analyzing {} tokens", total_tokens);

        // Calculate context size needed: total tokens + some buffer (512).
        // Ensure it's at least 4096 (standard Llama context).
        let n_ctx = (total_tokens as u32 + 512).max(4096);
        let n_batch = 512;

        log::info!(
            "Initializing context with n_ctx={}, n_batch={}",
            n_ctx,
            n_batch
        );

        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(n_ctx))
            .with_n_batch(n_batch);

        let mut ctx = model
            .new_context(backend, ctx_params)
            .map_err(|e| format!("Failed to create context: {}", e))?;

        let mut compact_results: Vec<(usize, f32, Vec<(i32, f32)>)> =
            Vec::with_capacity(total_tokens);

        let mut processed_count = 0;

        let mut batch = LlamaBatch::new(n_batch as usize, 1);
        let mut logits: Vec<(i32, f32)> = Vec::with_capacity(32000);

        log::info!("Decoding in batches...");

        // Process tokens in batches to avoid overwhelming the context or memory.
        // This loop decodes a chunk of tokens, then checks the model's prediction
        // for each token against the *actual* next token in the sequence.
        for chunk in tokens.chunks(n_batch as usize) {
            if let Some(tx) = progress_tx {
                let _ = tx.send(WorkerMessage::Progress {
                    current: processed_count,
                    total: total_tokens,
                });
            }

            batch.clear();

            for (i, &token) in chunk.iter().enumerate() {
                let pos = processed_count + i;
                batch
                    .add(token, pos as i32, &[0], true)
                    .map_err(|e| format!("Failed to add token to batch: {}", e))?;
            }

            ctx.decode(&mut batch)
                .map_err(|e| format!("Failed to decode batch: {}", e))?;

            // detailed_results extraction loop
            // For each token we just decoded, we look at the logits generated.
            // These logits represent the model's prediction for the NEXT token.
            for i in 0..chunk.len() {
                let global_pos = processed_count + i;
                let next_token = if global_pos + 1 < total_tokens {
                    Some(tokens[global_pos + 1])
                } else {
                    None
                };

                logits.clear();
                let candidates = ctx.candidates_ith(i as i32);
                logits.extend(candidates.map(|td| (td.id().0, td.logit())));

                let (rank, prob, top_preds) = if let Some(next_tok) = next_token {
                    Self::calculate_token_metrics(&mut logits, Some(next_tok))
                } else {
                    (1, 0.0, Vec::new())
                };

                compact_results.push((rank, prob, top_preds));
            }

            processed_count += chunk.len();
        }

        log::info!("Formatting token texts...");

        if let Some(tx) = progress_tx {
            let _ = tx.send(WorkerMessage::Progress {
                current: total_tokens,
                total: total_tokens,
            });
        }

        let format_start = std::time::Instant::now();

        let mut decoder = encoding_rs::UTF_8.new_decoder();

        let analyzed_tokens: Vec<AnalyzedToken> = tokens
            .iter()
            .enumerate()
            .map(|(i, &token)| {
                let token_text = model
                    .token_to_piece(token, &mut decoder, true, None)
                    .unwrap_or_else(|_| format!("[{}]", token.0));

                let (rank, prob, top_preds_raw) = if i == 0 {
                    (1, 0.0, Vec::new())
                } else {
                    compact_results[i - 1].clone()
                };

                let top_predictions: Vec<(String, f32)> = top_preds_raw
                    .into_iter()
                    .map(|(id, prob)| {
                        let pred_text = model
                            .token_to_piece(
                                llama_cpp_2::token::LlamaToken(id),
                                &mut decoder,
                                true,
                                None,
                            )
                            .unwrap_or_else(|_| format!("[{}]", id));
                        (pred_text, prob)
                    })
                    .collect();

                AnalyzedToken {
                    text: token_text,
                    rank,
                    top_predictions,
                    probability: prob,
                }
            })
            .collect();

        log::info!(
            "Results formatted in {}ms",
            format_start.elapsed().as_millis()
        );

        let elapsed = start_time.elapsed().as_millis() as u64;
        log::info!("Analysis completed in {}ms", elapsed);

        Ok(AnalysisResult {
            tokens: analyzed_tokens,
            processing_time_ms: elapsed,
        })
    }

    // Calculates rank, probability and top predictions for the target token
    // using the raw logits. Performs a Softmax with the "max-trick" for numerical stability.
    fn calculate_token_metrics(
        logits: &mut [(i32, f32)],
        target_token: Option<llama_cpp_2::token::LlamaToken>,
    ) -> (usize, f32, Vec<(i32, f32)>) {
        if logits.is_empty() {
            return (1, 0.0, Vec::new());
        }

        let max_logit = logits
            .iter()
            .map(|(_, l)| *l)
            .fold(f32::NEG_INFINITY, f32::max);

        let sum_exp: f32 = logits.iter().map(|(_, l)| (l - max_logit).exp()).sum();

        logits.sort_unstable_by(|(_, a), (_, b)| {
            b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut rank = 1;
        let mut probability = 0.0;

        if let Some(target) = target_token {
            let target_id = target.0;

            if let Some(idx) = logits.iter().position(|(id, _)| *id == target_id) {
                rank = idx + 1;
                let val = logits[idx].1;

                probability = (val - max_logit).exp() / sum_exp;
            }
        }

        let top_preds = logits
            .iter()
            .take(5)
            .map(|(id, l)| (*id, (l - max_logit).exp() / sum_exp))
            .collect();
        (rank, probability, top_preds)
    }

    pub fn count_tokens(&self, text: &str) -> usize {
        let model = match self.model() {
            Ok(m) => m,
            Err(_) => return 0,
        };
        match model.str_to_token(text, llama_cpp_2::model::AddBos::Never) {
            Ok(tokens) => tokens.len(),
            Err(_) => 0,
        }
    }
}

/// Persistent worker loop that handles model lifecycle and analysis commands.
/// The worker starts with no model loaded and responds to LoadModel/UnloadModel
/// commands, enabling future JIT model swapping to conserve VRAM.
pub fn run_worker(
    cmd_rx: mpsc::Receiver<WorkerCommand>,
    msg_tx: mpsc::Sender<WorkerMessage>,
) {
    log::info!("Worker started, waiting for commands...");

    let mut analyzer = LlamaAnalyzer::new();

    loop {
        match cmd_rx.recv() {
            Ok(WorkerCommand::LoadModel(path)) => {
                match analyzer.load_model(&path) {
                    Ok(()) => {
                        let _ = msg_tx.send(WorkerMessage::ModelLoaded);
                    }
                    Err(e) => {
                        let _ = msg_tx.send(WorkerMessage::Error(
                            format!("Failed to load model: {}", e),
                        ));
                    }
                }
            }
            Ok(WorkerCommand::UnloadModel) => {
                analyzer.unload_model();
                let _ = msg_tx.send(WorkerMessage::ModelUnloaded);
            }
            Ok(WorkerCommand::Analyze(text)) => {
                let _ = msg_tx.send(WorkerMessage::Started);

                match analyzer.analyze(&text, Some(&msg_tx)) {
                    Ok(result) => {
                        let _ = msg_tx.send(WorkerMessage::Completed(result));
                    }
                    Err(e) => {
                        let _ = msg_tx.send(WorkerMessage::Error(e));
                    }
                }
            }
            Ok(WorkerCommand::Tokenize(text)) => {
                let count = analyzer.count_tokens(&text);
                let _ = msg_tx.send(WorkerMessage::TokenCount(count));
            }
            Ok(WorkerCommand::Shutdown) => {
                log::info!("Worker received shutdown command");
                break;
            }
            Err(_) => {
                log::info!("Worker channel closed, shutting down");
                break;
            }
        }
    }
}
