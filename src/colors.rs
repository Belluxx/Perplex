use egui::{Color32, Visuals};

pub const RANK_PERFECT: Color32 = Color32::from_rgb(143, 188, 159);
pub const RANK_GOOD_START: Color32 = Color32::from_rgb(216, 195, 165);
pub const RANK_MODERATE: Color32 = Color32::from_rgb(210, 160, 146);
pub const RANK_POOR: Color32 = Color32::from_rgb(192, 132, 132);
pub const RANK_VERY_POOR: Color32 = Color32::from_rgb(164, 112, 120);

pub const ACCENT_PRIMARY: Color32 = Color32::from_rgb(139, 118, 173);
pub const SUCCESS: Color32 = Color32::from_rgb(100, 161, 115);
pub const WARNING: Color32 = Color32::from_rgb(184, 122, 68);
pub const ERROR: Color32 = Color32::from_rgb(205, 115, 115);
pub const INFO: Color32 = Color32::from_rgb(124, 156, 191);

fn themed(visuals: &Visuals, dark: Color32, light: Color32) -> Color32 {
    if visuals.dark_mode {
        dark
    } else {
        light
    }
}

pub fn secondary_bg(visuals: &Visuals) -> Color32 {
    themed(
        visuals,
        Color32::from_rgb(50, 50, 50),
        Color32::from_rgb(210, 210, 210),
    )
}

pub fn text_primary(visuals: &Visuals) -> Color32 {
    themed(
        visuals,
        Color32::from_rgb(225, 227, 232),
        Color32::from_rgb(38, 40, 45),
    )
}

pub fn text_muted(visuals: &Visuals) -> Color32 {
    themed(
        visuals,
        Color32::from_rgb(148, 152, 162),
        Color32::from_rgb(100, 104, 114),
    )
}

pub fn text_very_muted(visuals: &Visuals) -> Color32 {
    themed(
        visuals,
        Color32::from_rgb(108, 112, 122),
        Color32::from_rgb(130, 134, 144),
    )
}

pub fn error_bg(visuals: &Visuals) -> Color32 {
    themed(
        visuals,
        Color32::from_rgb(48, 32, 36),
        Color32::from_rgb(255, 235, 238),
    )
}

pub fn progress_bar_fill(visuals: &Visuals) -> Color32 {
    themed(
        visuals,
        Color32::from_rgb(143, 143, 143),
        Color32::from_rgb(94, 94, 94),
    )
}

pub fn rank_to_color(rank: usize) -> Color32 {
    match rank {
        0 | 1 => RANK_PERFECT,
        2..=10 => interpolate_color(RANK_PERFECT, RANK_GOOD_START, (rank - 1) as f32 / 9.0),
        11..=50 => interpolate_color(RANK_GOOD_START, RANK_MODERATE, (rank - 10) as f32 / 40.0),
        51..=100 => interpolate_color(RANK_MODERATE, RANK_POOR, (rank - 50) as f32 / 50.0),
        _ => interpolate_color(
            RANK_POOR,
            RANK_VERY_POOR,
            ((rank - 100) as f32 / 200.0).min(1.0),
        ),
    }
}

pub fn interpolate_color(start: Color32, end: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgb(
        (start.r() as f32 + (end.r() as f32 - start.r() as f32) * t) as u8,
        (start.g() as f32 + (end.g() as f32 - start.g() as f32) * t) as u8,
        (start.b() as f32 + (end.b() as f32 - start.b() as f32) * t) as u8,
    )
}

/// Color for the "Average Rank" unified mode: blend from the average rank of both models.
pub fn average_rank_color(rank_a: usize, rank_b: usize) -> Color32 {
    let avg = (rank_a + rank_b) / 2;
    rank_to_color(avg)
}

/// Color for the "Average Probability" unified mode.
/// Uses the rank color palette mapped from average probability.
pub fn average_prob_color(prob_a: f32, prob_b: f32) -> Color32 {
    let avg = (prob_a + prob_b) / 2.0;
    prob_to_color(avg)
}

/// Map a probability (0.0 - 1.0) to a color using the rank palette.
/// High probability → green (RANK_PERFECT), low → red (RANK_VERY_POOR).
pub fn prob_to_color(prob: f32) -> Color32 {
    let p = prob.clamp(0.0, 1.0);
    if p > 0.5 {
        interpolate_color(RANK_GOOD_START, RANK_PERFECT, (p - 0.5) * 2.0)
    } else if p > 0.1 {
        interpolate_color(RANK_MODERATE, RANK_GOOD_START, (p - 0.1) / 0.4)
    } else if p > 0.01 {
        interpolate_color(RANK_POOR, RANK_MODERATE, (p - 0.01) / 0.09)
    } else {
        interpolate_color(RANK_VERY_POOR, RANK_POOR, (p / 0.01).min(1.0))
    }
}

// Divergence palette
const DIVERGE_AGREE: Color32 = Color32::from_rgb(152, 190, 210); // matte light blue
const DIVERGE_NEUTRAL: Color32 = Color32::from_rgb(195, 185, 195); // desaturated lavender
const DIVERGE_DISAGREE: Color32 = Color32::from_rgb(195, 110, 110); // matte red

/// Helper: map a 0..1 divergence factor through the agree→disagree palette.
fn divergence_gradient(t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        interpolate_color(DIVERGE_AGREE, DIVERGE_NEUTRAL, t * 2.0)
    } else {
        interpolate_color(DIVERGE_NEUTRAL, DIVERGE_DISAGREE, (t - 0.5) * 2.0)
    }
}

/// Color for the "Rank Divergence" unified mode.
/// Green when ranks agree, red when they disagree.
pub fn rank_divergence_color(rank_a: usize, rank_b: usize) -> Color32 {
    let diff = (rank_a as f32 - rank_b as f32).abs();
    // Log scale: log(1 + diff) / log(1 + 200) capped at 1.0
    let t = ((1.0 + diff).ln() / (1.0 + 200.0_f32).ln()).min(1.0);
    divergence_gradient(t)
}

/// Color for the "Probability Divergence" unified mode.
/// Green when probabilities are close, red when they differ.
pub fn prob_divergence_color(prob_a: f32, prob_b: f32) -> Color32 {
    let diff = (prob_a - prob_b).abs();
    // Linear: diff of 0 → agree, diff of 0.5+ → fully disagree
    let t = (diff * 2.0).min(1.0);
    divergence_gradient(t)
}
