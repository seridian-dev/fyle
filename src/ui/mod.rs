//! UI components for the file manager.

pub mod context_menu;
pub mod dialogs;
pub mod grid_view;
pub mod list_view;
pub mod sidebar;
pub mod toolbar;

use crate::types::{FileEntry, SortBy};
use eframe::egui::Pos2;
use std::path::PathBuf;

/// Truncate a name for display, adding ellipsis if needed.
#[must_use]
pub fn truncate_name(name: &str, max_len: usize) -> String {
    if name.chars().count() > max_len {
        format!(
            "{}...",
            name.chars()
                .take(max_len.saturating_sub(3))
                .collect::<String>()
        )
    } else {
        name.to_string()
    }
}

/// UI constants for consistent sizing.
pub mod constants {
    /// Width of grid items.
    pub const GRID_ITEM_WIDTH: f32 = 130.0;

    /// Height of grid items.
    pub const GRID_ITEM_HEIGHT: f32 = 120.0;

    /// Inner width of grid item content.
    pub const GRID_ITEM_INNER_WIDTH: f32 = 100.0;

    /// Inner height of grid item content.
    pub const GRID_ITEM_INNER_HEIGHT: f32 = 100.0;

    /// Context menu width.
    pub const CONTEXT_MENU_WIDTH: f32 = 200.0;

    /// Maximum characters for truncated names.
    pub const MAX_NAME_LENGTH: usize = 20;

    /// Icon size for grid view.
    pub const GRID_ICON_SIZE: f32 = 32.0;

    /// Font size for grid item names.
    pub const GRID_NAME_SIZE: f32 = 12.0;

    /// Font size for grid item details.
    pub const GRID_DETAIL_SIZE: f32 = 10.0;
}

/// Actions that can be triggered from file views.
#[derive(Debug, Clone)]
pub enum ViewAction {
    /// Navigate to a directory.
    Navigate(PathBuf),
    /// Toggle selection of a file.
    ToggleSelection(PathBuf),
    /// Select only this file.
    SelectSingle(PathBuf),
    /// Show context menu.
    ShowContextMenu(Pos2, FileEntry),
    /// Change sort order.
    ChangeSort(SortBy),
    /// Error occurred while opening file.
    OpenError(String),
}

#[cfg(test)]
mod tests {
    use super::truncate_name;

    #[test]
    fn truncation_accepts_multibyte_names() {
        assert_eq!(truncate_name("日本語の長い名前.txt", 6), "日本語...");
        assert_eq!(truncate_name("📁abc", 4), "📁abc");
    }
}
