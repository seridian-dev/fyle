//! Dialog components for file operations.

use crate::core::{format_size, job::FileOperation};

const DELETE_LABEL: &str = if cfg!(feature = "trash") {
    "Move to Trash"
} else {
    "Delete Permanently"
};

use crate::theme::colors;
use crate::types::FileEntry;

#[allow(unused_imports)] // DialogType used for documentation reference
use crate::types::DialogType;
use eframe::egui::{self, Context, RichText};
use std::path::{Path, PathBuf};

/// Result of dialog interaction.
pub enum DialogResult {
    /// Dialog is still open.
    Open,
    /// Dialog was closed/cancelled.
    Closed,
    /// Operation completed successfully.
    Execute(FileOperation),
}

/// Show the delete confirmation dialog.
pub fn show_delete_dialog(ctx: &Context, paths: &[PathBuf]) -> DialogResult {
    let mut result = DialogResult::Open;
    let mut open = true;

    egui::Window::new(DELETE_LABEL)
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(
                RichText::new(format!("{} {} item(s)?", DELETE_LABEL, paths.len()))
                    .color(colors::AMBER),
            );

            ui.add_space(8.0);

            egui::ScrollArea::vertical()
                .max_height(200.0)
                .show(ui, |ui| {
                    for path in paths {
                        ui.label(format!("  • {}", path.display()));
                    }
                });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                let response = ui.button(RichText::new(DELETE_LABEL).color(colors::CRIMSON));
                crate::ui::expose_control(ui, "delete_confirm", DELETE_LABEL, &response);
                if response.clicked() {
                    result = DialogResult::Execute(FileOperation::Remove(paths.to_vec()));
                }
                let response = ui.button("Cancel");
                crate::ui::expose_control(ui, "delete_cancel", "Cancel", &response);
                if response.clicked() {
                    result = DialogResult::Closed;
                }
            });
        });

    if !open {
        return DialogResult::Closed;
    }

    result
}

/// Show the copy confirmation dialog.
pub fn show_copy_dialog(ctx: &Context, sources: &[PathBuf], destination: &Path) -> DialogResult {
    let mut result = DialogResult::Open;
    let mut open = true;

    egui::Window::new("Confirm Copy")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(format!(
                "Copy {} item(s) to {}?",
                sources.len(),
                destination.display()
            ));

            ui.add_space(8.0);

            egui::ScrollArea::vertical()
                .max_height(200.0)
                .show(ui, |ui| {
                    for source in sources {
                        ui.label(format!("  • {}", source.display()));
                    }
                });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                let response = ui.button(RichText::new("Copy").color(colors::CYBER_GREEN));
                crate::ui::expose_control(ui, "copy_confirm", "Confirm copy", &response);
                if response.clicked() {
                    result = DialogResult::Execute(FileOperation::Copy(
                        sources.to_vec(),
                        destination.to_path_buf(),
                    ));
                }
                if ui.button("Cancel").clicked() {
                    result = DialogResult::Closed;
                }
            });
        });

    if !open {
        return DialogResult::Closed;
    }

    result
}

/// Show the move confirmation dialog.
pub fn show_move_dialog(ctx: &Context, sources: &[PathBuf], destination: &Path) -> DialogResult {
    let mut result = DialogResult::Open;
    let mut open = true;

    egui::Window::new("Confirm Move")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(format!(
                "Move {} item(s) to {}?",
                sources.len(),
                destination.display()
            ));

            ui.add_space(8.0);

            egui::ScrollArea::vertical()
                .max_height(200.0)
                .show(ui, |ui| {
                    for source in sources {
                        ui.label(format!("  • {}", source.display()));
                    }
                });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                if ui
                    .button(RichText::new("Move").color(colors::AMBER))
                    .clicked()
                {
                    result = DialogResult::Execute(FileOperation::Move(
                        sources.to_vec(),
                        destination.to_path_buf(),
                    ));
                }
                if ui.button("Cancel").clicked() {
                    result = DialogResult::Closed;
                }
            });
        });

    if !open {
        return DialogResult::Closed;
    }

    result
}

/// Show the file properties dialog.
pub fn show_properties_dialog(ctx: &Context, entry: &FileEntry) -> DialogResult {
    let mut result = DialogResult::Open;
    let mut open = true;

    egui::Window::new("Properties")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            egui::Grid::new("properties_grid")
                .num_columns(2)
                .spacing([10.0, 8.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("Name:").color(colors::CYBER_GREEN));
                    ui.label(&entry.name);
                    ui.end_row();

                    ui.label(RichText::new("Path:").color(colors::ELECTRIC_BLUE));
                    ui.label(entry.path.display().to_string());
                    ui.end_row();

                    ui.label(RichText::new("Type:").color(colors::MAGENTA));
                    ui.label(&entry.file_type);
                    ui.end_row();

                    ui.label(RichText::new("Size:").color(colors::AMBER));
                    ui.label(format_size(entry.size));
                    ui.end_row();

                    ui.label(RichText::new("Modified:").color(colors::CYAN));
                    ui.label(entry.modified.format("%Y-%m-%d %H:%M:%S").to_string());
                    ui.end_row();

                    ui.label(RichText::new("Permissions:").color(colors::MAGENTA));
                    if let Ok(metadata) = std::fs::metadata(&entry.path) {
                        let readonly = metadata.permissions().readonly();
                        ui.label(if readonly { "Read-only" } else { "Read-write" });
                    } else {
                        ui.label("Unknown");
                    }
                    ui.end_row();
                });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            if ui.button("Close").clicked() {
                result = DialogResult::Closed;
            }
        });

    if !open {
        return DialogResult::Closed;
    }

    result
}

