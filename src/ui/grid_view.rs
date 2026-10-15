//! Grid view component for displaying files.

use crate::core::{format_size, get_file_icon, open_with_recommended};
use crate::theme::{colors, get_entry_color, get_selection_bg_color, get_selection_stroke_color};
use crate::types::FileEntry;
use crate::ui::constants::*;
use crate::ui::{truncate_name, ViewAction};
use eframe::egui::{self, RichText, Sense, Stroke, Ui};

/// Render a single grid item.
fn render_grid_item(ui: &mut Ui, entry: &FileEntry) -> Option<ViewAction> {
    #[cfg(test)]
    crate::ui::RENDERED_ENTRIES.with(|count| count.set(count.get() + 1));
    let mut action = None;
    let name_color = get_entry_color(entry.is_dir);
    let bg_color = get_selection_bg_color(entry.selected);
    let stroke_color = get_selection_stroke_color(entry.selected);

    ui.vertical(|ui| {
        ui.set_width(GRID_ITEM_WIDTH - 10.0);
        ui.set_height(GRID_ITEM_HEIGHT);

        egui::Frame::none()
            .fill(bg_color)
            .stroke(Stroke::new(1.0_f32, stroke_color))
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.set_width(GRID_ITEM_INNER_WIDTH);
                    ui.set_height(GRID_ITEM_INNER_HEIGHT);

                    // Selection checkbox
                    let mut selected = entry.selected;
                    if ui.checkbox(&mut selected, "").changed() {
                        action = Some(ViewAction::ToggleSelection(entry.path.clone()));
                    }

                    // File icon
                    ui.label(RichText::new(get_file_icon(entry)).size(GRID_ICON_SIZE));

                    // File name with truncation
                    let display_name = truncate_name(&entry.name, MAX_NAME_LENGTH);

                    let name_response = ui
                        .add(
                            egui::Label::new(
                                RichText::new(&display_name)
                                    .color(name_color)
                                    .size(GRID_NAME_SIZE),
                            )
                            .sense(Sense::click()),
                        )
                        .on_hover_text(&entry.name);

                    crate::ui::expose_control(
                        ui,
                        &format!("entry:{}", entry.name),
                        &entry.name,
                        &name_response,
                    );

                    // Double-click to navigate or open
                    if name_response.double_clicked() {
                        if entry.is_dir {
                            action = Some(ViewAction::Navigate(entry.path.clone()));
                        } else {
                            if let Err(e) = open_with_recommended(&entry.path) {
                                action = Some(ViewAction::OpenError(format!(
                                    "Failed to open file: {}",
                                    e
                                )));
                            }
                        }
                    }

                    // Right-click for context menu
                    if name_response.secondary_clicked() {
                        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                            action = Some(ViewAction::ShowContextMenu(pos, entry.clone()));
                        }
                    }

                    // Single click to select
                    if action.is_none() && name_response.clicked() {
                        action = Some(ViewAction::SelectSingle(entry.path.clone()));
                    }

                    // Size and type labels
                    ui.label(
                        RichText::new(format_size(entry.size))
                            .color(colors::AMBER)
                            .size(GRID_DETAIL_SIZE),
                    );
                    ui.label(
                        RichText::new(&entry.file_type)
                            .color(colors::MAGENTA)
                            .size(GRID_DETAIL_SIZE),
                    );
                });
            });
    });

    action
}

/// Render the complete grid view.
/// Returns a list of actions triggered during rendering.
pub fn render_grid_view(ui: &mut Ui, entries: &[FileEntry]) -> Vec<ViewAction> {
    let mut actions = Vec::new();

    let items_per_row = (ui.available_width() / GRID_ITEM_WIDTH).floor().max(1.0) as usize;
    let total_rows = entries.len().div_ceil(items_per_row);
    egui::ScrollArea::vertical()
        .id_source("file_grid_scroll")
        .auto_shrink([false, false])
        .show_rows(ui, GRID_ITEM_HEIGHT, total_rows, |ui, rows| {
            for row in rows {
                let start = row * items_per_row;
                let end = (start + items_per_row).min(entries.len());
                ui.horizontal(|ui| {
                    for entry in &entries[start..end] {
                        if let Some(action) = render_grid_item(ui, entry) {
                            actions.push(action);
                        }
                    }
                });
            }
        });
    actions
}
