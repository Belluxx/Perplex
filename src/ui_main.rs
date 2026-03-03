use crate::analysis::{AnalysisResult, AnalyzedToken};
use crate::colors;
use egui::{Color32, FontId, RichText, Ui, Vec2};

// ── View mode enums ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Split,
    Unified,
}

impl std::fmt::Display for ViewMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViewMode::Split => write!(f, "Split"),
            ViewMode::Unified => write!(f, "Unified"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnifiedColorMode {
    AvgRank,
    AvgProbability,
    RankDivergence,
    ProbDivergence,
}

impl std::fmt::Display for UnifiedColorMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnifiedColorMode::AvgRank => write!(f, "Average rank"),
            UnifiedColorMode::AvgProbability => write!(f, "Average probability"),
            UnifiedColorMode::RankDivergence => write!(f, "Divergence rank"),
            UnifiedColorMode::ProbDivergence => write!(f, "Divergence probability"),
        }
    }
}

// ── Header ──────────────────────────────────────────────────────────────────

/// Renders the app header. Returns `true` if the settings button was clicked.
pub fn render_header(
    ui: &mut Ui,
    model_path_a: Option<&str>,
    model_path_b: Option<&str>,
    is_loading_a: bool,
    is_loading_b: bool,
) -> bool {
    let mut settings_clicked = false;
    ui.horizontal(|ui| {
        ui.heading(
            RichText::new("🔮 Perplex")
                .size(28.0)
                .color(colors::ACCENT_PRIMARY),
        );

        ui.add_space(20.0);

        render_model_badge(ui, "A", model_path_a, is_loading_a);
        ui.add_space(10.0);
        render_model_badge(ui, "B", model_path_b, is_loading_b);

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(egui::Button::new(RichText::new("⚙").size(18.0)))
                .clicked()
            {
                settings_clicked = true;
            }
        });
    });

    ui.add_space(8.0);
    ui.separator();
    settings_clicked
}

fn render_model_badge(ui: &mut Ui, label: &str, path: Option<&str>, is_loading: bool) {
    if is_loading {
        ui.spinner();
        ui.label(
            RichText::new(format!("{}: Loading…", label))
                .color(colors::text_primary(ui.visuals()))
                .size(12.0),
        );
    } else if let Some(p) = path {
        let name = std::path::Path::new(p)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(p);
        ui.label(
            RichText::new(format!("📦 {}: {}", label, name))
                .color(colors::SUCCESS)
                .size(12.0),
        );
    } else {
        ui.label(
            RichText::new(format!("❌ {}: None", label))
                .color(colors::text_muted(ui.visuals()))
                .size(12.0),
        );
    }
}

// ── Model selection panel ───────────────────────────────────────────────────

pub fn render_model_panel(ui: &mut Ui, has_model_a: bool, has_model_b: bool) -> (bool, bool) {
    let mut clicked_a = false;
    let mut clicked_b = false;

    ui.horizontal(|ui| {
        if ui
            .button(
                RichText::new(if has_model_a {
                    "🔄 Change A"
                } else {
                    "📂 Select A"
                })
                .size(14.0),
            )
            .clicked()
        {
            clicked_a = true;
        }

        ui.add_space(8.0);

        if ui
            .button(
                RichText::new(if has_model_b {
                    "🔄 Change B"
                } else {
                    "📂 Select B"
                })
                .size(14.0),
            )
            .clicked()
        {
            clicked_b = true;
        }

        ui.add_space(10.0);

        ui.label(
            RichText::new("Select .gguf models to compare")
                .color(colors::text_muted(ui.visuals()))
                .size(13.0),
        );
    });

    (clicked_a, clicked_b)
}

// ── Text input ──────────────────────────────────────────────────────────────

pub fn render_text_input(
    ui: &mut Ui,
    text: &mut String,
    enabled: bool,
    height: f32,
    token_count: Option<usize>,
) -> bool {
    ui.add_space(12.0);

    ui.horizontal(|ui| {
        ui.label(
            RichText::new("📝 Input Text")
                .size(16.0)
                .color(colors::text_primary(ui.visuals())),
        );

        if let Some(count) = token_count {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("{} tokens", count))
                        .color(colors::text_muted(ui.visuals()))
                        .size(12.0),
                );
            });
        }
    });

    ui.add_space(4.0);

    let scroll_height = (height - 40.0).max(80.0);
    let mut changed = false;

    egui::ScrollArea::vertical()
        .id_salt("text_input_scroll")
        .max_height(scroll_height)
        .show(ui, |ui| {
            let response = ui.add(
                egui::TextEdit::multiline(text)
                    .desired_width(f32::INFINITY)
                    .desired_rows(6)
                    .font(FontId::monospace(14.0))
                    .hint_text("Paste your text here to analyze its perplexity…")
                    .interactive(enabled),
            );
            changed = response.changed();
        });

    changed
}

