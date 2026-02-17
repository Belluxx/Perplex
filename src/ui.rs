use crate::colors;
use crate::utils::{AnalysisResult, AnalyzedToken};
use egui::{Color32, FontId, RichText, Ui, Vec2};

pub fn render_header(ui: &mut Ui, model_path: Option<&str>, is_loading: bool) {
    ui.horizontal(|ui| {
        ui.heading(
            RichText::new("🔮 Perplex")
                .size(28.0)
                .color(colors::ACCENT_PRIMARY),
        );

        ui.add_space(20.0);

        if is_loading {
            ui.spinner();
            ui.label(RichText::new("Loading model...").color(colors::text_primary(ui.visuals())));
        } else if let Some(path) = model_path {
            let file_name = std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path);
            ui.label(
                RichText::new(format!("📦 {}", file_name))
                    .color(colors::SUCCESS)
                    .size(14.0),
            );
        } else {
            ui.label(RichText::new("❌ No model loaded").color(colors::text_muted(ui.visuals())));
        }
    });

    ui.add_space(8.0);
    ui.separator();
}

pub fn render_model_panel(ui: &mut Ui, has_model: bool) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        if ui
            .button(
                RichText::new(if has_model {
                    "🔄 Change Model"
                } else {
                    "📂 Select Model"
                })
                .size(16.0),
            )
            .clicked()
        {
            clicked = true;
        }

        ui.add_space(10.0);

        ui.label(
            RichText::new("Select a .gguf model file to begin analysis")
                .color(colors::text_muted(ui.visuals()))
                .size(13.0),
        );
    });
    clicked
}

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
            let text_edit = egui::TextEdit::multiline(text)
                .desired_width(f32::INFINITY)
                .desired_rows(6)
                .font(FontId::monospace(14.0))
                .hint_text("Paste your text here to analyze its perplexity...")
                .interactive(enabled);

            let response = ui.add(text_edit);
            if response.changed() {
                changed = true;
            }
        });

    changed
}

pub fn render_controls(
    ui: &mut Ui,
    can_analyze: bool,
    is_analyzing: bool,
    progress: Option<f32>,
) -> bool {
    ui.add_space(12.0);

    let mut clicked = false;
    ui.horizontal(|ui| {
        let button_text = if is_analyzing {
            "⏳ Analyzing..."
        } else {
            "🔍 Analyze"
        };

        if ui
            .add_enabled(
                can_analyze && !is_analyzing,
                egui::Button::new(RichText::new(button_text).size(18.0))
                    .min_size(Vec2::new(140.0, 40.0)),
            )
            .clicked()
        {
            clicked = true;
        }

        ui.add_space(16.0);

        if let Some(pct) = progress {
            ui.label(
                RichText::new(format!("{:3.0}%", pct * 100.0))
                    .font(FontId::monospace(14.0))
                    .color(colors::text_muted(ui.visuals())),
            );
            ui.add_space(8.0);
            let progress_bar =
                egui::ProgressBar::new(pct).fill(colors::progress_bar_fill(ui.visuals()));
            ui.add_sized(Vec2::new(150.0, 20.0), progress_bar);
        }
    });
    clicked
}

pub fn render_results(ui: &mut Ui, result: &AnalysisResult, height: f32) {
    ui.add_space(16.0);
    ui.separator();
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.label(
            RichText::new("📊 Analysis Results")
                .size(16.0)
                .color(colors::text_primary(ui.visuals())),
        );

        ui.add_space(20.0);

        ui.label(
            RichText::new(format!("⏱ {}s", result.processing_time_ms / 1000))
                .color(colors::text_muted(ui.visuals()))
                .size(12.0),
        );

        ui.add_space(10.0);

        ui.label(
            RichText::new(format!("📈 Avg Rank: {:.0}", result.average_rank()))
                .color(colors::INFO)
                .size(12.0),
        );

        ui.add_space(10.0);

        ui.label(
            RichText::new(format!(
                "✅ Exact: {:.0}%",
                result.exact_prediction_percentage()
            ))
            .color(colors::SUCCESS)
            .size(12.0),
        );

        ui.add_space(10.0);

        ui.label(
            RichText::new(format!("❓ Perplexity: {:.2}", result.perplexity()))
                .color(colors::WARNING)
                .size(12.0),
        )
        .on_hover_text("Perplexity (lower means MORE predictable by the model)");
    });

    ui.add_space(12.0);

    render_legend(ui);

    ui.add_space(12.0);

    let scroll_height = (height - 100.0).max(100.0);
    egui::ScrollArea::vertical()
        .id_salt("results_scroll")
        .max_height(scroll_height)
        .show(ui, |ui| {
            render_analyzed_tokens(ui, &result.tokens);
        });
}

