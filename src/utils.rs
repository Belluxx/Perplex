use crate::colors;
use egui::Color32;

#[derive(Clone, Debug)]
pub struct AnalyzedToken {
    #[allow(dead_code)]
    pub text: String,
    pub display_text: String,
    pub rank: usize,
    pub top_predictions: Vec<(String, f32)>,
    pub probability: f32,
}

impl AnalyzedToken {
    pub fn new(
        text: String,
        rank: usize,
        top_predictions: Vec<(String, f32)>,
        probability: f32,
    ) -> Self {
        let display_text = text.replace('\n', "↵\n").replace('\t', "→");
        Self {
            text,
            display_text,
            rank,
            top_predictions,
            probability,
        }
    }

    pub fn get_color(&self) -> Color32 {
        colors::rank_to_color(self.rank)
    }
}

#[derive(Clone, Debug)]
pub struct AnalysisResult {
    pub tokens: Vec<AnalyzedToken>,
    pub processing_time_ms: u64,
}

impl AnalysisResult {
    pub fn new(tokens: Vec<AnalyzedToken>, processing_time_ms: u64) -> Self {
        Self {
            tokens,
            processing_time_ms,
        }
    }

    pub fn average_rank(&self) -> f32 {
        if self.tokens.len() <= 1 {
            return 0.0;
        }

        let tokens_scored = &self.tokens[1..];
        let sum: usize = tokens_scored.iter().map(|t| t.rank).sum();
        sum as f32 / tokens_scored.len() as f32
    }

    pub fn exact_prediction_percentage(&self) -> f32 {
        if self.tokens.len() <= 1 {
            return 0.0;
        }

        let tokens_scored = &self.tokens[1..];
        let exact = tokens_scored.iter().filter(|t| t.rank == 1).count();
        (exact as f32 / tokens_scored.len() as f32) * 100.0
    }

    // Perplexity is the exponential of the average negative log-likelihood per token.
    // Formula: exp( - (1/N) * Σ ln(P(word_i)) )
    pub fn perplexity(&self) -> f32 {
        if self.tokens.len() <= 1 {
            return 0.0;
        }

        let tokens_scored = &self.tokens[1..];

        let sum_log_probs: f32 = tokens_scored.iter().map(|t| -t.probability.ln()).sum();

        (sum_log_probs / tokens_scored.len() as f32).exp()
    }

    pub fn text_entropy(&self) -> f32 {
        if self.tokens.len() <= 1 {
            return 0.0;
        }

        let ppl = self.perplexity();
        let n = self.tokens.len() as f32;
        n * ppl.log2()
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_average_rank() {
        let tokens = vec![
            AnalyzedToken::new("a".to_string(), 1, vec![], 0.9),
            AnalyzedToken::new("b".to_string(), 5, vec![], 0.1),
            AnalyzedToken::new("c".to_string(), 10, vec![], 0.05),
        ];
        let result = AnalysisResult::new(tokens, 100);

        assert!((result.average_rank() - 7.5).abs() < 0.1);
    }

    #[test]
    fn test_perplexity() {
        let tokens = vec![
            AnalyzedToken::new("a".to_string(), 1, vec![], 0.9),
            AnalyzedToken::new("b".to_string(), 5, vec![], 0.1),
            AnalyzedToken::new("c".to_string(), 10, vec![], 0.05),
        ];
        let result = AnalysisResult::new(tokens, 100);

        assert!((result.perplexity() - 14.14).abs() < 0.1);
    }
}