// ── Controls (analyze button + progress) ────────────────────────────────────

pub fn render_controls(
    ui: &mut Ui,
    can_analyze: bool,
    is_analyzing: bool,
    progress_a: Option<f32>,
    progress_b: Option<f32>,
) -> bool {
    ui.add_space(12.0);

    let mut clicked = false;
    ui.horizontal(|ui| {
        let label = if is_analyzing {
            "⏳ Analyzing…"
        } else {
            "🔍 Analyze"
        };

        if ui
            .add_enabled(
                can_analyze && !is_analyzing,
                egui::Button::new(RichText::new(label).size(18.0)).min_size(Vec2::new(140.0, 40.0)),
            )
            .clicked()
        {
            clicked = true;
        }

        ui.add_space(16.0);

        render_progress_bar(ui, "A", progress_a);
        render_progress_bar(ui, "B", progress_b);
    });
    clicked
}

fn render_progress_bar(ui: &mut Ui, label: &str, progress: Option<f32>) {
    if let Some(pct) = progress {
        ui.label(
            RichText::new(format!("{}: {:3.0}%", label, pct * 100.0))
                .font(FontId::monospace(12.0))
                .color(colors::text_muted(ui.visuals())),
        );
        let bar = egui::ProgressBar::new(pct).fill(colors::progress_bar_fill(ui.visuals()));
        ui.add_sized(Vec2::new(100.0, 16.0), bar);
        ui.add_space(8.0);
    }
}

// ── Results ─────────────────────────────────────────────────────────────────

pub fn render_results(
    ui: &mut Ui,
    result_a: Option<&AnalysisResult>,
    result_b: Option<&AnalysisResult>,
    model_name_a: Option<&str>,
    model_name_b: Option<&str>,
    height: f32,
    view_mode: &mut ViewMode,
    unified_color_mode: &mut UnifiedColorMode,
) {
    ui.add_space(16.0);
    ui.separator();
    ui.add_space(8.0);

    let both = result_a.is_some() && result_b.is_some();

    // Show view-mode segmented control when both models have results
    if both {
        ui.horizontal(|ui| {
            // Segmented control: Split | Unified
            ui.label(
                RichText::new("View:")
                    .size(12.0)
                    .color(colors::text_muted(ui.visuals())),
            );
            ui.add_space(4.0);

            let split_selected = *view_mode == ViewMode::Split;
            let unified_selected = *view_mode == ViewMode::Unified;

            if ui
                .selectable_label(split_selected, RichText::new("🔀 Split").size(12.0))
                .clicked()
            {
                *view_mode = ViewMode::Split;
            }
            if ui
                .selectable_label(unified_selected, RichText::new("⊞ Unified").size(12.0))
                .clicked()
            {
                *view_mode = ViewMode::Unified;
            }

            // Show color-mode combo when in Unified mode
            if *view_mode == ViewMode::Unified {
                ui.add_space(16.0);
                ui.label(
                    RichText::new("Color:")
                        .size(12.0)
                        .color(colors::text_muted(ui.visuals())),
                );
                egui::ComboBox::from_id_salt("unified_color_mode")
                    .selected_text(RichText::new(unified_color_mode.to_string()).size(12.0))
                    .width(130.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            unified_color_mode,
                            UnifiedColorMode::AvgRank,
                            "Average rank",
                        );
                        ui.selectable_value(
                            unified_color_mode,
                            UnifiedColorMode::AvgProbability,
                            "Average probability",
                        );
                        ui.selectable_value(
                            unified_color_mode,
                            UnifiedColorMode::RankDivergence,
                            "Divergence rank",
                        );
                        ui.selectable_value(
                            unified_color_mode,
                            UnifiedColorMode::ProbDivergence,
                            "Divergence probability",
                        );
                    });
            }
        });
        ui.add_space(4.0);
    }

    // Legend (varies by mode)
    if both && *view_mode == ViewMode::Unified {
        match *unified_color_mode {
            UnifiedColorMode::AvgProbability => render_prob_legend(ui),
            UnifiedColorMode::RankDivergence | UnifiedColorMode::ProbDivergence => {
                render_divergence_legend(ui)
            }
            UnifiedColorMode::AvgRank => render_legend(ui),
        }
    } else {
        render_legend(ui);
    }
    ui.add_space(12.0);

    if both {
        if *view_mode == ViewMode::Unified {
            render_unified_result(
                ui,
                result_a.unwrap(),
                result_b.unwrap(),
                model_name_a,
                model_name_b,
                height,
                *unified_color_mode,
            );
        } else {
            render_dual_results(
                ui,
                result_a.unwrap(),
                result_b.unwrap(),
                model_name_a,
                model_name_b,
                height,
            );
        }
    } else {
        let (result, name) = if let Some(r) = result_a {
            (r, model_name_a.unwrap_or("Model A"))
        } else {
            (result_b.unwrap(), model_name_b.unwrap_or("Model B"))
        };
        render_single_result(ui, result, name, height);
    }
}

