//! Cyberpunk theme configuration for the file manager.

use eframe::egui::{self, Color32, Stroke, Visuals};

/// Cyberpunk color palette.
#[path = "palette.rs"]
pub mod colors;
pub mod shader;

/// Apply the cyberpunk theme to the egui context.
pub fn setup_cyberpunk_theme(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();

    // Text and backgrounds
    visuals.override_text_color = Some(colors::TEXT);
    visuals.panel_fill = colors::DEEP_CHARCOAL;
    visuals.window_fill = colors::DARK_SLATE;
    visuals.faint_bg_color = colors::MIDNIGHT_PURPLE;

    visuals.widgets.noninteractive.bg_fill = colors::DARK_SLATE;
    visuals.widgets.inactive.bg_fill = colors::MIDNIGHT_PURPLE;
    visuals.widgets.hovered.bg_fill = colors::ELECTRIC_BLUE.linear_multiply(0.15);
    visuals.widgets.active.bg_fill = colors::ELECTRIC_BLUE.linear_multiply(0.25);
    visuals.widgets.open.bg_fill = colors::CYBER_GREEN.linear_multiply(0.15);

    // egui uses weak_bg_fill for window title bars. Give dialogs a tinted glass
    // header and let the Fyle surface show through their dark, translucent body.
    visuals.widgets.open.weak_bg_fill = Color32::from_rgba_unmultiplied(0, 91, 112, 220);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgba_unmultiplied(0, 91, 112, 185);
    visuals.widgets.active.weak_bg_fill = Color32::from_rgba_unmultiplied(0, 117, 136, 220);
    visuals.widgets.inactive.weak_bg_fill = Color32::from_rgba_unmultiplied(17, 33, 48, 200);
    visuals.window_fill = Color32::from_rgba_unmultiplied(18, 23, 43, 236);
    visuals.window_stroke = Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(0, 212, 255, 120));
    visuals.window_shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 10.0),
        blur: 26.0,
        spread: 0.0,
        color: Color32::from_rgba_unmultiplied(0, 0, 0, 140),
    };

    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, colors::BORDER);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, colors::ELECTRIC_BLUE);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, colors::ELECTRIC_BLUE);

    // Selection colors
    visuals.selection.bg_fill = colors::ELECTRIC_BLUE.linear_multiply(0.35);
    visuals.selection.stroke = Stroke::new(1.5_f32, colors::ELECTRIC_BLUE);

    // Other colors
    visuals.hyperlink_color = colors::CYAN;
    visuals.error_fg_color = colors::CRIMSON;
    visuals.warn_fg_color = colors::AMBER;

    visuals.window_rounding = egui::Rounding::same(8.0);
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.rounding = egui::Rounding::same(2.0);
        widget.fg_stroke.color = colors::TEXT;
    }
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, colors::BORDER);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, colors::BORDER);
    ctx.set_visuals(visuals);
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    style.spacing.interact_size.y = 30.0;
    style.spacing.window_margin = egui::Margin::same(16.0);
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::monospace(12.0));
    style
        .text_styles
        .insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
    style
        .text_styles
        .insert(egui::TextStyle::Heading, egui::FontId::monospace(21.0));
    ctx.set_style(style);
}

/// Get the appropriate color for a file entry name.
#[must_use]
pub fn get_entry_color(is_dir: bool) -> Color32 {
    if is_dir {
        colors::CYBER_GREEN
    } else {
        colors::ELECTRIC_BLUE
    }
}

/// Get the selection background color.
#[must_use]
pub fn get_selection_bg_color(selected: bool) -> Color32 {
    if selected {
        colors::ELECTRIC_BLUE.linear_multiply(0.2)
    } else {
        Color32::TRANSPARENT
    }
}

/// Get the selection stroke color.
#[must_use]
pub fn get_selection_stroke_color(selected: bool) -> Color32 {
    if selected {
        colors::ELECTRIC_BLUE
    } else {
        Color32::TRANSPARENT
    }
}

/// Apply preferences without changing the original semantic file colors.
pub fn apply_appearance(ctx: &egui::Context, settings: &fyle::appearance::Appearance) {
    setup_cyberpunk_theme(ctx);
    ctx.set_zoom_factor(settings.scale);
    let mut style = (*ctx.style()).clone();
    let accent = Color32::from_rgb(
        settings.highlight[0],
        settings.highlight[1],
        settings.highlight[2],
    );
    style.visuals.selection.bg_fill = accent.linear_multiply(0.28);
    style.visuals.selection.stroke = Stroke::new(1.0_f32, accent);
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, accent);
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, accent);
    ctx.set_style(style);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_entry_color_directory() {
        assert_eq!(get_entry_color(true), colors::CYBER_GREEN);
    }

    #[test]
    fn test_get_entry_color_file() {
        assert_eq!(get_entry_color(false), colors::ELECTRIC_BLUE);
    }

    #[test]
    fn test_get_selection_bg_color_selected() {
        let color = get_selection_bg_color(true);
        assert_ne!(color, Color32::TRANSPARENT);
    }

    #[test]
    fn test_get_selection_bg_color_not_selected() {
        assert_eq!(get_selection_bg_color(false), Color32::TRANSPARENT);
    }

    #[test]
    fn test_get_selection_stroke_color() {
        assert_eq!(get_selection_stroke_color(true), colors::ELECTRIC_BLUE);
        assert_eq!(get_selection_stroke_color(false), Color32::TRANSPARENT);
    }
}
