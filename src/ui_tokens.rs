use crate::analysis::AnalyzedToken;
use crate::colors;
use crate::ui_main::UnifiedColorMode;
use egui::{Color32, RichText, Ui, Vec2};

// ── Shared helpers ──────────────────────────────────────────────────────────

fn format_display_text(text: &str) -> String {
    text.replace('\n', "↵\n").replace('\t', "→")
}

fn render_token_label(ui: &mut Ui, display_text: &str, bg_color: Color32) -> egui::Response {
    ui.add(
        egui::Label::new(
            RichText::new(display_text)
                .color(Color32::BLACK)
                .background_color(bg_color)
                .size(14.0)
                .family(egui::FontFamily::Monospace),
        )
        .sense(egui::Sense::hover()),
    )
}

fn render_tooltip_header(ui: &mut Ui, token_text: &str) {
    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
        ui.label(
            RichText::new(token_text)
                .strong()
                .monospace()
                .size(15.0)
                .background_color(colors::secondary_bg(ui.visuals())),
        );
    });
    ui.add_space(6.0);
}

// ── Split-view token rendering ──────────────────────────────────────────────

pub fn render_analyzed_tokens(
    ui: &mut Ui,
    tokens: &[AnalyzedToken],
    other_tokens: Option<&[AnalyzedToken]>,
    self_label: &str,
    other_label: &str,
) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, 4.0);

        for (i, token) in tokens.iter().enumerate() {
            let other = other_tokens.and_then(|ot| ot.get(i));
            render_token(ui, token, other, self_label, other_label);
        }
    });
}

fn render_token(
    ui: &mut Ui,
    token: &AnalyzedToken,
    other_token: Option<&AnalyzedToken>,
    self_label: &str,
    other_label: &str,
) {
    let bg_color = colors::rank_to_color(token.rank);
    let display_text = format_display_text(&token.text);

    let response = render_token_label(ui, &display_text, bg_color);

    response.on_hover_ui(|ui| {
        ui.set_max_width(340.0);
        ui.set_min_width(340.0);

        render_tooltip_header(ui, &token.text);

        if let Some(other) = other_token {
            render_comparison_tooltip(ui, token, other, self_label, other_label);
        } else {
            render_single_tooltip(ui, token);
        }
    });
}

// ── Unified-view token rendering ────────────────────────────────────────────

pub fn render_unified_tokens(
    ui: &mut Ui,
    tokens_a: &[AnalyzedToken],
    tokens_b: &[AnalyzedToken],
    label_a: &str,
    label_b: &str,
    color_mode: UnifiedColorMode,
) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, 4.0);

        let len = tokens_a.len().max(tokens_b.len());
        for i in 0..len {
            let tok_a = tokens_a.get(i);
            let tok_b = tokens_b.get(i);

            let display_token = tok_a.or(tok_b).unwrap();
            let display_text = format_display_text(&display_token.text);

            let bg_color = match (tok_a, tok_b) {
                (Some(a), Some(b)) => match color_mode {
                    UnifiedColorMode::AvgRank => colors::average_rank_color(a.rank, b.rank),
                    UnifiedColorMode::AvgProbability => {
                        colors::average_prob_color(a.probability, b.probability)
                    }
                    UnifiedColorMode::RankDivergence => {
                        colors::rank_divergence_color(a.rank, b.rank)
                    }
                    UnifiedColorMode::ProbDivergence => {
                        colors::prob_divergence_color(a.probability, b.probability)
                    }
                },
                (Some(a), None) => colors::rank_to_color(a.rank),
                (None, Some(b)) => colors::rank_to_color(b.rank),
                (None, None) => unreachable!(),
            };

            let response = render_token_label(ui, &display_text, bg_color);

            response.on_hover_ui(|ui| {
                ui.set_max_width(320.0);
                ui.set_min_width(320.0);

                render_tooltip_header(ui, &display_token.text);

                if let (Some(a), Some(b)) = (tok_a, tok_b) {
                    render_comparison_tooltip(ui, a, b, label_a, label_b);
                } else if let Some(t) = tok_a.or(tok_b) {
                    render_single_tooltip(ui, t);
                }
            });
        }
    });
}

// ── Tooltips ────────────────────────────────────────────────────────────────

fn render_comparison_tooltip(
    ui: &mut Ui,
    token: &AnalyzedToken,
    other: &AnalyzedToken,
    self_label: &str,
    other_label: &str,
) {
    ui.separator();
    ui.add_space(4.0);

    egui::Grid::new(ui.next_auto_id())
        .num_columns(3)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            ui.label(RichText::new("").size(11.0));
            ui.label(
                RichText::new(self_label)
                    .strong()
                    .size(11.0)
                    .color(colors::INFO),
            );
            ui.label(
                RichText::new(other_label)
                    .strong()
                    .size(11.0)
                    .color(colors::WARNING),
            );
            ui.end_row();

            ui.label(RichText::new("Rank").size(11.0));
            render_rank_badge(ui, token.rank);
            render_rank_badge(ui, other.rank);
            ui.end_row();

            ui.label(RichText::new("Prob").size(11.0));
            render_prob_label(ui, token.probability);
            render_prob_label(ui, other.probability);
            ui.end_row();
        });

    ui.add_space(6.0);
    ui.separator();
    ui.add_space(4.0);

    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.label(
                RichText::new(self_label)
                    .strong()
                    .size(11.0)
                    .color(colors::INFO),
            );
            render_prediction_list(ui, &token.top_predictions);
        });

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(12.0);

        ui.vertical(|ui| {
            ui.label(
                RichText::new(other_label)
                    .strong()
                    .size(11.0)
                    .color(colors::WARNING),
            );
            render_prediction_list(ui, &other.top_predictions);
        });
    });
}

fn render_single_tooltip(ui: &mut Ui, token: &AnalyzedToken) {
    ui.label(RichText::new(format!("Rank: {}", token.rank)).size(12.0));

    if !token.top_predictions.is_empty() {
        ui.add_space(6.0);
        ui.label(RichText::new("Top Predictions:").strong().size(11.0));
        render_prediction_list(ui, &token.top_predictions);
    }
}

// ── Tooltip helpers ─────────────────────────────────────────────────────────

fn render_rank_badge(ui: &mut Ui, rank: usize) {
    let color = colors::rank_to_color(rank);
    ui.label(
        RichText::new(format!("#{}", rank))
            .strong()
            .size(12.0)
            .background_color(color)
            .color(Color32::BLACK),
    );
}

fn render_prob_label(ui: &mut Ui, prob: f32) {
    let text = if prob < 0.001 {
        "<0.1%".to_string()
    } else {
        format!("{:.1}%", prob * 100.0)
    };
    let color = colors::prob_to_color(prob);
    ui.label(
        RichText::new(text)
            .strong()
            .size(11.0)
            .background_color(color)
            .color(Color32::BLACK),
    );
}

fn render_prediction_list(ui: &mut Ui, predictions: &[(String, f32)]) {
    if predictions.is_empty() {
        ui.label(RichText::new("—").size(11.0));
        return;
    }
    for (i, (pred_text, prob)) in predictions.iter().enumerate() {
        let display = pred_text.replace('\n', "↵").replace('\t', "→");
        let pct = if *prob < 0.01 {
            "<1%".to_string()
        } else {
            format!("{:.0}%", prob * 100.0)
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{}.", i + 1)).size(11.0));
            ui.label(RichText::new(&display).monospace().size(11.0));
            ui.label(
                RichText::new(pct)
                    .size(10.0)
                    .color(colors::text_muted(ui.visuals())),
            );
        });
    }
}
