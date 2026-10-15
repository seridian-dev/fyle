//! List view component for displaying files.

use crate::core::{format_size, get_file_icon, open_with_recommended};
use crate::theme::{colors, get_entry_color};
use crate::types::{FileEntry, SortBy, SortOrder};
use crate::ui::ViewAction;
use eframe::egui::{self, RichText, Sense, Ui};

/// Render the list view header with sort buttons.
fn render_header(ui: &mut Ui, current_sort: &SortBy, sort_order: &SortOrder) -> Option<SortBy> {
    let mut action = None;

    let sort_indicator = |is_current: bool| -> &str {
        if is_current {
            match sort_order {
                SortOrder::Ascending => " ▲",
                SortOrder::Descending => " ▼",
            }
        } else {
            ""
        }
    };

    // Name column header
    let name_text = format!("Name{}", sort_indicator(*current_sort == SortBy::Name));
    if ui
        .add(egui::Button::new(
            RichText::new(name_text).color(colors::CYBER_GREEN),
        ))
        .clicked()
    {
        action = Some(SortBy::Name);
    }

    // Size column header
    let size_text = format!("Size{}", sort_indicator(*current_sort == SortBy::Size));
    if ui
        .add(egui::Button::new(
            RichText::new(size_text).color(colors::ELECTRIC_BLUE),
        ))
        .clicked()
    {
        action = Some(SortBy::Size);
    }

    // Type column header
    let type_text = format!("Type{}", sort_indicator(*current_sort == SortBy::Type));
    if ui
        .add(egui::Button::new(
            RichText::new(type_text).color(colors::MAGENTA),
        ))
        .clicked()
    {
        action = Some(SortBy::Type);
    }

    // Modified column header (now sortable)
    let modified_text = format!(
        "Modified{}",
        sort_indicator(*current_sort == SortBy::DateModified)
    );
    if ui
        .add(egui::Button::new(
            RichText::new(modified_text).color(colors::AMBER),
        ))
        .clicked()
    {
        action = Some(SortBy::DateModified);
    }

    ui.end_row();

    action
}

/// Render a single file entry row.
fn render_entry_row(ui: &mut Ui, entry: &FileEntry, selected: bool) -> Option<ViewAction> {
    let mut action = None;
    let name_color = get_entry_color(entry.is_dir);

    // Checkbox for selection
    let mut is_selected = selected;
    if ui.checkbox(&mut is_selected, "").changed() {
        action = Some(ViewAction::ToggleSelection(entry.path.clone()));
    }

    // File icon
    ui.label(get_file_icon(entry));

    // File name with click handling
    let name_response = ui
        .add(egui::Label::new(RichText::new(&entry.name).color(name_color)).sense(Sense::click()));

    // Double-click to navigate or open
    if name_response.double_clicked() {
        if entry.is_dir {
            action = Some(ViewAction::Navigate(entry.path.clone()));
        } else {
            if let Err(e) = open_with_recommended(&entry.path) {
                action = Some(ViewAction::OpenError(format!("Failed to open file: {}", e)));
            }
        }
    }

    // Right-click for context menu
    if name_response.secondary_clicked() {
        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
            action = Some(ViewAction::ShowContextMenu(pos, entry.clone()));
        }
    }

    // Single click to select (only if no other action was taken)
    if action.is_none() && name_response.clicked() {
        action = Some(ViewAction::SelectSingle(entry.path.clone()));
    }

    // Size column
    ui.label(format_size(entry.size));

    // Type column
    ui.label(&entry.file_type);

    // Modified column
    ui.label(entry.modified.format("%Y-%m-%d %H:%M").to_string());

    ui.end_row();

    action
}

/// Render the complete list view.
/// Returns a list of actions triggered during rendering.
pub fn render_list_view(
    ui: &mut Ui,
    entries: &[FileEntry],
    current_sort: &SortBy,
    sort_order: &SortOrder,
) -> Vec<ViewAction> {
    let mut actions = Vec::new();
    let available_height = ui.available_height();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(available_height)
        .show(ui, |ui| {
            egui::Grid::new("file_list")
                .striped(true)
                .min_col_width(100.0)
                .show(ui, |ui| {
                    // Render header
                    if let Some(sort_by) = render_header(ui, current_sort, sort_order) {
                        actions.push(ViewAction::ChangeSort(sort_by));
                    }

                    // Render entries
                    for entry in entries {
                        if let Some(action) = render_entry_row(ui, entry, entry.selected) {
                            actions.push(action);
                        }
                    }
                });
        });

    actions
}