/// State for create folder dialog.
#[derive(Default)]
pub struct CreateFolderState {
    pub name: String,
}

/// Show the create folder dialog.
pub fn show_create_folder_dialog(
    ctx: &Context,
    current_path: &Path,
    state: &mut CreateFolderState,
) -> DialogResult {
    let mut result = DialogResult::Open;
    let mut open = true;

    egui::Window::new("Create Folder")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label("Folder name:");
            let response = ui.text_edit_singleline(&mut state.name);
            crate::ui::expose_control(ui, "dialog_name", "Name", &response);

            // Focus the text field
            if response.lost_focus()
                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                && !state.name.is_empty()
            {
                result = DialogResult::Execute(FileOperation::CreateFolder(
                    current_path.to_path_buf(),
                    state.name.clone(),
                ));
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                if ui
                    .button(RichText::new("Create").color(colors::CYBER_GREEN))
                    .clicked()
                    && !state.name.is_empty()
                {
                    result = DialogResult::Execute(FileOperation::CreateFolder(
                        current_path.to_path_buf(),
                        state.name.clone(),
                    ));
                }
                if ui.button("Cancel").clicked() {
                    state.name.clear();
                    result = DialogResult::Closed;
                }
            });
        });

    if !open {
        state.name.clear();
        return DialogResult::Closed;
    }

    result
}

/// State for create file dialog.
#[derive(Default)]
pub struct CreateFileState {
    pub name: String,
}

/// Show the create file dialog.
pub fn show_create_file_dialog(
    ctx: &Context,
    current_path: &Path,
    state: &mut CreateFileState,
) -> DialogResult {
    let mut result = DialogResult::Open;
    let mut open = true;

    egui::Window::new("Create File")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label("File name:");
            let response = ui.text_edit_singleline(&mut state.name);
            crate::ui::expose_control(ui, "dialog_name", "Name", &response);

            // Enter key to confirm
            if response.lost_focus()
                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                && !state.name.is_empty()
            {
                result = DialogResult::Execute(FileOperation::CreateFile(
                    current_path.to_path_buf(),
                    state.name.clone(),
                ));
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                if ui
                    .button(RichText::new("Create").color(colors::ELECTRIC_BLUE))
                    .clicked()
                    && !state.name.is_empty()
                {
                    result = DialogResult::Execute(FileOperation::CreateFile(
                        current_path.to_path_buf(),
                        state.name.clone(),
                    ));
                }
                if ui.button("Cancel").clicked() {
                    state.name.clear();
                    result = DialogResult::Closed;
                }
            });
        });

    if !open {
        state.name.clear();
        return DialogResult::Closed;
    }

    result
}

/// State for rename dialog.
pub struct RenameState {
    pub new_name: String,
}

impl RenameState {
    pub fn new(current_name: &str) -> Self {
        Self {
            new_name: current_name.to_string(),
        }
    }
}

/// Show the rename dialog.
pub fn show_rename_dialog(
    ctx: &Context,
    old_path: &Path,
    old_name: &str,
    state: &mut RenameState,
) -> DialogResult {
    let mut result = DialogResult::Open;
    let mut open = true;

    egui::Window::new("Rename")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(RichText::new("New name:").color(colors::CYBER_GREEN));
            let response = ui.text_edit_singleline(&mut state.new_name);
            crate::ui::expose_control(ui, "dialog_name", "New name", &response);

            // Enter key to confirm
            if response.lost_focus()
                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                && !state.new_name.is_empty()
                && state.new_name != old_name
            {
                result = DialogResult::Execute(FileOperation::Rename(
                    old_path.to_path_buf(),
                    state.new_name.clone(),
                ));
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                let can_rename = !state.new_name.is_empty() && state.new_name != old_name;

                if ui
                    .add_enabled(
                        can_rename,
                        egui::Button::new(RichText::new("Rename").color(colors::CYBER_GREEN)),
                    )
                    .clicked()
                {
                    result = DialogResult::Execute(FileOperation::Rename(
                        old_path.to_path_buf(),
                        state.new_name.clone(),
                    ));
                }
                if ui.button("Cancel").clicked() {
                    result = DialogResult::Closed;
                }
            });
        });

    if !open {
        return DialogResult::Closed;
    }

    result
}

/// Show an error message dialog.
/// Returns true if the dialog should remain open.
pub fn show_error_dialog(ctx: &Context, message: &str) -> bool {
    let mut should_close = false;

    egui::Window::new("Error")
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(RichText::new(message).color(colors::CRIMSON));

            ui.add_space(8.0);

            if ui.button("OK").clicked() {
                should_close = true;
            }
        });

    !should_close
}
