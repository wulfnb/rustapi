//! Design tokens and the egui themes built from them.
//!
//! Colour roles, state layers, shapes and type scale follow Material 3. The
//! primary colour is the ember amber of the app icon. Both a dark and a light
//! scheme are registered with egui, so "follow system" switches live.

use egui::{
    epaint::Shadow, vec2, Color32, Context, FontFamily, FontId, Margin, Rounding, Stroke, Style,
    TextStyle, Theme, ThemePreference,
};
use rustapi::storage::ThemeChoice;

/// Material 3 colour roles for one scheme.
pub struct Tokens {
    pub surface: Color32,
    pub surface_container_low: Color32,
    pub surface_container: Color32,
    pub surface_container_high: Color32,
    pub surface_container_highest: Color32,
    /// Text-field fill.
    pub field: Color32,
    /// Pressed button fill.
    pub pressed: Color32,
    pub on_surface: Color32,
    pub on_surface_variant: Color32,
    pub outline: Color32,
    pub outline_variant: Color32,
    pub primary: Color32,
    pub on_primary: Color32,
    pub secondary_container: Color32,
    pub error: Color32,
    pub error_container: Color32,
    pub on_error_container: Color32,
    pub success: Color32,
    pub warning: Color32,
}

pub const DARK: Tokens = Tokens {
    surface: Color32::from_rgb(22, 23, 28),
    surface_container_low: Color32::from_rgb(27, 28, 34),
    surface_container: Color32::from_rgb(32, 33, 40),
    surface_container_high: Color32::from_rgb(40, 41, 49),
    surface_container_highest: Color32::from_rgb(51, 52, 61),
    field: Color32::from_rgb(17, 18, 22),
    pressed: Color32::from_rgb(62, 63, 74),
    on_surface: Color32::from_rgb(230, 226, 232),
    on_surface_variant: Color32::from_rgb(172, 169, 180),
    outline: Color32::from_rgb(120, 119, 132),
    outline_variant: Color32::from_rgb(52, 53, 62),
    primary: Color32::from_rgb(255, 160, 64),
    on_primary: Color32::from_rgb(43, 23, 0),
    secondary_container: Color32::from_rgb(66, 44, 24),
    error: Color32::from_rgb(255, 180, 171),
    error_container: Color32::from_rgb(105, 20, 24),
    on_error_container: Color32::from_rgb(255, 218, 214),
    success: Color32::from_rgb(143, 209, 138),
    warning: Color32::from_rgb(242, 194, 107),
};

pub const LIGHT: Tokens = Tokens {
    surface: Color32::from_rgb(248, 247, 250),
    surface_container_low: Color32::from_rgb(241, 240, 244),
    surface_container: Color32::from_rgb(235, 234, 239),
    surface_container_high: Color32::from_rgb(227, 226, 232),
    surface_container_highest: Color32::from_rgb(217, 216, 224),
    field: Color32::from_rgb(255, 255, 255),
    pressed: Color32::from_rgb(204, 203, 214),
    on_surface: Color32::from_rgb(28, 27, 31),
    on_surface_variant: Color32::from_rgb(73, 69, 79),
    outline: Color32::from_rgb(121, 116, 126),
    outline_variant: Color32::from_rgb(202, 196, 208),
    primary: Color32::from_rgb(148, 72, 0),
    on_primary: Color32::from_rgb(255, 255, 255),
    secondary_container: Color32::from_rgb(255, 221, 189),
    error: Color32::from_rgb(186, 26, 26),
    error_container: Color32::from_rgb(255, 218, 214),
    on_error_container: Color32::from_rgb(65, 0, 2),
    success: Color32::from_rgb(36, 112, 44),
    warning: Color32::from_rgb(130, 82, 0),
};

/// Hover and pressed state-layer opacities.
pub const HOVER_LAYER: f32 = 0.08;
pub const PRESSED_LAYER: f32 = 0.12;

/// Corner radii: small widgets, containers, dialogs.
pub const RADIUS_SM: f32 = 6.0;
pub const RADIUS_MD: f32 = 8.0;
pub const RADIUS_LG: f32 = 12.0;

/// `color` at `opacity`, for state layers and tinted containers.
pub fn layer(color: Color32, opacity: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), (opacity * 255.0) as u8)
}

/// The token set of the theme `ctx` is currently showing.
pub fn tokens(ctx: &Context) -> &'static Tokens {
    match ctx.theme() {
        Theme::Dark => &DARK,
        Theme::Light => &LIGHT,
    }
}

/// An accent such as an HTTP method or status colour, darkened on the light
/// theme so it keeps text contrast (the source colours are tuned for dark).
pub fn readable(ctx: &Context, color: Color32) -> Color32 {
    match ctx.theme() {
        Theme::Dark => color,
        Theme::Light => {
            let k = 0.55;
            let scale = |c: u8| (f32::from(c) * k) as u8;
            Color32::from_rgb(scale(color.r()), scale(color.g()), scale(color.b()))
        }
    }
}

/// egui's preference for the stored choice.
pub fn preference(choice: ThemeChoice) -> ThemePreference {
    match choice {
        ThemeChoice::System => ThemePreference::System,
        ThemeChoice::Dark => ThemePreference::Dark,
        ThemeChoice::Light => ThemePreference::Light,
    }
}

