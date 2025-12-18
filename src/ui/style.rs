//! UI styling and theming

use egui::{Context, Style, Visuals, Color32, Rounding, Stroke};

/// Apply custom dark theme
pub fn apply_dark_theme(ctx: &Context) {
    let mut style = (*ctx.style()).clone();
    
    // Dark theme colors - inspired by One Dark
    let bg_dark = Color32::from_rgb(30, 33, 39);
    let bg_medium = Color32::from_rgb(40, 44, 52);
    let bg_light = Color32::from_rgb(50, 55, 65);
    let accent = Color32::from_rgb(97, 175, 239);
    let text_primary = Color32::from_rgb(220, 223, 228);
    let text_secondary = Color32::from_rgb(150, 155, 165);

    style.visuals.dark_mode = true;
    style.visuals.panel_fill = bg_dark;
    style.visuals.window_fill = bg_medium;
    style.visuals.widgets.noninteractive.bg_fill = bg_medium;
    style.visuals.widgets.inactive.bg_fill = bg_light;
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(60, 65, 75);
    style.visuals.widgets.active.bg_fill = accent;
    style.visuals.selection.bg_fill = accent.linear_multiply(0.3);
    style.visuals.override_text_color = Some(text_primary);
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, text_secondary);
    
    // Rounded corners
    style.visuals.window_rounding = Rounding::same(8.0);
    style.visuals.widgets.noninteractive.rounding = Rounding::same(6.0);
    style.visuals.widgets.inactive.rounding = Rounding::same(6.0);
    style.visuals.widgets.hovered.rounding = Rounding::same(6.0);
    style.visuals.widgets.active.rounding = Rounding::same(6.0);

    ctx.set_style(style);
}

/// Colors for different UI elements
pub struct AppColors;

impl AppColors {
    pub fn success() -> Color32 {
        Color32::from_rgb(152, 195, 121)
    }
    
    pub fn warning() -> Color32 {
        Color32::from_rgb(229, 192, 123)
    }
    
    pub fn error() -> Color32 {
        Color32::from_rgb(224, 108, 117)
    }
    
    pub fn info() -> Color32 {
        Color32::from_rgb(97, 175, 239)
    }
    
    pub fn muted() -> Color32 {
        Color32::from_rgb(150, 155, 165)
    }
    
    pub fn bg_panel() -> Color32 {
        Color32::from_rgb(40, 44, 52)
    }
}

