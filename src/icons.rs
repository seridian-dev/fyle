//! Embedded icon font and icon constants for the file manager.
//!
//! This module uses the Feather Icons font, which provides clean, minimal icons
//! perfect for a cyberpunk-themed file manager.

use eframe::egui;

/// Feather Icons font data (embedded at compile time).
const FONT_DATA: &[u8] = include_bytes!("../assets/fonts/feather.ttf");

pub const ICON_FONT_FAMILY: &str = "feather_icons";

/// Icon codes for Feather Icons.
///
/// Feather Icons uses Unicode codepoints for each icon. These are the
/// standard icon codes from the Feather Icons font.
pub mod icons {
    pub const ARROW_LEFT: char = '\u{e905}';
    pub const ARROW_RIGHT: char = '\u{e906}';
    pub const ARROW_UP: char = '\u{e907}';
    pub const ARROW_UP_LEFT: char = '\u{e908}';

    pub const FILE: char = '\u{e9b7}';
    pub const FOLDER: char = '\u{e9a7}';
    pub const FOLDER_PLUS: char = '\u{e9a9}';
    pub const FILE_TEXT: char = '\u{e9b8}';

    pub const COPY: char = '\u{e90c}';
    pub const CUT: char = '\u{e92b}';
    pub const TRASH: char = '\u{e997}';
    pub const EDIT: char = '\u{e94e}';
    pub const SAVE: char = '\u{e97b}';
    pub const SEARCH: char = '\u{e986}';

    pub const GRID: char = '\u{e969}';
    pub const LIST: char = '\u{e957}';
    pub const EYE: char = '\u{e938}';
    pub const EYE_OFF: char = '\u{e939}';

    pub const TERMINAL: char = '\u{e992}';
    pub const COMMAND: char = '\u{e8a5}';
    pub const DOWNLOAD: char = '\u{e9c0}';

    pub const MORE_VERTICAL: char = '\u{e960}';
    pub const MORE_HORIZONTAL: char = '\u{e95f}';
    pub const STAR: char = '\u{e928}';
    pub const HARD_DRIVE: char = '\u{e8ad}';
    pub const REFRESH: char = '\u{e972}';
    pub const CHECK: char = '\u{e89c}';
    pub const X: char = '\u{e8a5}';
    pub const INFO: char = '\u{e93a}';
    pub const WARNING: char = '\u{e9ad}';
    pub const HOME: char = '\u{e944}';
}

/// Register the icon font with the egui context.
///
/// This should be called during app initialization to make the icon font
/// available throughout the application.
pub fn register_icon_font(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        ICON_FONT_FAMILY.to_owned(),
        egui::FontData::from_static(FONT_DATA),
    );

    let family_name: egui::FontFamily = egui::FontFamily::Name(ICON_FONT_FAMILY.into());
    fonts
        .families
        .entry(family_name)
        .or_insert_with(Vec::new)
        .push(ICON_FONT_FAMILY.to_owned());

    ctx.set_fonts(fonts);
}

/// Get the FontId for icon text.
///
/// Returns a FontId configured for the icon font at the specified size.
pub fn icon_font_id(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name(ICON_FONT_FAMILY.into()))
}

/// Create RichText with an icon.
///
/// Helper function to create formatted text containing an icon.
pub fn icon_text(icon: char, size: f32) -> egui::RichText {
    egui::RichText::new(icon.to_string())
        .font(icon_font_id(size))
        .size(size)
}

/// Create a styled icon button with label.
///
/// Creates a button containing an icon and optional label text.
pub fn icon_button(
    ui: &mut egui::Ui,
    icon: char,
    label: Option<&str>,
    size: f32,
    color: egui::Color32,
) -> egui::Response {
    let text = if let Some(label_text) = label {
        format!("{} {}", icon, label_text)
    } else {
        icon.to_string()
    };

    let button = egui::Button::new(
        egui::RichText::new(text)
            .font(icon_font_id(size))
            .size(size)
            .color(color),
    );

    ui.add(button)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icon_constants_are_valid() {
        assert!(icons::ARROW_LEFT.is_valid());
        assert!(icons::FOLDER.is_valid());
        assert!(icons::FILE.is_valid());
    }

    #[test]
    fn test_font_data_is_not_empty() {
        assert!(!FONT_DATA.is_empty(), "Font data should not be empty");
    }
}
