//! Cyberpunk theme configuration for the file manager.

use eframe::egui::{self, Color32, Stroke, Visuals};

/// Cyberpunk color palette.
pub mod colors {
    use eframe::egui::Color32;

    /// Electric blue - primary accent color.
    pub const ELECTRIC_BLUE: Color32 = Color32::from_rgb(0, 212, 255);

    /// Cyber green - secondary accent color.
    pub const CYBER_GREEN: Color32 = Color32::from_rgb(0, 255, 136);

    /// Magenta - highlight color.
    pub const MAGENTA: Color32 = Color32::from_rgb(255, 0, 128);

    /// Amber - warning/info color.
    pub const AMBER: Color32 = Color32::from_rgb(255, 165, 0);

    /// Crimson red - error color.
    pub const CRIMSON: Color32 = Color32::from_rgb(255, 7, 58);

    /// Cyan - hyperlink color.
    pub const CYAN: Color32 = Color32::from_rgb(0, 255, 255);

    /// Deep charcoal - panel background.
    pub const DEEP_CHARCOAL: Color32 = Color32::from_rgb(10, 10, 10);

    /// Dark slate blue - window background.
    pub const DARK_SLATE: Color32 = Color32::from_rgb(26, 26, 46);

    /// Midnight purple - faint background.
    pub const MIDNIGHT_PURPLE: Color32 = Color32::from_rgb(22, 33, 62);
}

/// Apply the cyberpunk theme to the egui context.
pub fn setup_cyberpunk_theme(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();

    // Text and backgrounds
    visuals.override_text_color = Some(colors::ELECTRIC_BLUE);
    visuals.panel_fill = colors::DEEP_CHARCOAL;
    visuals.window_fill = colors::DARK_SLATE;
    visuals.faint_bg_color = colors::MIDNIGHT_PURPLE;

    visuals.widgets.noninteractive.bg_fill = colors::DARK_SLATE;
    visuals.widgets.inactive.bg_fill = colors::MIDNIGHT_PURPLE;
    visuals.widgets.hovered.bg_fill = colors::ELECTRIC_BLUE.linear_multiply(0.15);
    visuals.widgets.active.bg_fill = colors::ELECTRIC_BLUE.linear_multiply(0.25);
    visuals.widgets.open.bg_fill = colors::CYBER_GREEN.linear_multiply(0.15);

    visuals.widgets.noninteractive.bg_stroke =
        Stroke::new(1.5_f32, colors::ELECTRIC_BLUE.linear_multiply(0.6));
    visuals.widgets.hovered.bg_stroke = Stroke::new(2.5_f32, colors::ELECTRIC_BLUE);
    visuals.widgets.active.bg_stroke = Stroke::new(2.5_f32, colors::MAGENTA);

    // Selection colors
    visuals.selection.bg_fill = colors::ELECTRIC_BLUE.linear_multiply(0.35);
    visuals.selection.stroke = Stroke::new(1.5_f32, colors::ELECTRIC_BLUE);

    // Other colors
    visuals.hyperlink_color = colors::CYAN;
    visuals.error_fg_color = colors::CRIMSON;
    visuals.warn_fg_color = colors::AMBER;

    ctx.set_visuals(visuals);
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
