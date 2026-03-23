use crate::analysis::AnalysisResult;
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

#[derive(Default)]
pub struct HeaderAction {
    pub settings: bool,
    pub eject_a: bool,
    pub eject_b: bool,
}

pub fn render_header(
    ui: &mut Ui,
    model_path_a: Option<&str>,
    model_path_b: Option<&str>,
    is_loading_a: bool,
    is_loading_b: bool,
) -> HeaderAction {
    let mut action = HeaderAction::default();
    ui.horizontal(|ui| {
        ui.heading(
            RichText::new("🔮 Perplex")
                .size(28.0)
                .color(colors::ACCENT_PRIMARY),
        );

        ui.add_space(20.0);

        ui.vertical(|ui| {
            if render_model_badge(ui, colors::INFO, model_path_a, is_loading_a) {
                action.eject_a = true;
            }
            ui.add_space(2.0);
            if render_model_badge(ui, colors::WARNING, model_path_b, is_loading_b) {
                action.eject_b = true;
            }
        });

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(egui::Button::new(RichText::new("⚙").size(18.0)))
                .clicked()
            {
                action.settings = true;
            }
        });
    });

    ui.add_space(8.0);
    ui.separator();
    action
}

/// Returns true if the eject button was clicked.
fn render_model_badge(ui: &mut Ui, color: Color32, path: Option<&str>, is_loading: bool) -> bool {
    let mut ejected = false;
    if is_loading {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(RichText::new("Loading…").color(color).size(12.0));
        });
    } else if let Some(p) = path {
        let name = crate::model_name_from_path(Some(p)).unwrap_or(p);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("📦 {}", name))
                    .color(color)
                    .size(12.0),
            );
            if ui
                .add(
                    egui::Button::new(RichText::new("⏏").size(12.0))
                        .frame(false),
                )
                .on_hover_text("Eject model")
                .clicked()
            {
                ejected = true;
            }
        });
    } else {
        ui.label(
            RichText::new("❌ None")
                .color(colors::text_muted(ui.visuals()))
                .size(12.0),
        );
    }
    ejected
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
    token_count_a: Option<usize>,
    token_count_b: Option<usize>,
) -> bool {
    ui.add_space(12.0);

    ui.horizontal(|ui| {
        ui.label(
            RichText::new("📝 Input Text")
                .size(16.0)
                .color(colors::text_primary(ui.visuals())),
        );

        let has_any = token_count_a.is_some() || token_count_b.is_some();
        if has_any {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Right-to-left layout reverses visual order, so add items
                // in reverse so they appear left-to-right on screen.
                match (token_count_a, token_count_b) {
                    (Some(a), Some(b)) if a == b => {
                        ui.label(
                            RichText::new(format!("{} tokens", a))
                                .color(colors::text_muted(ui.visuals()))
                                .size(12.0),
                        );
                    }
                    (Some(a), Some(b)) => {
                        // Show both counts, attributed by color
                        ui.label(
                            RichText::new(format!("{}", b))
                                .color(colors::WARNING)
                                .size(12.0),
                        );
                        ui.label(
                            RichText::new("/")
                                .color(colors::text_muted(ui.visuals()))
                                .size(12.0),
                        );
                        ui.label(
                            RichText::new(format!("{}", a))
                                .color(colors::INFO)
                                .size(12.0),
                        );
                        ui.label(
                            RichText::new("tokens:")
                                .color(colors::text_muted(ui.visuals()))
                                .size(12.0),
                        );
                    }
                    (Some(count), None) | (None, Some(count)) => {
                        ui.label(
                            RichText::new(format!("{} tokens", count))
                                .color(colors::text_muted(ui.visuals()))
                                .size(12.0),
                        );
                    }
                    (None, None) => unreachable!(),
                }
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

/// Check whether two analysis results have compatible tokenizers by comparing
/// their token text sequences. When the tokenizers match, every token at the
/// same index covers the same piece of text, which is required for the unified
/// view and for index-based cross-model comparison in tooltips.
fn tokenizers_match(a: &AnalysisResult, b: &AnalysisResult) -> bool {
    if a.tokens.len() != b.tokens.len() {
        return false;
    }
    a.tokens
        .iter()
        .zip(b.tokens.iter())
        .all(|(ta, tb)| ta.text == tb.text)
}

fn render_tokenizer_warning(ui: &mut Ui) {
    egui::Frame::none()
        .fill(colors::warning_bg(ui.visuals()))
        .rounding(8.0)
        .inner_margin(10.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("⚠").size(16.0));
                ui.add_space(6.0);
                ui.label(
                    RichText::new(
                        "The two models use different tokenizers, \
                         unified view is disabled and token comparison is unavailable.",
                    )
                    .color(colors::WARNING)
                    .size(12.0),
                );
            });
        });
    ui.add_space(4.0);
}

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

    let tok_match = if both {
        tokenizers_match(result_a.unwrap(), result_b.unwrap())
    } else {
        false
    };

    // Force split view when tokenizers differ
    if both && !tok_match && *view_mode == ViewMode::Unified {
        *view_mode = ViewMode::Split;
    }

    if both {
        // Tokenizer mismatch warning
        if !tok_match {
            render_tokenizer_warning(ui);
        }

        ui.horizontal(|ui| {
            // Segmented control: Split | Unified
            ui.label(
                RichText::new("View:")
                    .size(12.0)
                    .color(colors::text_muted(ui.visuals())),
            );
            ui.add_space(4.0);

            let split_selected = *view_mode == ViewMode::Split;

            if ui
                .selectable_label(split_selected, RichText::new("🔀 Split").size(12.0))
                .clicked()
            {
                *view_mode = ViewMode::Split;
            }

            // Only allow unified view when tokenizers match
            if tok_match {
                let unified_selected = *view_mode == ViewMode::Unified;
                if ui
                    .selectable_label(unified_selected, RichText::new("⊞ Unified").size(12.0))
                    .clicked()
                {
                    *view_mode = ViewMode::Unified;
                }
            } else {
                ui.add_enabled_ui(false, |ui| {
                    ui.selectable_label(false, RichText::new("⊞ Unified").size(12.0))
                        .on_disabled_hover_text("Unified view requires matching tokenizers");
                });
            }

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
                tok_match,
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
    tokenizers_compatible: bool,
) {
    let label_a = model_name_a.unwrap_or("Model A");
    let label_b = model_name_b.unwrap_or("Model B");
    let scroll_height = (height - 120.0).max(100.0);

    // When tokenizers differ, don't pass the other model's tokens for
    // index-based comparison — the indices don't correspond to the same text.
    let other_b = if tokenizers_compatible {
        Some(result_b.tokens.as_slice())
    } else {
        None
    };
    let other_a = if tokenizers_compatible {
        Some(result_a.tokens.as_slice())
    } else {
        None
    };

    egui::ScrollArea::vertical()
        .id_salt("results_dual_scroll")
        .max_height(scroll_height)
        .show(ui, |ui| {
            ui.columns(2, |columns| {
                columns[0].vertical(|ui| {
                    render_column_header(ui, label_a, colors::INFO);
                    render_stats_bar(ui, result_a);
                    ui.add_space(8.0);
                    crate::ui_tokens::render_analyzed_tokens(
                        ui,
                        &result_a.tokens,
                        other_b,
                        label_a,
                        label_b,
                    );
                });

                columns[1].vertical(|ui| {
                    render_column_header(ui, label_b, colors::WARNING);
                    render_stats_bar(ui, result_b);
                    ui.add_space(8.0);
                    crate::ui_tokens::render_analyzed_tokens(
                        ui,
                        &result_b.tokens,
                        other_a,
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
            crate::ui_tokens::render_analyzed_tokens(ui, &result.tokens, None, name, "");
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
            RichText::new(format!("Entropy: {:.0} bits", result.text_entropy()))
                .color(colors::ACCENT_PRIMARY)
                .size(12.0),
        )
        .on_hover_text("Information needed to reconstruct the text using this model");
    });
}

// ── Legend ───────────────────────────────────────────────────────────────────

fn render_legend_row(ui: &mut Ui, title: &str, swatches: &[(Color32, &str)]) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).size(12.0));
        ui.add_space(8.0);
        for &(color, label) in swatches {
            legend_swatch(ui, color, label);
        }
    });
}

fn render_legend(ui: &mut Ui) {
    render_legend_row(ui, "Legend (rank):", &[
        (colors::RANK_PERFECT, "1"),
        (colors::RANK_GOOD_START, "2-10"),
        (colors::RANK_MODERATE, "11-50"),
        (colors::RANK_POOR, "> 50"),
    ]);
}

fn render_divergence_legend(ui: &mut Ui) {
    render_legend_row(ui, "Legend (divergence):", &[
        (colors::rank_divergence_color(1, 1), "Agree"),
        (colors::rank_divergence_color(1, 20), "Some divergence"),
        (colors::rank_divergence_color(1, 200), "Disagree"),
    ]);
}

fn render_prob_legend(ui: &mut Ui) {
    render_legend_row(ui, "Legend (probability):", &[
        (colors::prob_to_color(0.75), ">50%"),
        (colors::prob_to_color(0.25), "10-50%"),
        (colors::prob_to_color(0.05), "1-10%"),
        (colors::prob_to_color(0.005), "<1%"),
    ]);
}

fn legend_swatch(ui: &mut Ui, color: Color32, label: &str) {
    let rect = ui.allocate_space(Vec2::new(16.0, 16.0));
    ui.painter().rect_filled(rect.1, 2.0, color);
    ui.label(RichText::new(label).size(11.0));
    ui.add_space(8.0);
}

// ── Token rendering (delegated to ui_tokens) ────────────────────────────────

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
            crate::ui_tokens::render_unified_tokens(
                ui,
                &result_a.tokens,
                &result_b.tokens,
                label_a,
                label_b,
                color_mode,
            );
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