fn render_legend(ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Legend:").size(12.0));
        ui.add_space(8.0);

        let green_rect = ui.allocate_space(Vec2::new(16.0, 16.0));
        ui.painter()
            .rect_filled(green_rect.1, 2.0, colors::RANK_PERFECT);
        ui.label(RichText::new("Rank 1").size(11.0));

        ui.add_space(8.0);

        let yellow_rect = ui.allocate_space(Vec2::new(16.0, 16.0));
        ui.painter()
            .rect_filled(yellow_rect.1, 2.0, colors::RANK_GOOD_START);
        ui.label(RichText::new("Rank 2-10").size(11.0));

        ui.add_space(8.0);

        let orange_rect = ui.allocate_space(Vec2::new(16.0, 16.0));
        ui.painter()
            .rect_filled(orange_rect.1, 2.0, colors::RANK_MODERATE);
        ui.label(RichText::new("Rank 11-50").size(11.0));

        ui.add_space(8.0);

        let red_rect = ui.allocate_space(Vec2::new(16.0, 16.0));
        ui.painter().rect_filled(red_rect.1, 2.0, colors::RANK_POOR);
        ui.label(RichText::new("Rank > 50").size(11.0));
    });
}

fn render_analyzed_tokens(ui: &mut Ui, tokens: &[AnalyzedToken]) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, 4.0);

        for token in tokens {
            render_single_token(ui, token);
        }
    });
}

fn render_single_token(ui: &mut Ui, token: &AnalyzedToken) {
    let bg_color = token.get_color();

    let text_color = if is_light_color(bg_color) {
        colors::TEXT_DARK
    } else {
        colors::TEXT_WHITE
    };

    let response = ui.add(
        egui::Label::new(
            RichText::new(&token.display_text)
                .color(text_color)
                .background_color(bg_color)
                .size(14.0)
                .family(egui::FontFamily::Monospace),
        )
        .sense(egui::Sense::hover()),
    );

    response.on_hover_ui(|ui| {
        ui.set_max_width(200.0);

        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            // The token text should have a grey background
            ui.label(
                RichText::new(token.text.clone())
                    .strong()
                    .monospace()
                    .background_color(colors::secondary_bg(ui.visuals())),
            );
            ui.label(RichText::new(format!("(Rank: {})", token.rank)));
        });

        if !token.top_predictions.is_empty() {
            ui.add_space(8.0);
            ui.label(RichText::new("Top Predictions:").strong());
            for (i, (pred_text, prob)) in token.top_predictions.iter().enumerate() {
                let display_pred = pred_text.replace('\n', "↵").replace('\t', "→");
                ui.horizontal(|ui| {
                    ui.label(format!("{}.", i + 1));
                    ui.monospace(&display_pred);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if *prob < 0.01 {
                            ui.label("<1%");
                        } else {
                            ui.label(format!("{:.0}%", prob * 100.0));
                        }
                    });
                });
            }
        }
    });
}

fn is_light_color(color: Color32) -> bool {
    let luminance = 0.299 * color.r() as f32 + 0.587 * color.g() as f32 + 0.114 * color.b() as f32;
    luminance > 128.0
}

pub fn render_empty_state(ui: &mut Ui, has_model: bool) {
    ui.add_space(40.0);

    ui.vertical_centered(|ui| {
        ui.label(RichText::new("🔮").size(64.0));

        ui.add_space(16.0);

        let message = if has_model {
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
            RichText::new(
                "Tokens will be highlighted based on how predictable they are by the LLM",
            )
            .size(14.0)
            .color(colors::text_very_muted(ui.visuals())),
        );
    });
}

pub fn render_error(ui: &mut Ui, error: &str) {
    ui.add_space(12.0);

    let error_bg = colors::error_bg(ui.visuals());
    egui::Frame::none()
        .fill(error_bg)
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
