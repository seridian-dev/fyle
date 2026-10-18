//! Toolbar components for file manager.

use crate::theme::colors;
use crate::types::ViewMode;
use eframe::egui::{self, Color32, RichText, Ui, Vec2};
use std::path::Path;

/// Actions from the toolbar.
#[derive(Debug, Clone)]
pub enum ToolbarAction {
    /// Navigate back.
    GoBack,
    Navigate(std::path::PathBuf),
    Location,
    Appearance,
    Refresh,
    Jump,
    Review,
    TogglePreview,
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

use crate::ui::icons::{self, Icon};
fn button(
    ui: &mut Ui,
    id: &str,
    label: &str,
    icon: Option<Icon>,
    selected: bool,
) -> egui::Response {
    let width = if label.is_empty() {
        32.0
    } else {
        ui.fonts(|fonts| {
            fonts
                .layout_no_wrap(label.into(), egui::FontId::monospace(12.0), colors::TEXT)
                .size()
                .x
        }) + if icon.is_some() { 42.0 } else { 22.0 }
    };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 32.0), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Button,
            selected,
            if label.is_empty() { id } else { label },
        )
    });
    let fill = if selected {
        Color32::from_rgb(10, 51, 66)
    } else if response.hovered() {
        colors::MIDNIGHT_PURPLE
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 2.0, fill);
    if response.hovered() || selected {
        ui.painter()
            .rect_stroke(rect, 2.0, egui::Stroke::new(1.0_f32, colors::ELECTRIC_BLUE));
    }
    let color = if response.enabled() {
        if selected {
            colors::ELECTRIC_BLUE
        } else {
            colors::TEXT
        }
    } else {
        colors::MUTED.linear_multiply(0.4)
    };
    if let Some(icon) = icon {
        icons::paint(
            ui.painter(),
            egui::Rect::from_center_size(
                egui::pos2(rect.left() + 16.0, rect.center().y),
                egui::vec2(18.0, 18.0),
            ),
            icon,
            color,
        );
    }
    if !label.is_empty() {
        ui.painter().text(
            egui::pos2(
                rect.left() + if icon.is_some() { 32.0 } else { 11.0 },
                rect.center().y,
            ),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::monospace(12.0),
            color,
        );
    }
    crate::ui::expose_control(ui, id, label, &response);
    response
}
#[allow(clippy::too_many_arguments)]
pub fn render_toolbar(
    ui: &mut Ui,
    current_path: &Path,
    _file_count: usize,
    show_hidden: bool,
    view_mode: &ViewMode,
    search_query: &mut String,
    can_go_back: bool,
    can_go_forward: bool,
    more_open: &mut bool,
) -> Option<ToolbarAction> {
    let mut action = None;
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("F Y L E")
                .size(24.0)
                .monospace()
                .strong()
                .color(colors::TEXT),
        );
        ui.label(
            RichText::new("// FIELD SYSTEM")
                .size(12.0)
                .color(colors::MUTED),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if button(ui, "review", "REVIEW", Some(Icon::Code), false)
                .on_hover_text("Read-only staged and working tree diffs")
                .clicked()
            {
                action = Some(ToolbarAction::Review);
            }
            if button(ui, "jump", "JUMP", Some(Icon::Code), false)
                .on_hover_text("Ctrl+P · find files and commands")
                .clicked()
            {
                action = Some(ToolbarAction::Jump);
            }
            if button(ui, "peek", "PEEK", Some(Icon::Peek), false)
                .on_hover_text("Space · local text preview")
                .clicked()
            {
                action = Some(ToolbarAction::TogglePreview);
            }
        });
    });
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        ui.add_enabled_ui(can_go_back, |ui| {
            if button(ui, "back", "", Some(Icon::Back), false)
                .on_hover_text("Back · Alt+Left")
                .clicked()
            {
                action = Some(ToolbarAction::GoBack);
            }
        });
        ui.add_enabled_ui(can_go_forward, |ui| {
            if button(ui, "forward", "", Some(Icon::Forward), false)
                .on_hover_text("Forward · Alt+Right")
                .clicked()
            {
                action = Some(ToolbarAction::GoForward);
            }
        });
        if button(ui, "up", "", Some(Icon::Up), false)
            .on_hover_text("Parent folder")
            .clicked()
        {
            action = Some(ToolbarAction::GoUp);
        }
        let available = (ui.available_width() - 45.0).max(50.0);
        ui.allocate_ui_with_layout(
            egui::vec2(available, 32.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_clip_rect(ui.clip_rect().intersect(ui.max_rect()));
                let ancestors: Vec<_> = current_path.ancestors().collect();
                let count = if available > 500.0 {
                    5
                } else if available > 300.0 {
                    3
                } else {
                    1
                };
                for (index, path) in ancestors.iter().take(count).rev().enumerate() {
                    if index > 0 {
                        ui.label(RichText::new("/").color(colors::MUTED));
                    }
                    let label = path
                        .file_name()
                        .unwrap_or(path.as_os_str())
                        .to_string_lossy();
                    let response = button(
                        ui,
                        &format!("breadcrumb:{}", path.display()),
                        &crate::ui::truncate_name(if label == "/" { "Root" } else { &label }, 20),
                        None,
                        *path == current_path,
                    )
                    .on_hover_text(path.display().to_string());
                    if response.clicked() {
                        action = Some(ToolbarAction::Navigate(path.to_path_buf()));
                    }
                }
            },
        );
        if button(ui, "location", "…", None, false)
            .on_hover_text("Go to location · Ctrl+L")
            .clicked()
        {
            action = Some(ToolbarAction::Location);
        }
    });
    ui.horizontal_wrapped(|ui| {
        let width = (ui.available_width() - 250.0).clamp(90.0, 380.0);
        let response = ui.add(
            egui::TextEdit::singleline(search_query)
                .id(egui::Id::new("file_search"))
                .hint_text("Filter files · Ctrl+F")
                .desired_width(width)
                .margin(Vec2::new(10.0, 7.0)),
        );
        crate::ui::expose_control(ui, "search", "Filter files", &response);
        if response.changed() {
            action = Some(ToolbarAction::Search(search_query.clone()));
        }
        if button(ui, "hidden", "Hidden", None, show_hidden)
            .on_hover_text("Show hidden files · Ctrl+H")
            .clicked()
        {
            action = Some(ToolbarAction::ToggleHidden);
        }
        if button(
            ui,
            "list",
            "",
            Some(Icon::List),
            *view_mode == ViewMode::List,
        )
        .on_hover_text("List view")
        .clicked()
        {
            action = Some(ToolbarAction::SetViewMode(ViewMode::List));
        }
        if button(
            ui,
            "grid",
            "",
            Some(Icon::Grid),
            *view_mode == ViewMode::Grid,
        )
        .on_hover_text("Card view")
        .clicked()
        {
            action = Some(ToolbarAction::SetViewMode(ViewMode::Grid));
        }
        if button(ui, "folder", "New folder", Some(Icon::Plus), false).clicked() {
            action = Some(ToolbarAction::NewFolder);
        }
        let more = button(ui, "toolbar_more", "More", Some(Icon::Plus), *more_open);
        let more = if *more_open {
            more
        } else {
            more.on_hover_text("More file and workspace actions")
        };
        let more_rect = more.rect;
        if more.clicked() {
            *more_open = !*more_open;
        }
        if *more_open {
            let popup = egui::Area::new(egui::Id::new("toolbar_more_popup"))
                .fixed_pos(more_rect.right_bottom() - egui::vec2(196.0, 0.0))
                .constrain(true)
                .order(egui::Order::Foreground)
                .show(ui.ctx(), |ui| {
                    egui::Frame::none()
                        .fill(colors::DEEP_CHARCOAL)
                        .stroke(egui::Stroke::new(1.0_f32, colors::ELECTRIC_BLUE))
                        .inner_margin(egui::Margin::same(5.0))
                        .show(ui, |ui| {
                            ui.set_width(186.0);
                            for (id, label, icon, value) in [
                                ("new_file", "New file", Icon::Plus, ToolbarAction::NewFile),
                                (
                                    "open_terminal",
                                    "Open terminal",
                                    Icon::Terminal,
                                    ToolbarAction::OpenTerminal,
                                ),
                                ("refresh", "Refresh", Icon::Forward, ToolbarAction::Refresh),
                                (
                                    "appearance",
                                    "Appearance",
                                    Icon::Peek,
                                    ToolbarAction::Appearance,
                                ),
                            ] {
                                if crate::ui::context_menu::menu_item(
                                    ui,
                                    &format!("toolbar_{id}"),
                                    label,
                                    icon,
                                    colors::ELECTRIC_BLUE,
                                ) {
                                    action = Some(value);
                                    *more_open = false;
                                }
                            }
                        });
                });
            if ui.ctx().input(|input| input.pointer.primary_clicked()) {
                if let Some(pointer) = ui.ctx().input(|input| input.pointer.interact_pos()) {
                    if !more_rect.contains(pointer) && !popup.response.rect.contains(pointer) {
                        *more_open = false;
                    }
                }
            }
        }
    });
    action
}
pub fn render_bulk_operations(ui: &mut Ui, selected_count: usize) -> Option<BulkAction> {
    if selected_count == 0 {
        return None;
    }
    let mut action = None;
    egui::Frame::none()
        .fill(colors::DARK_SLATE)
        .rounding(8.0)
        .inner_margin(6.0)
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(format!("{selected_count} selected"))
                        .color(colors::ELECTRIC_BLUE),
                );
                for (id, label, icon, value) in [
                    (
                        "archive_selection",
                        "ZIP",
                        Icon::Archive,
                        BulkAction::CreateZip,
                    ),
                    (
                        "archive_selection_7z",
                        "7z",
                        Icon::Archive,
                        BulkAction::Create7z,
                    ),
                    ("copy_selection", "Copy", Icon::Copy, BulkAction::Copy),
                    ("cut_selection", "Cut", Icon::Cut, BulkAction::Cut),
                    (
                        "delete_selection",
                        "Delete…",
                        Icon::Delete,
                        BulkAction::Delete,
                    ),
                    (
                        "select_all",
                        "Select all",
                        Icon::Plus,
                        BulkAction::SelectAll,
                    ),
                    (
                        "clear_selection",
                        "Clear",
                        Icon::Delete,
                        BulkAction::SelectNone,
                    ),
                ] {
                    if button(ui, id, label, Some(icon), false).clicked() {
                        action = Some(value);
                    }
                }
            });
        });
    action
}

/// Bulk operation actions.
#[derive(Debug, Clone)]
pub enum BulkAction {
    CreateZip,
    Create7z,
    Copy,
    Cut,
    Delete,
    SelectAll,
    SelectNone,
}

#[cfg(test)]
mod accessibility_tests {
    use super::*;
    #[test]
    fn focused_vector_controls_have_accessibility_nodes() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let output = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                button(ui, "test_back", "Back", Some(Icon::Back), false).request_focus();
            });
        });
        let update = output.platform_output.accesskit_update.unwrap();
        assert!(
            update
                .nodes
                .iter()
                .any(|(id, node)| *id == update.focus && node.name() == Some("Back")),
            "Focused custom control must appear in the accessibility tree"
        );
    }
}
