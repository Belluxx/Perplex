#[derive(Clone, Debug)]
pub struct AnalyzedToken {
    pub text: String,
    pub rank: usize,
    pub top_predictions: Vec<(String, f32)>,
    pub probability: f32,
}

#[derive(Clone, Debug)]
pub struct AnalysisResult {
    pub tokens: Vec<AnalyzedToken>,
    pub processing_time_ms: u64,
}

impl AnalysisResult {
    fn scored_tokens(&self) -> &[AnalyzedToken] {
        if self.tokens.len() <= 1 {
            &[]
        } else {
            &self.tokens[1..]
        }
    }

    // Perplexity is the exponential of the average negative log-likelihood per token.
    // Formula: exp( - (1/N) * Σ ln(P(word_i)) )
    pub fn perplexity(&self) -> f32 {
        let scored = self.scored_tokens();
        if scored.is_empty() {
            return 0.0;
        }
        let sum_log_probs: f32 = scored.iter().map(|t| -t.probability.ln()).sum();
        (sum_log_probs / scored.len() as f32).exp()
    }

    pub fn text_entropy(&self) -> f32 {
        if self.scored_tokens().is_empty() {
            return 0.0;
        }
        let ppl = self.perplexity();
        let n = self.scored_tokens().len() as f32;
        n * ppl.log2()
    }
}