/// Register both schemes with egui and select `choice`.
pub fn apply_theme(ctx: &Context, choice: ThemeChoice) {
    ctx.set_style_of(
        Theme::Dark,
        style(ctx.style_of(Theme::Dark).as_ref(), &DARK, true),
    );
    ctx.set_style_of(
        Theme::Light,
        style(ctx.style_of(Theme::Light).as_ref(), &LIGHT, false),
    );
    ctx.set_theme(preference(choice));
}

fn style(base: &Style, t: &Tokens, dark: bool) -> Style {
    let mut style = base.clone();

    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(18.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(14.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(11.5, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
    ]
    .into();

    let s = &mut style.spacing;
    s.item_spacing = vec2(8.0, 6.0);
    s.button_padding = vec2(10.0, 4.0);
    s.interact_size = vec2(40.0, 26.0);
    s.menu_margin = Margin::same(6.0);
    s.window_margin = Margin::same(12.0);
    s.indent = 16.0;

    let (shadow_alpha, popup_alpha) = if dark { (110, 88) } else { (40, 32) };
    let v = &mut style.visuals;
    v.dark_mode = dark;
    v.override_text_color = None;
    v.panel_fill = t.surface;
    v.window_fill = t.surface_container;
    v.window_stroke = Stroke::new(1.0_f32, t.outline_variant);
    v.window_rounding = Rounding::same(RADIUS_LG);
    v.menu_rounding = Rounding::same(RADIUS_MD);
    v.window_shadow = Shadow {
        offset: vec2(0.0, 8.0),
        blur: 24.0,
        spread: 0.0,
        color: Color32::from_black_alpha(shadow_alpha),
    };
    v.popup_shadow = Shadow {
        offset: vec2(0.0, 4.0),
        blur: 12.0,
        spread: 0.0,
        color: Color32::from_black_alpha(popup_alpha),
    };
    v.extreme_bg_color = t.field;
    v.faint_bg_color = t.surface_container_low;
    v.code_bg_color = t.surface_container_high;
    v.hyperlink_color = t.primary;
    v.warn_fg_color = t.warning;
    v.error_fg_color = t.error;
    v.selection.bg_fill = t.secondary_container;
    v.selection.stroke = Stroke::new(1.0_f32, t.primary);
    v.collapsing_header_frame = false;
    v.indent_has_left_vline = true;

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = t.surface_container;
    w.noninteractive.weak_bg_fill = t.surface_container;
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, t.outline_variant);
    w.noninteractive.fg_stroke = Stroke::new(1.0_f32, t.on_surface);

    // Rest: tonal fill, hairline outline so inputs and buttons stay defined.
    w.inactive.bg_fill = t.surface_container_high;
    w.inactive.weak_bg_fill = t.surface_container_high;
    w.inactive.bg_stroke = Stroke::new(1.0_f32, t.outline_variant);
    w.inactive.fg_stroke = Stroke::new(1.0_f32, t.on_surface);

    w.hovered.bg_fill = t.surface_container_highest;
    w.hovered.weak_bg_fill = t.surface_container_highest;
    w.hovered.bg_stroke = Stroke::new(1.0_f32, t.outline);
    w.hovered.fg_stroke = Stroke::new(1.5_f32, t.on_surface);

    // egui also uses the active style for keyboard focus, so the primary
    // outline doubles as the focus ring.
    w.active.bg_fill = t.primary;
    w.active.weak_bg_fill = t.pressed;
    w.active.bg_stroke = Stroke::new(1.5_f32, t.primary);
    w.active.fg_stroke = Stroke::new(2.0_f32, t.on_surface);

    w.open.bg_fill = t.surface_container_highest;
    w.open.weak_bg_fill = t.surface_container_highest;
    w.open.bg_stroke = Stroke::new(1.0_f32, t.outline);
    w.open.fg_stroke = Stroke::new(1.0_f32, t.on_surface);

    for visuals in [
        &mut w.noninteractive,
        &mut w.inactive,
        &mut w.hovered,
        &mut w.active,
        &mut w.open,
    ] {
        visuals.rounding = Rounding::same(RADIUS_SM);
        // Widgets keep their size on hover instead of growing by a pixel.
        visuals.expansion = 0.0;
    }

    style
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(c: Color32) -> f64 {
        let channel = |v: u8| {
            let v = f64::from(v) / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
    }

    fn contrast(a: Color32, b: Color32) -> f64 {
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// WCAG AA: 4.5:1 for text roles on every surface they appear on.
    #[test]
    fn text_roles_meet_wcag_aa_on_all_surfaces() {
        for t in [&DARK, &LIGHT] {
            for surface in [
                t.surface,
                t.surface_container_low,
                t.surface_container,
                t.surface_container_high,
                t.field,
            ] {
                for text in [t.on_surface, t.on_surface_variant, t.primary, t.error] {
                    let ratio = contrast(text, surface);
                    assert!(ratio >= 4.5, "{text:?} on {surface:?} is {ratio:.2}:1");
                }
            }
            assert!(contrast(t.on_primary, t.primary) >= 4.5);
            assert!(contrast(t.on_error_container, t.error_container) >= 4.5);
            assert!(contrast(t.on_surface, t.secondary_container) >= 4.5);
            assert!(contrast(t.primary, t.secondary_container) >= 3.0);
        }
    }
}