fn render_dual_results(
    ui: &mut Ui,
    result_a: &AnalysisResult,
    result_b: &AnalysisResult,
    model_name_a: Option<&str>,
    model_name_b: Option<&str>,
    height: f32,
) {
    let label_a = model_name_a.unwrap_or("Model A");
    let label_b = model_name_b.unwrap_or("Model B");
    let scroll_height = (height - 120.0).max(100.0);

    egui::ScrollArea::vertical()
        .id_salt("results_dual_scroll")
        .max_height(scroll_height)
        .show(ui, |ui| {
            ui.columns(2, |columns| {
                columns[0].vertical(|ui| {
                    render_column_header(ui, label_a, colors::INFO);
                    render_stats_bar(ui, result_a);
                    ui.add_space(8.0);
                    render_analyzed_tokens(
                        ui,
                        &result_a.tokens,
                        Some(&result_b.tokens),
                        label_a,
                        label_b,
                    );
                });

                columns[1].vertical(|ui| {
                    render_column_header(ui, label_b, colors::WARNING);
                    render_stats_bar(ui, result_b);
                    ui.add_space(8.0);
                    render_analyzed_tokens(
                        ui,
                        &result_b.tokens,
                        Some(&result_a.tokens),
                        label_b,
                        label_a,
                    );
                });
            });
        });
}

fn render_single_result(ui: &mut Ui, result: &AnalysisResult, name: &str, height: f32) {
    render_column_header(ui, name, colors::INFO);
    ui.add_space(8.0);

    render_stats_bar(ui, result);
    ui.add_space(12.0);

    let scroll_height = (height - 160.0).max(100.0);
    egui::ScrollArea::vertical()
        .id_salt("results_single_scroll")
        .max_height(scroll_height)
        .show(ui, |ui| {
            render_analyzed_tokens(ui, &result.tokens, None, name, "");
        });
}

fn render_column_header(ui: &mut Ui, label: &str, color: Color32) {
    ui.label(
        RichText::new(format!("📦 {}", label))
            .strong()
            .size(14.0)
            .color(color),
    );
    ui.add_space(6.0);
}

/// Inline stats bar used in both single and dual views.
fn render_stats_bar(ui: &mut Ui, result: &AnalysisResult) {
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(format!(
                "⏱ {:.1}s",
                result.processing_time_ms as f32 / 1000.0
            ))
            .color(colors::text_muted(ui.visuals()))
            .size(12.0),
        );

        ui.add_space(10.0);

        ui.label(
            RichText::new(format!("PPL: {:.2}", result.perplexity()))
                .color(colors::WARNING)
                .size(12.0),
        )
        .on_hover_text("Perplexity (lower = more predictable)");

        ui.add_space(10.0);

        ui.label(
            RichText::new(format!("Entropy: {:.0}b", result.text_entropy()))
                .color(colors::ACCENT_PRIMARY)
                .size(12.0),
        )
        .on_hover_text("Information needed to reconstruct the text using this model");
    });
}

// ── Legend ───────────────────────────────────────────────────────────────────

fn render_legend(ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Legend (rank):").size(12.0));
        ui.add_space(8.0);

        legend_swatch(ui, colors::RANK_PERFECT, "1");
        legend_swatch(ui, colors::RANK_GOOD_START, "2-10");
        legend_swatch(ui, colors::RANK_MODERATE, "11-50");
        legend_swatch(ui, colors::RANK_POOR, "> 50");
    });
}

fn render_divergence_legend(ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Legend (divergence):").size(12.0));
        ui.add_space(8.0);

        legend_swatch(ui, colors::rank_divergence_color(1, 1), "Agree");
        legend_swatch(ui, colors::rank_divergence_color(1, 20), "Some divergence");
        legend_swatch(ui, colors::rank_divergence_color(1, 200), "Disagree");
    });
}

fn render_prob_legend(ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Legend (probability):").size(12.0));
        ui.add_space(8.0);

        legend_swatch(ui, colors::prob_to_color(0.75), ">50%");
        legend_swatch(ui, colors::prob_to_color(0.25), "10-50%");
        legend_swatch(ui, colors::prob_to_color(0.05), "1-10%");
        legend_swatch(ui, colors::prob_to_color(0.005), "<1%");
    });
}

