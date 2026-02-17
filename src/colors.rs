use egui::{Color32, Visuals};

pub const RANK_PERFECT: Color32 = Color32::from_rgb(143, 188, 159);
pub const RANK_GOOD_START: Color32 = Color32::from_rgb(216, 195, 165);
pub const RANK_MODERATE: Color32 = Color32::from_rgb(210, 160, 146);
pub const RANK_POOR: Color32 = Color32::from_rgb(192, 132, 132);
pub const RANK_VERY_POOR: Color32 = Color32::from_rgb(164, 112, 120);

pub const ACCENT_PRIMARY: Color32 = Color32::from_rgb(164, 145, 194);
pub const SUCCESS: Color32 = Color32::from_rgb(100, 161, 115);
pub const WARNING: Color32 = Color32::from_rgb(204, 152, 88);
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

fn interpolate_color(start: Color32, end: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgb(
        (start.r() as f32 + (end.r() as f32 - start.r() as f32) * t) as u8,
        (start.g() as f32 + (end.g() as f32 - start.g() as f32) * t) as u8,
        (start.b() as f32 + (end.b() as f32 - start.b() as f32) * t) as u8,
    )
}
