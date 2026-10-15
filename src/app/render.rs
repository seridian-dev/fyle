use super::*;

impl FileManager {
    pub(super) fn show_dialogs(&mut self, ctx: &egui::Context) {
        let result = match &self.dialog {
            DialogType::None => return,
            DialogType::ConfirmDelete(paths) => show_delete_dialog(ctx, paths),
            DialogType::ConfirmCopy(sources, dest) => show_copy_dialog(ctx, sources, dest),
            DialogType::ConfirmMove(sources, dest) => show_move_dialog(ctx, sources, dest),
            DialogType::FileProperties(entry) => show_properties_dialog(ctx, entry),
            DialogType::CreateFolder => {
                show_create_folder_dialog(ctx, &self.current_path, &mut self.create_folder_state)
            }
            DialogType::CreateFile => {
                show_create_file_dialog(ctx, &self.current_path, &mut self.create_file_state)
            }
            DialogType::Rename(old_path, old_name) => {
                if self.rename_state.is_none() {
                    self.rename_state = Some(RenameState::new(old_name));
                }
                if let Some(ref mut state) = self.rename_state {
                    show_rename_dialog(ctx, old_path, old_name, state)
                } else {
                    DialogResult::Closed
                }
            }
        };

        match result {
            DialogResult::Open => {}
            DialogResult::Closed => {
                self.dialog = DialogType::None;
                self.rename_state = None;
            }
            DialogResult::Success => {
                self.dialog = DialogType::None;
                self.rename_state = None;
                // Clear clipboard after successful move
                if matches!(self.clipboard_operation, Some(ClipboardOperation::Cut)) {
                    self.clipboard.clear();
                    self.clipboard_operation = None;
                }
                self.load_entries();
            }
            DialogResult::Error(msg) => {
                self.error_message = Some(msg);
                self.dialog = DialogType::None;
                self.rename_state = None;
            }
        }
    }

    pub(super) fn show_error(&mut self, ctx: &egui::Context) {
        if let Some(ref message) = self.error_message.clone() {
            if !show_error_dialog(ctx, message) {
                self.error_message = None;
            }
        }
    }

    /// Render the bottom details panel.
    fn render_details_panel(&self, ui: &mut egui::Ui) {
        ui.heading("File Details");

        if let Some(selected) = self.entries.iter().find(|e| e.selected) {
            egui::Grid::new("details_grid")
                .num_columns(2)
                .spacing([10.0, 4.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("Name:").color(colors::CYBER_GREEN));
                    ui.label(&selected.name);
                    ui.end_row();

                    ui.label(RichText::new("Size:").color(colors::ELECTRIC_BLUE));
                    ui.label(format_size(selected.size));
                    ui.end_row();

                    ui.label(RichText::new("Type:").color(colors::MAGENTA));
                    ui.label(&selected.file_type);
                    ui.end_row();

                    ui.label(RichText::new("Modified:").color(colors::AMBER));
                    ui.label(selected.modified.format("%Y-%m-%d %H:%M:%S").to_string());
                    ui.end_row();

                    ui.label(RichText::new("Path:").color(colors::CYAN));
                    ui.label(selected.path.display().to_string());
                    ui.end_row();
                });
        } else {
            ui.label("Select a file to see details here");
        }
    }
}

impl eframe::App for FileManager {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle keyboard shortcuts
        self.handle_keyboard(ctx);

        // Show error messages
        self.show_error(ctx);

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            let can_go_back = !self.history.is_empty();
            let can_go_forward = !self.forward_history.is_empty();

            if let Some(action) = render_toolbar(
                ui,
                &self.current_path,
                self.entries.len(),
                self.show_hidden,
                &self.view_mode,
                &mut self.search_query,
                can_go_back,
                can_go_forward,
            ) {
                self.handle_toolbar_action(action);
            }
        });

        // Sidebar
        egui::SidePanel::left("sidebar").show(ctx, |ui| {
            if let Some(action) = render_sidebar(ui, &self.bookmarks, &self.current_path) {
                self.handle_sidebar_action(action);
            }
        });

        // Bottom details panel
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            self.render_details_panel(ui);
        });

        // Main content area
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Files");

            // Bulk operations toolbar
            if let Some(action) = render_bulk_operations(ui, self.selected_entries.len()) {
                self.handle_bulk_action(action);
            }

            // Clone entries to avoid borrow issues
            let entries_snapshot = self.entries.clone();

            // Render the appropriate view and collect actions
            let actions = match self.view_mode {
                ViewMode::List => {
                    render_list_view(ui, &entries_snapshot, &self.sort_by, &self.sort_order)
                }
                ViewMode::Grid => render_grid_view(ui, &entries_snapshot),
            };

            // Handle view actions
            for action in actions {
                self.handle_view_action(action);
            }
        });

        // Show context menu
        if self.context_menu.visible {
            if let Some(ref target) = self.context_menu.target.clone() {
                let action = show_context_menu(ctx, self.context_menu.position, target);
                self.handle_context_menu_action(action);
            }
        }

        self.show_dialogs(ctx);
    }
}
