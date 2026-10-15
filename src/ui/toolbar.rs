//! Toolbar components for file manager.

use crate::theme::colors;
use crate::types::ViewMode;
use eframe::egui::{self, Color32, RichText, Rounding, Stroke, Ui, Vec2};
use std::path::Path;

/// Actions from the toolbar.
#[derive(Debug, Clone)]
pub enum ToolbarAction {
    /// Navigate back.
    GoBack,
    /// Navigate forward.
    GoForward,
    /// Navigate to parent directory.
    GoUp,
    /// Create new folder.
    NewFolder,
    /// Create new file.
    NewFile,
    /// Toggle hidden files.
    ToggleHidden,
    /// Change view mode.
    SetViewMode(ViewMode),
    /// Search query changed.
    Search(String),
    /// Open terminal here.
    OpenTerminal,
}

fn render_toolbar_button(ui: &mut Ui, icon: &str, label: &str, color: Color32) -> bool {
    let button = egui::Button::new(
        RichText::new(format!("{} {}", icon, label))
            .color(color)
            .size(13.0),
    )
    .fill(color.linear_multiply(0.05))
    .stroke(Stroke::new(1.0_f32, color.linear_multiply(0.2)))
    .rounding(Rounding::same(4.0));

    let response = ui.add(button);
    crate::ui::expose_control(ui, &label.to_ascii_lowercase(), label, &response);

    if response.hovered() {
        ui.painter().rect_stroke(
            response.rect,
            Rounding::same(4.0),
            Stroke::new(1.0_f32, color.linear_multiply(0.8)),
        );
    }

    response.clicked()
}

/// Render the main toolbar.
#[allow(clippy::too_many_arguments)] // Explicit view inputs; no global app dependency.
pub fn render_toolbar(
    ui: &mut Ui,
    current_path: &Path,
    file_count: usize,
    show_hidden: bool,
    view_mode: &ViewMode,
    search_query: &mut String,
    can_go_back: bool,
    can_go_forward: bool,
) -> Option<ToolbarAction> {
    let mut action = None;

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        // Left side - app name with glow effect
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.add_space(8.0);
            ui.label(
                RichText::new("FYLE")
                    .font(egui::FontId::proportional(24.0))
                    .color(colors::ELECTRIC_BLUE)
                    .strong(),
            );

            // Subtitle
            ui.add_space(4.0);
            ui.label(
                RichText::new(concat!("v", env!("CARGO_PKG_VERSION")))
                    .color(colors::ELECTRIC_BLUE.linear_multiply(0.4))
                    .size(10.0),
            );
        });

        // Right side - path breadcrumbs or info
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!("{} items", file_count))
                    .color(colors::ELECTRIC_BLUE.linear_multiply(0.6))
                    .size(12.0),
            );
            ui.separator();

            let path_display = current_path.display().to_string();
            let truncated_path = if path_display.len() > 50 {
                format!("...{}", &path_display[path_display.len() - 47..])
            } else {
                path_display
            };

            ui.label(
                RichText::new(truncated_path)
                    .color(colors::CYBER_GREEN)
                    .size(12.0)
                    .monospace(),
            );
            ui.label(
                RichText::new("PATH:")
                    .color(colors::CYBER_GREEN.linear_multiply(0.4))
                    .size(10.0)
                    .strong(),
            );
        });
    });

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.add_space(4.0);

        // Navigation history
        if ui
            .add_enabled(
                can_go_back,
                egui::Button::new(RichText::new("◀").color(colors::ELECTRIC_BLUE)),
            )
            .clicked()
        {
            action = Some(ToolbarAction::GoBack);
        }

        if ui
            .add_enabled(
                can_go_forward,
                egui::Button::new(RichText::new("▶").color(colors::ELECTRIC_BLUE)),
            )
            .clicked()
        {
            action = Some(ToolbarAction::GoForward);
        }

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        // Navigation button
        if render_toolbar_button(ui, "⬆", "UP", colors::CYBER_GREEN) {
            action = Some(ToolbarAction::GoUp);
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        // Create buttons
        if render_toolbar_button(ui, "📁", "FOLDER", colors::CYBER_GREEN) {
            action = Some(ToolbarAction::NewFolder);
        }

        if render_toolbar_button(ui, "📄", "FILE", colors::ELECTRIC_BLUE) {
            action = Some(ToolbarAction::NewFile);
        }

        if render_toolbar_button(ui, "💻", "TERM", colors::MAGENTA) {
            action = Some(ToolbarAction::OpenTerminal);
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        // Search with styled frame
        ui.label(
            RichText::new("SEARCH")
                .color(colors::ELECTRIC_BLUE.linear_multiply(0.5))
                .size(10.0)
                .strong(),
        );

        let search_width = 200.0;

        let search_response = ui.add(
            egui::TextEdit::singleline(search_query)
                .hint_text("Filter files...")
                .desired_width(search_width)
                .margin(Vec2::new(8.0, 4.0)),
        );

        crate::ui::expose_control(ui, "search", "Filter files", &search_response);
        if search_response.changed() {
            action = Some(ToolbarAction::Search(search_query.clone()));
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        // Hidden files toggle
        let hidden_text = if show_hidden {
            "HIDDEN: ON"
        } else {
            "HIDDEN: OFF"
        };

        if ui
            .add(egui::SelectableLabel::new(
                show_hidden,
                RichText::new(hidden_text).size(11.0),
            ))
            .clicked()
        {
            action = Some(ToolbarAction::ToggleHidden);
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(4.0);

            // View mode toggles
            let grid_active = *view_mode == ViewMode::Grid;
            let grid_response = ui.add(egui::SelectableLabel::new(
                grid_active,
                RichText::new("GRID").size(11.0),
            ));
            crate::ui::expose_control(ui, "grid", "Grid view", &grid_response);
            if grid_response.clicked() && !grid_active {
                action = Some(ToolbarAction::SetViewMode(ViewMode::Grid));
            }

            let list_active = *view_mode == ViewMode::List;
            let list_response = ui.add(egui::SelectableLabel::new(
                list_active,
                RichText::new("LIST").size(11.0),
            ));
            crate::ui::expose_control(ui, "list", "List view", &list_response);
            if list_response.clicked() && !list_active {
                action = Some(ToolbarAction::SetViewMode(ViewMode::List));
            }

            ui.label(
                RichText::new("VIEW:")
                    .color(colors::MAGENTA.linear_multiply(0.5))
                    .size(10.0)
                    .strong(),
            );
        });
    });

    ui.add_space(4.0);

    action
}

