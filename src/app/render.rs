use super::*;

impl FileManager {
    pub(super) fn show_dialogs(&mut self, ctx: &egui::Context) {
        if self.operation.is_some() {
            return;
        }
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
            DialogResult::Execute(operation) => {
                self.context_menu.hide();
                self.operation = Some(fyle::core::job::OperationJob::start(operation));
                self.dialog = DialogType::None;
                self.rename_state = None;
                self.create_folder_state.name.clear();
                self.create_file_state.name.clear();
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

        if let Some(selected) = self
            .selected_entries
            .iter()
            .next()
            .and_then(|path| self.entries.iter().find(|entry| &entry.path == path))
        {
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
            ui.label("Ctrl+P jump · Ctrl+F filter · Space peek · F5 refresh");
        }
    }
}

impl eframe::App for FileManager {
    fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        self.automation_input(ctx, input);
    }
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.operation.is_some() && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.error_message = Some(
                "Please wait for the current file operation to finish before closing Fyle.".into(),
            );
        }
        self.poll_directory();
        self.poll_operation();
        self.poll_preview();
        if self.loading || self.operation.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        // Handle keyboard shortcuts
        if self.operation.is_none() {
            self.handle_keyboard(ctx);
        }

        // Show error messages
        self.show_error(ctx);

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.set_enabled(self.operation.is_none() && !self.palette.open);
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
            ui.set_enabled(self.operation.is_none() && !self.palette.open);
            if let Some(action) = render_sidebar(
                ui,
                &self.bookmarks,
                &self.current_path,
                &mut self.storage_cache,
            ) {
                self.handle_sidebar_action(action);
            }
        });

        // Bottom details panel
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.set_enabled(self.operation.is_none() && !self.palette.open);
            self.render_details_panel(ui);
        });

        self.render_preview(ctx);

        // Main content area
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.set_enabled(self.operation.is_none() && !self.palette.open);
            ui.horizontal(|ui| {
                ui.heading("Files");
                if self.operation.is_some() {
                    ui.spinner();
                    ui.label("Working…");
                }
                ui.label(format!(
                    "{} items · {} selected",
                    self.entries.len(),
                    self.selected_entries.len()
                ));
                if self.loading {
                    ui.spinner();
                    ui.label("Loading folder…");
                }
            });
            if !self.loading && self.entries.is_empty() {
                if self.search_query.is_empty() {
                    ui.label("A fresh space. Create a file or folder from the toolbar.");
                } else {
                    ui.label("No files match this lens.");
                    let response = ui.button("Clear filters");
                    crate::ui::expose_control(ui, "clear_filters", "Clear filters", &response);
                    if response.clicked() {
                        self.search_query.clear();
                        self.apply_filter();
                    }
                }
            }

            self.render_lenses(ui);

            // Bulk operations toolbar
            if let Some(action) = render_bulk_operations(ui, self.selected_entries.len()) {
                self.handle_bulk_action(action);
            }

            // Render the appropriate view and collect actions
            let actions = match self.view_mode {
                ViewMode::List => {
                    render_list_view(ui, &self.entries, &self.sort_by, &self.sort_order)
                }
                ViewMode::Grid => render_grid_view(ui, &self.entries),
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
        self.render_palette(ctx);
        self.poll_preview();
        if self.preview_loading {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
        self.automation_finish(ctx);
    }
}
