use super::*;
use fyle::core::open_with_recommended;

impl FileManager {
    pub(super) fn open_file(&mut self, path: PathBuf) {
        if fyle::core::editor::is_text_candidate(&path) {
            if let Err(error) = self.editor.open(path) {
                self.error_message = Some(format!("Cannot open in Fyle editor: {error:#}"));
            }
        } else if let Err(error) = open_with_recommended(&path) {
            self.error_message = Some(format!("Failed to open file: {error}"));
        }
    }

    pub(super) fn handle_keyboard(&mut self, ctx: &egui::Context) {
        if self.dialog == DialogType::None
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Comma))
        {
            self.appearance_open = true;
            self.preferences_section = PreferencesSection::General;
            return;
        }
        if self.terminal.is_open() {
            if ctx.input(|i| i.key_pressed(egui::Key::T) && i.modifiers.ctrl && i.modifiers.alt) {
                self.terminal.toggle(&self.current_path);
            }
            return;
        }
        if ctx.input_mut(|i| {
            i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::G)
        }) {
            self.appearance_open = false;
            self.review.open(self.current_path.clone());
            return;
        }
        if self.editor.is_open() {
            return;
        }
        if self.dialog == DialogType::None
            && !self.appearance_open
            && !self.palette.open
            && !self.location_open
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::F))
        {
            ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("file_search")));
            return;
        }
        if self.dialog == DialogType::None
            && !self.appearance_open
            && !self.location_open
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::P))
        {
            if self.palette.open {
                self.palette.open = false;
            } else {
                self.open_palette();
            }
            return;
        }
        if self.dialog == DialogType::None
            && !self.appearance_open
            && !self.palette.open
            && !self.location_open
        {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::L)) {
                self.open_location();
                return;
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::T)) {
                self.new_tab();
                return;
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::W)) {
                self.close_tab(self.active_tab);
                return;
            }
        }
        if self.appearance_open
            || self.location_open
            || self.palette.open
            || self.dialog != DialogType::None
            || ctx.wants_keyboard_input()
        {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Space)) {
            self.preview_open = !self.preview_open;
        }
        // Ctrl+Up - Go to parent
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp) && i.modifiers.ctrl) {
            self.go_up();
        }

        // Ctrl+G - Grid view
        if ctx.input(|i| i.key_pressed(egui::Key::G) && i.modifiers.ctrl) {
            self.view_mode = ViewMode::Grid;
        }

        // Ctrl+A - Select all
        if ctx.input(|i| i.key_pressed(egui::Key::A) && i.modifiers.ctrl) {
            self.select_all();
        }

        // Escape - Deselect all
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.select_none();
            self.context_menu.hide();
        }

        // Delete - Delete selected
        if ctx.input(|i| i.key_pressed(egui::Key::Delete)) {
            let selected = self.get_selected_entries();
            if !selected.is_empty() {
                let paths = selected.into_iter().map(|e| e.path).collect();
                self.dialog = DialogType::ConfirmDelete(paths);
            }
        }

        // Ctrl+C - Copy
        if ctx.input(|i| i.key_pressed(egui::Key::C) && i.modifiers.ctrl) {
            self.copy_to_clipboard();
        }

        // Ctrl+X - Cut
        if ctx.input(|i| i.key_pressed(egui::Key::X) && i.modifiers.ctrl) {
            self.cut_to_clipboard();
        }

        // Ctrl+V - Paste
        if ctx.input(|i| i.key_pressed(egui::Key::V) && i.modifiers.ctrl) {
            self.paste();
        }

        // Ctrl+H - Toggle hidden
        if ctx.input(|i| i.key_pressed(egui::Key::H) && i.modifiers.ctrl) {
            self.show_hidden = !self.show_hidden;
            self.load_entries();
        }

        // Ctrl+T - Open terminal
        if ctx.input(|i| i.key_pressed(egui::Key::T) && i.modifiers.ctrl && i.modifiers.alt) {
            self.terminal.toggle(&self.current_path);
        }

        // F5 - Refresh
        if ctx.input(|i| i.key_pressed(egui::Key::F5)) {
            self.load_entries();
        }

        // Back/Forward shortcuts
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft) && i.modifiers.alt) {
            self.go_back();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight) && i.modifiers.alt) {
            self.go_forward();
        }
    }

    /// Route files dropped in from another application through the same
    /// confirm dialogs as in-app copy/move. Drops are ignored while an
    /// operation or dialog is active; those states already disable mutations.
    pub(super) fn handle_external_drop(&mut self, ctx: &egui::Context) {
        if self.storage.open || self.operation.is_some() || self.dialog != DialogType::None {
            return;
        }
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if dropped.is_empty() {
            return;
        }
        let effect = ctx
            .input(|i| fyle::desktop::dnd::drag_effect(i.raw.modifiers.alt, i.raw.modifiers.shift));
        let paths: Vec<PathBuf> = dropped.into_iter().filter_map(|file| file.path).collect();
        if let Some(intent) = fyle::desktop::dnd::incoming_intent(paths, &self.current_path, effect)
        {
            self.dialog = match intent.effect {
                fyle::desktop::dnd::DragEffect::Copy => {
                    DialogType::ConfirmCopy(intent.paths, self.current_path.clone())
                }
                fyle::desktop::dnd::DragEffect::Move => {
                    DialogType::ConfirmMove(intent.paths, self.current_path.clone())
                }
            };
        }
    }

    /// Handle toolbar actions.
    pub(super) fn handle_toolbar_action(&mut self, action: ToolbarAction) {
        match action {
            ToolbarAction::Navigate(path) => self.navigate_to(path),
            ToolbarAction::Appearance => {
                self.appearance_open = true;
                self.preferences_section = PreferencesSection::General;
            }
            ToolbarAction::Location => self.open_location(),
            ToolbarAction::GoBack => self.go_back(),
            ToolbarAction::Refresh => self.load_entries(),
            ToolbarAction::Jump => self.open_palette(),
            ToolbarAction::Review => {
                self.appearance_open = false;
                self.review.open(self.current_path.clone());
            }
            ToolbarAction::TogglePreview => self.preview_open = !self.preview_open,
            ToolbarAction::GoForward => self.go_forward(),
            ToolbarAction::GoUp => self.go_up(),
            ToolbarAction::NewFolder => self.dialog = DialogType::CreateFolder,
            ToolbarAction::NewFile => self.dialog = DialogType::CreateFile,
            ToolbarAction::ToggleHidden => {
                self.show_hidden = !self.show_hidden;
                self.load_entries();
            }
            ToolbarAction::SetViewMode(mode) => self.view_mode = mode,
            ToolbarAction::Search(query) => {
                self.search_query = query;
                self.apply_filter();
            }
            ToolbarAction::OpenTerminal => {
                self.terminal.toggle(&self.current_path);
            }
        }
    }

    /// Handle bulk action from toolbar.
    pub(super) fn handle_bulk_action(&mut self, action: BulkAction) {
        match action {
            BulkAction::CreateZip => {
                let selected = self
                    .get_selected_entries()
                    .into_iter()
                    .map(|entry| entry.path)
                    .collect();
                self.start_zip(selected);
            }
            BulkAction::Create7z => {
                let selected = self
                    .get_selected_entries()
                    .into_iter()
                    .map(|entry| entry.path)
                    .collect();
                self.start_7z(selected);
            }
            BulkAction::Copy => self.copy_to_clipboard(),
            BulkAction::Cut => self.cut_to_clipboard(),
            BulkAction::Delete => {
                let selected = self.get_selected_entries();
                let paths = selected.into_iter().map(|e| e.path).collect();
                self.dialog = DialogType::ConfirmDelete(paths);
            }
            BulkAction::SelectAll => self.select_all(),
            BulkAction::SelectNone => self.select_none(),
        }
    }

    fn start_zip(&mut self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            return;
        }
        let name = if paths.len() == 1 {
            format!(
                "{}.zip",
                paths[0].file_name().unwrap_or_default().to_string_lossy()
            )
        } else {
            "selection.zip".to_owned()
        };
        self.operation = Some(fyle::core::job::OperationJob::start(
            fyle::core::job::FileOperation::CreateZip(paths, self.current_path.join(name)),
        ));
    }

    fn start_7z(&mut self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            return;
        }
        let name = if paths.len() == 1 {
            format!(
                "{}.7z",
                paths[0].file_name().unwrap_or_default().to_string_lossy()
            )
        } else {
            "selection.7z".to_owned()
        };
        self.operation = Some(fyle::core::job::OperationJob::start(
            fyle::core::job::FileOperation::Create7z(paths, self.current_path.join(name)),
        ));
    }

    /// Handle sidebar actions.
    pub(super) fn handle_sidebar_action(&mut self, action: SidebarAction) {
        match action {
            SidebarAction::Navigate(path) => self.navigate_to(path),
            SidebarAction::StorageExplorer => self.storage.toggle(&self.current_path),
            SidebarAction::RemoveBookmark(index) => {
                if remove_bookmark(&mut self.bookmarks, index) {
                    if let Err(e) = save_bookmarks(&self.bookmarks) {
                        self.error_message = Some(format!("Failed to save bookmarks: {}", e));
                    }
                }
            }
            SidebarAction::AddBookmark(name) => {
                add_bookmark(&mut self.bookmarks, name, self.current_path.clone());
                if let Err(e) = save_bookmarks(&self.bookmarks) {
                    self.error_message = Some(format!("Failed to save bookmarks: {}", e));
                }
            }
        }
    }

    /// Handle actions from the file view (list or grid).
    pub(super) fn handle_view_action(&mut self, action: ViewAction) {
        match action {
            ViewAction::Navigate(path) => self.navigate_to(path),
            ViewAction::OpenFile(path) => self.open_file(path),
            ViewAction::ToggleSelection(path) => self.toggle_selection(&path),
            ViewAction::SelectSingle(path) => self.select_single(&path),
            ViewAction::ShowContextMenu(pos, entry) => self.context_menu.show(pos, entry),
            ViewAction::ChangeSort(sort_by) => self.toggle_sort(sort_by),
        }
    }

    /// Handle context menu actions.
    pub(super) fn handle_context_menu_action(&mut self, action: ContextMenuAction) {
        match action {
            ContextMenuAction::None => {}
            ContextMenuAction::OpenFile(path) => {
                self.open_file(path);
                self.context_menu.hide();
            }
            ContextMenuAction::Navigate(path) => {
                self.navigate_to(path);
                self.context_menu.hide();
            }
            ContextMenuAction::Copy => {
                if let Some(target) = &self.context_menu.target {
                    self.clipboard = vec![target.path.clone()];
                    self.clipboard_operation = Some(ClipboardOperation::Copy);
                }
                self.context_menu.hide();
            }
            ContextMenuAction::Cut => {
                if let Some(target) = &self.context_menu.target {
                    self.clipboard = vec![target.path.clone()];
                    self.clipboard_operation = Some(ClipboardOperation::Cut);
                }
                self.context_menu.hide();
            }
            ContextMenuAction::Rename => {
                if let Some(target) = &self.context_menu.target {
                    self.rename_state = Some(RenameState::new(&target.name));
                    self.dialog = DialogType::Rename(target.path.clone(), target.name.clone());
                }
                self.context_menu.hide();
            }
            ContextMenuAction::Delete => {
                if let Some(target) = &self.context_menu.target {
                    self.dialog = DialogType::ConfirmDelete(vec![target.path.clone()]);
                }
                self.context_menu.hide();
            }
            ContextMenuAction::Properties => {
                if let Some(ref target) = self.context_menu.target.clone() {
                    self.dialog = DialogType::FileProperties(target.clone());
                }
                self.context_menu.hide();
            }
            ContextMenuAction::AddBookmark => {
                if let Some(target) = &self.context_menu.target {
                    add_bookmark(
                        &mut self.bookmarks,
                        target.name.clone(),
                        target.path.clone(),
                    );
                    if let Err(e) = save_bookmarks(&self.bookmarks) {
                        self.error_message = Some(format!("Failed to save bookmarks: {}", e));
                    }
                }
                self.context_menu.hide();
            }
            ContextMenuAction::OpenTerminal => {
                let path = if let Some(target) = &self.context_menu.target {
                    if target.is_dir {
                        target.path.clone()
                    } else {
                        target
                            .path
                            .parent()
                            .unwrap_or(&self.current_path)
                            .to_path_buf()
                    }
                } else {
                    self.current_path.clone()
                };

                self.terminal.toggle(&path);
                self.context_menu.hide();
            }
            ContextMenuAction::CreateZip(path) => {
                self.start_zip(vec![path]);
                self.context_menu.hide();
            }
            ContextMenuAction::Create7z(path) => {
                self.start_7z(vec![path]);
                self.context_menu.hide();
            }
            ContextMenuAction::ExtractArchive(path) => {
                self.operation = Some(fyle::core::job::OperationJob::start(
                    fyle::core::job::FileOperation::ExtractArchive(path, self.current_path.clone()),
                ));
                self.context_menu.hide();
            }
            ContextMenuAction::Close => {
                self.context_menu.hide();
            }
        }
    }

    // Show active dialogs.
}