fn legend_swatch(ui: &mut Ui, color: Color32, label: &str) {
    let rect = ui.allocate_space(Vec2::new(16.0, 16.0));
    ui.painter().rect_filled(rect.1, 2.0, color);
    ui.label(RichText::new(label).size(11.0));
    ui.add_space(8.0);
}

// ── Token rendering ─────────────────────────────────────────────────────────

fn render_analyzed_tokens(
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
    let display_text = token.text.replace('\n', "↵\n").replace('\t', "→");

    let response = ui.add(
        egui::Label::new(
            RichText::new(&display_text)
                .color(Color32::BLACK)
                .background_color(bg_color)
                .size(14.0)
                .family(egui::FontFamily::Monospace),
        )
        .sense(egui::Sense::hover()),
    );

    response.on_hover_ui(|ui| {
        ui.set_max_width(340.0);
        ui.set_min_width(340.0);

        // Token text header
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(token.text.clone())
                    .strong()
                    .monospace()
                    .size(15.0)
                    .background_color(colors::secondary_bg(ui.visuals())),
            );
        });

        ui.add_space(6.0);

        if let Some(other) = other_token {
            render_comparison_tooltip(ui, token, other, self_label, other_label);
        } else {
            render_single_tooltip(ui, token);
        }
    });
}

fn render_comparison_tooltip(
    ui: &mut Ui,
    token: &AnalyzedToken,
    other: &AnalyzedToken,
    self_label: &str,
    other_label: &str,
) {
    ui.separator();
    ui.add_space(4.0);

    // Rank + probability comparison grid
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

    // Side-by-side predictions
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

// ── Unified view rendering ──────────────────────────────────────────────────

fn render_unified_result(
    ui: &mut Ui,
    result_a: &AnalysisResult,
    result_b: &AnalysisResult,
    model_name_a: Option<&str>,
    model_name_b: Option<&str>,
    height: f32,
    color_mode: UnifiedColorMode,
) {
    let label_a = model_name_a.unwrap_or("Model A");
    let label_b = model_name_b.unwrap_or("Model B");

    let scroll_height = (height - 140.0).max(100.0);
    egui::ScrollArea::vertical()
        .id_salt("results_unified_scroll")
        .max_height(scroll_height)
        .auto_shrink(false)
        .show(ui, |ui| {
            render_unified_tokens(
                ui,
                &result_a.tokens,
                &result_b.tokens,
                label_a,
                label_b,
                color_mode,
            );
        });
}

fn render_unified_tokens(
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

            // Determine display text from model A (primary), fallback to B
            let display_token = tok_a.or(tok_b).unwrap();
            let display_text = display_token.text.replace('\n', "↵\n").replace('\t', "→");

            // Determine background color
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

            let response = ui.add(
                egui::Label::new(
                    RichText::new(&display_text)
                        .color(Color32::BLACK)
                        .background_color(bg_color)
                        .size(14.0)
                        .family(egui::FontFamily::Monospace),
                )
                .sense(egui::Sense::hover()),
            );

            response.on_hover_ui(|ui| {
                ui.set_max_width(320.0);
                ui.set_min_width(320.0);

                // Token text header
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(display_token.text.clone())
                            .strong()
                            .monospace()
                            .size(15.0)
                            .background_color(colors::secondary_bg(ui.visuals())),
                    );
                });

                ui.add_space(6.0);

                if let (Some(a), Some(b)) = (tok_a, tok_b) {
                    render_comparison_tooltip(ui, a, b, label_a, label_b);
                } else if let Some(t) = tok_a.or(tok_b) {
                    render_single_tooltip(ui, t);
                }
            });
        }
    });
}

// ── Empty state & error ─────────────────────────────────────────────────────

pub fn render_empty_state(ui: &mut Ui, has_any_model: bool) {
    ui.add_space(40.0);

    ui.vertical_centered(|ui| {
        ui.label(RichText::new("🔮").size(64.0));
        ui.add_space(16.0);

        let message = if has_any_model {
            "Enter some text and click 'Analyze'"
        } else {
            "Select a model to get started"
        };
        ui.label(
            RichText::new(message)
                .size(18.0)
                .color(colors::text_muted(ui.visuals())),
        );

        ui.add_space(8.0);

        ui.label(
            RichText::new("Tokens will be highlighted based on how predictable they are")
                .size(14.0)
                .color(colors::text_very_muted(ui.visuals())),
        );
    });
}

pub fn render_error(ui: &mut Ui, error: &str) {
    ui.add_space(12.0);

    egui::Frame::none()
        .fill(colors::error_bg(ui.visuals()))
        .rounding(8.0)
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("❌").size(18.0));
                ui.add_space(8.0);
                ui.label(RichText::new(error).color(colors::ERROR).size(14.0));
            });
        });
}
