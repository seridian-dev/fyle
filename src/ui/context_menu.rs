//! Context menu component for file operations.

use crate::core::open_with_recommended;
use crate::theme::colors;
use crate::types::FileEntry;
use crate::ui::constants::CONTEXT_MENU_WIDTH;
use eframe::egui::{self, Context, Pos2, Stroke};
use std::path::PathBuf;

/// Actions that can be triggered from the context menu.
#[derive(Debug, Clone, PartialEq)]
pub enum ContextMenuAction {
    /// No action taken.
    None,
    /// Open the file/directory.
    Open,
    /// Navigate to directory.
    Navigate(PathBuf),
    /// Copy to clipboard.
    Copy,
    /// Cut to clipboard.
    Cut,
    /// Initiate rename.
    Rename,
    /// Delete the item.
    Delete,
    /// Show properties dialog.
    Properties,
    /// Add to bookmarks.
    AddBookmark,
    /// Open in terminal.
    OpenTerminal,
    /// Close the menu.
    Close,
    /// Open failed with error.
    OpenError(String),
}

/// Show the context menu for a file entry.
///
/// Returns the action selected, or `ContextMenuAction::None` if the menu is still open.
pub fn show_context_menu(ctx: &Context, pos: Pos2, target: &FileEntry) -> ContextMenuAction {
    let mut action = ContextMenuAction::None;

    egui::Area::new(egui::Id::new("context_menu"))
        .fixed_pos(pos)
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(colors::DARK_SLATE)
                .stroke(Stroke::new(1.0_f32, colors::ELECTRIC_BLUE))
                .inner_margin(4.0)
                .show(ui, |ui| {
                    ui.set_width(CONTEXT_MENU_WIDTH);

                    // Open button
                    if ui.button("Open").clicked() {
                        if target.is_dir {
                            action = ContextMenuAction::Navigate(target.path.clone());
                        } else {
                            match open_with_recommended(&target.path) {
                                Ok(()) => action = ContextMenuAction::Open,
                                Err(e) => action = ContextMenuAction::OpenError(e.to_string()),
                            }
                        }
                    }

                    ui.separator();

                    // Copy button
                    if ui.button("Copy").clicked() {
                        action = ContextMenuAction::Copy;
                    }

                    // Cut button
                    if ui.button("Cut").clicked() {
                        action = ContextMenuAction::Cut;
                    }

                    // Rename button
                    if ui.button("Rename").clicked() {
                        action = ContextMenuAction::Rename;
                    }

                    ui.separator();

                    // Delete button
                    if ui.button("Delete").clicked() {
                        action = ContextMenuAction::Delete;
                    }

                    ui.separator();

                    // Properties button
                    if ui.button("Properties").clicked() {
                        action = ContextMenuAction::Properties;
                    }

                    // Add to Bookmarks button
                    if ui.button("Add to Bookmarks").clicked() {
                        action = ContextMenuAction::AddBookmark;
                    }

                    // Open in Terminal button
                    if ui.button("Open in Terminal").clicked() {
                        action = ContextMenuAction::OpenTerminal;
                    }
                });
        });

    // Close context menu on click outside
    if ctx.input(|i| i.pointer.primary_clicked()) {
        let menu_rect = egui::Rect::from_min_size(pos, egui::vec2(CONTEXT_MENU_WIDTH, 200.0));
        if let Some(pointer_pos) = ctx.input(|i| i.pointer.interact_pos()) {
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
    /// Show the context menu at the given position for the target entry.
    pub fn show(&mut self, pos: Pos2, target: FileEntry) {
        self.visible = true;
        self.position = pos;
        self.target = Some(target);
    }

    /// Hide the context menu.
    pub fn hide(&mut self) {
        self.visible = false;
        self.target = None;
    }
}
