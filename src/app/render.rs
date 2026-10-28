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

    /// Highlight the window while files from another application hover it.
    /// egui reports hovered drops per frame; the count says what will be
    /// copied when the drag is released.
    fn render_drop_overlay(&self, ctx: &egui::Context) {
        let count = ctx.input(|i| i.raw.hovered_files.len());
        if count == 0
            || self.storage.open
            || self.operation.is_some()
            || self.dialog != DialogType::None
        {
            return;
        }
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            egui::Id::new("external_drop_overlay"),
        ));
        let rect = ctx.screen_rect().shrink(18.0);
        painter.rect_filled(
            rect,
            10.0,
            egui::Color32::from_rgba_unmultiplied(2, 32, 24, 190),
        );
        painter.rect_stroke(rect, 10.0, egui::Stroke::new(2.0_f32, colors::CYBER_GREEN));
        let label = format!(
            "DROP {} ITEM{} INTO THIS FOLDER",
            count,
            if count == 1 { "" } else { "S" }
        );
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::monospace(16.0),
            colors::CYBER_GREEN,
        );
    }

    /// A stable status line keeps the file viewport from jumping on selection.
    fn render_details_panel(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if self.operation.is_some() {
                ui.label(
                    RichText::new("Working…")
                        .size(12.0)
                        .color(colors::CYBER_GREEN),
                );
            } else if self.selected_entries.len() == 1 {
                if let Some(entry) = self.entries.iter().find(|e| e.selected) {
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!(
                                "{}  ·  {}",
                                entry.name,
                                if entry.is_dir {
                                    "Folder".into()
                                } else {
                                    format_size(entry.size)
                                }
                            ))
                            .size(12.0)
                            .color(colors::MUTED),
                        )
                        .truncate(true),
                    );
                }
            } else if !self.selected_entries.is_empty() {
                ui.label(
                    RichText::new(format!(
                        "{} items selected  ·  {} items in folder",
                        self.selected_entries.len(),
                        self.entries.len()
                    ))
                    .size(12.0)
                    .color(colors::MUTED),
                );
            } else {
                ui.label(
                    RichText::new(format!("{} items", self.entries.len()))
                        .size(12.0)
                        .color(colors::MUTED),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.small(
                    RichText::new("Ctrl+P Jump   ·   Ctrl+T Tab   ·   Space Peek")
                        .color(colors::MUTED),
                );
            });
        });
    }
}

impl eframe::App for FileManager {
    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        if let (Some(shader), Some(gl)) = (self.shader.take(), gl) {
            shader.destroy(gl);
        }
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        self.automation_input(ctx, input);
        self.workspace_input(input);
    }
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(target_os = "macos")]
        if self
            .native_menu
            .as_ref()
            .is_some_and(crate::native_menu::NativeMenu::take_settings_request)
        {
            if self.appearance_open {
                self.appearance_open = false;
            } else {
                self.appearance_open = true;
                self.preferences_section = PreferencesSection::General;
            }
        }
        if self.operation.is_some() && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.error_message = Some(
                "Please wait for the current file operation to finish before closing Fyle.".into(),
            );
        }
        self.poll_directory();
        self.poll_operation();
        self.storage.poll();
        self.poll_preview();
        self.editor.poll_worker();
        self.review.poll();
        if self.loading
            || self.operation.is_some()
            || self.editor.is_loading()
            || self.editor.is_saving()
            || self.review.is_busy()
            || self.storage.is_scanning()
        {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        // Handle keyboard shortcuts
        if self.operation.is_none() {
            self.handle_keyboard(ctx);
            self.handle_external_drop(ctx);
        }

        // Show error messages
        self.show_error(ctx);
        self.render_drop_overlay(ctx);

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.set_enabled(
                self.operation.is_none()
                    && !self.palette.open
                    && !self.location_open
                    && !self.appearance_open,
            );
            #[cfg(target_os = "macos")]
            ui.add_space(26.0);
            self.render_tabs(ui);
            if !self.storage.open {
                ui.add_space(4.0);
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
                    &mut self.toolbar_more_open,
                ) {
                    self.handle_toolbar_action(action);
                }
            }
        });

        // Sidebar
        egui::SidePanel::left("sidebar")
            .default_width(196.0)
            .min_width(160.0)
            .max_width(260.0)
            .show(ctx, |ui| {
                ui.set_enabled(
                    self.operation.is_none()
                        && !self.palette.open
                        && !self.location_open
                        && !self.appearance_open,
                );
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
        if self.appearance.show_status_bar {
            egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
                if self.storage.open {
                    self.storage.render_status(ui);
                } else {
                    self.render_details_panel(ui);
                }
            });
        }

        self.render_preview(ctx);

        // Main content area
        egui::CentralPanel::default()
            .frame(
                egui::Frame::central_panel(&ctx.style())
                    .fill(egui::Color32::from_rgb(7, 16, 24))
                    .inner_margin(14.0),
            )
            .show(ctx, |ui| {
                ui.set_enabled(
                    self.operation.is_none()
                        && !self.palette.open
                        && !self.location_open
                        && !self.appearance_open,
                );
                self.render_background(ui);
                if self.storage.open {
                    if let Some(action) = self.storage.render(ui) {
                        match action {
                            #[cfg(feature = "trash")]
                            storage::StorageAction::Trash(path) => {
                                self.dialog = DialogType::ConfirmDelete(vec![path]);
                            }
                            storage::StorageAction::Preferences => {
                                self.appearance_open = true;
                                self.preferences_section = PreferencesSection::General;
                            }
                        }
                    }
                } else {
                    ui.label(
                        RichText::new("DIRECTORY // LOCAL ACCESS")
                            .monospace()
                            .size(10.0)
                            .color(colors::ELECTRIC_BLUE),
                    );
                    ui.horizontal(|ui| {
                        ui.heading(
                            self.current_path
                                .file_name()
                                .unwrap_or(self.current_path.as_os_str())
                                .to_string_lossy(),
                        );
                        if self.operation.is_some() {
                            ui.spinner();
                            ui.label("Working…");
                        }
                        ui.label(
                            RichText::new(format!(
                                "{} ITEMS / {:02} SELECTED",
                                self.entries.len(),
                                self.selected_entries.len()
                            ))
                            .monospace()
                            .size(11.0)
                            .color(colors::CYBER_GREEN),
                        );
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
                            crate::ui::expose_control(
                                ui,
                                "clear_filters",
                                "Clear filters",
                                &response,
                            );
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
        self.render_location(ctx);
        self.render_appearance(ctx);
        self.editor.render(ctx);
        self.review.render(ctx, self.current_path.clone());
        self.terminal
            .render(ctx, &self.current_path, &self.appearance);
        self.poll_preview();
        if self.preview_loading {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
        self.automation_finish(ctx);
    }
}
