//! Material-style building blocks on top of egui.

use super::theme::{layer, tokens, HOVER_LAYER, PRESSED_LAYER, RADIUS_MD, RADIUS_SM};
use egui::{
    vec2, Align2, Color32, FontId, Response, RichText, Sense, Stroke, TextStyle, TextWrapMode, Ui,
    WidgetInfo, WidgetText, WidgetType,
};

const LIST_ITEM_HEIGHT: f32 = 28.0;
const BADGE_WIDTH: f32 = 46.0;

/// Background for a hovered/pressed interactive surface.
fn state_layer(response: &Response) -> Color32 {
    let on_surface = tokens(&response.ctx).on_surface;
    if response.is_pointer_button_down_on() {
        layer(on_surface, PRESSED_LAYER)
    } else if response.hovered() || response.has_focus() {
        layer(on_surface, HOVER_LAYER)
    } else {
        Color32::TRANSPARENT
    }
}

/// A full-width list row: the whole row is the hit target and shows the
/// hover/pressed state layer; the selected row gets a tinted container and a
/// leading indicator. `badge` is a short coloured tag such as an HTTP method.
/// `accessible` is the label assistive tech (and the UI tests) see.
pub fn list_item(
    ui: &mut Ui,
    selected: bool,
    badge: Option<(&str, Color32)>,
    label: impl Into<WidgetText>,
    accessible: &str,
) -> Response {
    let t = tokens(ui.ctx());
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), LIST_ITEM_HEIGHT), Sense::click());
    response.widget_info(|| {
        WidgetInfo::selected(
            WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            accessible,
        )
    });
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let painter = ui.painter();
    let fill = if selected {
        t.secondary_container
    } else {
        state_layer(&response)
    };
    painter.rect_filled(rect, RADIUS_SM, fill);
    if selected {
        let bar = egui::Rect::from_min_size(
            rect.left_top() + vec2(0.0, 6.0),
            vec2(3.0, rect.height() - 12.0),
        );
        painter.rect_filled(bar, 1.5, t.primary);
    }
    let mut x = rect.left() + 10.0;
    if let Some((text, color)) = badge {
        painter.text(
            egui::pos2(x, rect.center().y),
            Align2::LEFT_CENTER,
            text,
            FontId::monospace(11.0),
            color,
        );
        x += BADGE_WIDTH;
    }
    let galley = label.into().into_galley(
        ui,
        Some(TextWrapMode::Truncate),
        (rect.right() - x - 8.0).max(0.0),
        TextStyle::Body,
    );
    let text_color = if selected {
        t.on_surface
    } else {
        ui.visuals().text_color()
    };
    painter.galley(
        egui::pos2(x, rect.center().y - galley.size().y / 2.0),
        galley,
        text_color,
    );
    response
}

/// A text tab with an underline indicator (Material primary tabs).
/// `badge` is an optional count painted after the label; the accessible
/// label stays `label` so it is stable for assistive tech and tests.
pub fn tab(ui: &mut Ui, selected: bool, label: &str, badge: Option<usize>) -> Response {
    let t = tokens(ui.ctx());
    let font = TextStyle::Button.resolve(ui.style());
    let text_color = if selected {
        t.primary
    } else {
        t.on_surface_variant
    };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font.clone(), text_color);
    let badge_galley = badge.filter(|n| *n > 0).map(|n| {
        ui.painter()
            .layout_no_wrap(n.to_string(), FontId::proportional(11.0), t.on_surface)
    });
    let badge_width = badge_galley.as_ref().map_or(0.0, |g| g.size().x + 14.0);
    let size = vec2(galley.size().x + badge_width + 20.0, 30.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| {
        WidgetInfo::selected(
            WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            label,
        )
    });
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, RADIUS_SM, state_layer(&response));
        let text_pos = egui::pos2(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0);
        let text_right = text_pos.x + galley.size().x;
        painter.galley(text_pos, galley, text_color);
        if let Some(badge) = badge_galley {
            let pill = egui::Rect::from_min_size(
                egui::pos2(text_right + 6.0, rect.center().y - 8.0),
                vec2(badge.size().x + 10.0, 16.0),
            );
            painter.rect_filled(pill, 8.0, t.surface_container_highest);
            painter.galley(pill.center() - badge.size() / 2.0, badge, t.on_surface);
        }
        if selected {
            let line = egui::Rect::from_min_max(
                egui::pos2(rect.left() + 6.0, rect.bottom() - 3.0),
                egui::pos2(rect.right() - 6.0, rect.bottom()),
            );
            painter.rect_filled(line, 1.5, t.primary);
        }
    }
    response
}

/// Primary call-to-action button.
pub fn filled_button(ctx: &egui::Context, text: &str) -> egui::Button<'static> {
    let t = tokens(ctx);
    egui::Button::new(RichText::new(text.to_owned()).color(t.on_primary).strong())
        .fill(t.primary)
        .stroke(Stroke::NONE)
        .rounding(RADIUS_MD)
}

/// A small read-only status chip, tinted by `color`.
pub fn chip(ui: &mut Ui, text: impl Into<String>, color: Color32) -> Response {
    egui::Frame::none()
        .fill(layer(color, 0.16))
        .rounding(RADIUS_MD)
        .inner_margin(egui::Margin::symmetric(8.0, 2.0))
        .show(ui, |ui| {
            ui.label(RichText::new(text.into()).color(color).strong())
        })
        .response
}

pub enum BannerKind {
    Error,
    Info,
}

/// A full-width message bar (Material banner). `actions` adds its buttons
/// on the right.
pub fn banner(ui: &mut Ui, kind: BannerKind, message: &str, actions: impl FnOnce(&mut Ui)) {
    let t = tokens(ui.ctx());
    let (fill, fg, icon) = match kind {
        BannerKind::Error => (t.error_container, t.on_error_container, "⚠"),
        BannerKind::Info => (t.surface_container_high, t.on_surface, "ℹ"),
    };
    egui::Frame::none()
        .fill(fill)
        .rounding(RADIUS_MD)
        .inner_margin(egui::Margin::symmetric(12.0, 8.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon).color(fg).size(16.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    actions(ui);
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.add(egui::Label::new(RichText::new(message).color(fg)).wrap());
                    });
                });
            });
        });
}

/// Centred placeholder for an empty area.
pub fn empty_state(ui: &mut Ui, title: &str, hint: &str) {
    let t = tokens(ui.ctx());
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.3);
        ui.label(RichText::new(title).color(t.on_surface_variant).size(15.0));
        ui.add_space(4.0);
        ui.label(RichText::new(hint).color(t.on_surface_variant).small());
    });
}

/// Bottom action row for a dialog. Show it with `show_inside` before the
/// dialog body (wrapped in a `ScrollArea`), so Save/Close stay visible
/// however long the body gets.
pub fn dialog_footer(id: &str) -> egui::TopBottomPanel {
    egui::TopBottomPanel::bottom(egui::Id::new(id).with("dialog_footer"))
        .frame(egui::Frame::none().inner_margin(egui::Margin {
            left: 0.0,
            right: 0.0,
            top: 8.0,
            bottom: 0.0,
        }))
        .show_separator_line(true)
}
