//! Fyle-styled context menu for file operations.

use crate::core::open_with_recommended;
use crate::theme::colors;
use crate::types::FileEntry;
use crate::ui::constants::CONTEXT_MENU_WIDTH;
use crate::ui::icons::{self, Icon};
use eframe::egui::{self, Color32, Context, Pos2, Stroke, Ui};
use std::path::PathBuf;

/// Actions that can be triggered from the context menu.
#[derive(Debug, Clone, PartialEq)]
pub enum ContextMenuAction {
    None,
    Open,
    Navigate(PathBuf),
    Copy,
    Cut,
    Rename,
    Delete,
    Properties,
    AddBookmark,
    OpenTerminal,
    Close,
    OpenError(String),
}

fn menu_item(ui: &mut Ui, id: &str, label: &str, icon: Icon, accent: Color32) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 29.0), egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, false, label));

    let fill = if response.is_pointer_button_down_on() {
        Color32::from_rgb(10, 51, 66)
    } else if response.hovered() || response.has_focus() {
        colors::MIDNIGHT_PURPLE
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 2.0, fill);
    if response.hovered() || response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            2.0,
            Stroke::new(1.0_f32, accent.linear_multiply(0.75)),
        );
        ui.painter().rect_filled(
            egui::Rect::from_min_max(rect.min, egui::pos2(rect.left() + 2.0, rect.bottom())),
            0.0,
            accent,
        );
    }
    icons::paint(
        ui.painter(),
        egui::Rect::from_center_size(
            egui::pos2(rect.left() + 17.0, rect.center().y),
            egui::vec2(16.0, 16.0),
        ),
        icon,
        if response.hovered() {
            accent
        } else {
            colors::MUTED
        },
    );
    ui.painter().text(
        egui::pos2(rect.left() + 34.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::monospace(12.0),
        if response.hovered() {
            colors::TEXT
        } else {
            colors::TEXT.linear_multiply(0.88)
        },
    );
    crate::ui::expose_control(ui, id, label, &response);
    response.clicked()
}

/// Show the context menu for a file entry.
pub fn show_context_menu(ctx: &Context, pos: Pos2, target: &FileEntry) -> ContextMenuAction {
    let mut action = ContextMenuAction::None;
    let mut menu_rect = egui::Rect::NOTHING;

    egui::Area::new(egui::Id::new("context_menu"))
        .fixed_pos(pos)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let response = egui::Frame::none()
                .fill(colors::DEEP_CHARCOAL)
                .stroke(Stroke::new(
                    1.0_f32,
                    colors::ELECTRIC_BLUE.linear_multiply(0.8),
                ))
                .inner_margin(egui::Margin::same(5.0))
                .show(ui, |ui| {
                    ui.set_width(CONTEXT_MENU_WIDTH);
                    ui.label(
                        egui::RichText::new("// FILE ACTIONS")
                            .monospace()
                            .size(9.0)
                            .color(colors::ELECTRIC_BLUE),
                    );
                    ui.add_space(3.0);

                    if menu_item(
                        ui,
                        "context_open",
                        "Open",
                        Icon::Open,
                        colors::ELECTRIC_BLUE,
                    ) {
                        if target.is_dir {
                            action = ContextMenuAction::Navigate(target.path.clone());
                        } else {
                            match open_with_recommended(&target.path) {
                                Ok(()) => action = ContextMenuAction::Open,
                                Err(error) => {
                                    action = ContextMenuAction::OpenError(error.to_string())
                                }
                            }
                        }
                    }
                    ui.separator();
                    if menu_item(ui, "context_copy", "Copy", Icon::Copy, colors::CYBER_GREEN) {
                        action = ContextMenuAction::Copy;
                    }
                    if menu_item(ui, "context_cut", "Cut", Icon::Cut, colors::AMBER) {
                        action = ContextMenuAction::Cut;
                    }
                    if menu_item(
                        ui,
                        "context_rename",
                        "Rename",
                        Icon::Rename,
                        colors::ELECTRIC_BLUE,
                    ) {
                        action = ContextMenuAction::Rename;
                    }
                    ui.separator();
                    if menu_item(
                        ui,
                        "context_delete",
                        "Delete",
                        Icon::Delete,
                        colors::CRIMSON,
                    ) {
                        action = ContextMenuAction::Delete;
                    }
                    ui.separator();
                    if menu_item(
                        ui,
                        "context_properties",
                        "Properties",
                        Icon::Properties,
                        colors::CYBER_GREEN,
                    ) {
                        action = ContextMenuAction::Properties;
                    }
                    if menu_item(
                        ui,
                        "context_bookmark",
                        "Add to Bookmarks",
                        Icon::Bookmark,
                        colors::AMBER,
                    ) {
                        action = ContextMenuAction::AddBookmark;
                    }
                    if menu_item(
                        ui,
                        "context_terminal",
                        "Open in Terminal",
                        Icon::Terminal,
                        colors::MAGENTA,
                    ) {
                        action = ContextMenuAction::OpenTerminal;
                    }
                });
            menu_rect = response.response.rect;
        });

    if ctx.input(|input| input.pointer.primary_clicked()) {
        if let Some(pointer_pos) = ctx.input(|input| input.pointer.interact_pos()) {
            if !menu_rect.contains(pointer_pos) && action == ContextMenuAction::None {
                return ContextMenuAction::Close;
            }
        }
    }
    action
}

/// State for tracking context menu visibility.
#[derive(Default)]
pub struct ContextMenuState {
    pub visible: bool,
    pub position: Pos2,
    pub target: Option<FileEntry>,
}

impl ContextMenuState {
    pub fn show(&mut self, pos: Pos2, target: FileEntry) {
        self.visible = true;
        self.position = pos;
        self.target = Some(target);
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.target = None;
    }
}