/// Render the bulk operations toolbar (shown when items are selected).
pub fn render_bulk_operations(ui: &mut Ui, selected_count: usize) -> Option<BulkAction> {
    if selected_count == 0 {
        return None;
    }

    let mut action = None;

    ui.add_space(2.0);
    egui::Frame::none()
        .fill(colors::ELECTRIC_BLUE.linear_multiply(0.05))
        .stroke(Stroke::new(
            1.0_f32,
            colors::ELECTRIC_BLUE.linear_multiply(0.2),
        ))
        .inner_margin(4.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("{} SELECTED", selected_count))
                        .color(colors::ELECTRIC_BLUE)
                        .strong()
                        .size(11.0),
                );

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(12.0);

                if render_toolbar_button(ui, "✂", "CUT", colors::AMBER) {
                    action = Some(BulkAction::Cut);
                }

                if render_toolbar_button(ui, "📋", "COPY", colors::CYBER_GREEN) {
                    action = Some(BulkAction::Copy);
                }

                if render_toolbar_button(ui, "🗑", "DELETE", colors::CRIMSON) {
                    action = Some(BulkAction::Delete);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(4.0);
                    if ui.small_button("NONE").clicked() {
                        action = Some(BulkAction::SelectNone);
                    }
                    if ui.small_button("ALL").clicked() {
                        action = Some(BulkAction::SelectAll);
                    }
                });
            });
        });

    ui.add_space(4.0);

    action
}

/// Bulk operation actions.
#[derive(Debug, Clone)]
pub enum BulkAction {
    Copy,
    Cut,
    Delete,
    SelectAll,
    SelectNone,
}
