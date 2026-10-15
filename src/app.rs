mod actions;
mod render;
use crate::theme::colors;
use crate::ui::{
    context_menu::{show_context_menu, ContextMenuAction, ContextMenuState},
    dialogs::{
        show_copy_dialog, show_create_file_dialog, show_create_folder_dialog, show_delete_dialog,
        show_error_dialog, show_move_dialog, show_properties_dialog, show_rename_dialog,
        CreateFileState, CreateFolderState, DialogResult, RenameState,
    },
    grid_view::render_grid_view,
    list_view::render_list_view,
    sidebar::{render_sidebar, SidebarAction},
    toolbar::{render_bulk_operations, render_toolbar, BulkAction, ToolbarAction},
    ViewAction,
};
use fyle::core::{
    add_bookmark, filter_entries, format_size, load_bookmarks, load_directory_entries,
    open_terminal, remove_bookmark, save_bookmarks, sort_entries,
};
use fyle::types::{
    Bookmark, ClipboardOperation, DialogType, FileEntry, SortBy, SortOrder, ViewMode,
};

use eframe::egui::{self, RichText};
use std::collections::HashSet;
use std::path::PathBuf;

pub(crate) struct FileManager {
    current_path: PathBuf,
    entries: Vec<FileEntry>,
    view_mode: ViewMode,
    sort_by: SortBy,
    sort_order: SortOrder,
    show_hidden: bool,
    search_query: String,
    selected_entries: HashSet<PathBuf>,
    bookmarks: Vec<Bookmark>,
    dialog: DialogType,
    error_message: Option<String>,
    clipboard: Vec<PathBuf>,
    clipboard_operation: Option<ClipboardOperation>,
    context_menu: ContextMenuState,
    rename_state: Option<RenameState>,
    create_folder_state: CreateFolderState,
    create_file_state: CreateFileState,
    history: Vec<PathBuf>,
    forward_history: Vec<PathBuf>,
}

impl FileManager {
    pub(crate) fn new(initial_path: PathBuf) -> Self {
        let current_path = if initial_path.is_dir() {
            initial_path
        } else {
            dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
        };

        let mut app = Self {
            current_path: current_path.clone(),
            entries: Vec::new(),
            view_mode: ViewMode::List,
            sort_by: SortBy::Name,
            sort_order: SortOrder::Ascending,
            show_hidden: true,
            search_query: String::new(),
            selected_entries: HashSet::new(),
            bookmarks: load_bookmarks(),
            dialog: DialogType::None,
            error_message: None,
            clipboard: Vec::new(),
            clipboard_operation: None,
            context_menu: ContextMenuState::default(),
            rename_state: None,
            create_folder_state: CreateFolderState::default(),
            create_file_state: CreateFileState::default(),
            history: Vec::new(),
            forward_history: Vec::new(),
        };
        app.load_entries();
        app
    }

    /// Reload entries from the current directory.
    fn load_entries(&mut self) {
        self.selected_entries.clear();

        match load_directory_entries(&self.current_path, self.show_hidden, &self.selected_entries) {
            Ok(mut entries) => {
                sort_entries(&mut entries, &self.sort_by, &self.sort_order);
                filter_entries(&mut entries, &self.search_query);
                self.entries = entries;
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to load directory: {}", e));
                self.entries.clear();
            }
        }
    }

    /// Navigate to a new directory.
    fn navigate_to(&mut self, path: PathBuf) {
        if path == self.current_path {
            return;
        }

        self.history.push(self.current_path.clone());
        self.forward_history.clear();
        self.current_path = path;
        self.search_query.clear();
        self.load_entries();
    }

    /// Go back in history.
    fn go_back(&mut self) {
        if let Some(path) = self.history.pop() {
            self.forward_history.push(self.current_path.clone());
            self.current_path = path;
            self.search_query.clear();
            self.load_entries();
        }
    }

    /// Go forward in history.
    fn go_forward(&mut self) {
        if let Some(path) = self.forward_history.pop() {
            self.history.push(self.current_path.clone());
            self.current_path = path;
            self.search_query.clear();
            self.load_entries();
        }
    }

    /// Navigate to parent directory.
    fn go_up(&mut self) {
        if let Some(parent) = self.current_path.parent() {
            self.current_path = parent.to_path_buf();
            self.load_entries();
        }
    }

    /// Select all entries.
    fn select_all(&mut self) {
        for entry in &mut self.entries {
            entry.selected = true;
            self.selected_entries.insert(entry.path.clone());
        }
    }

    /// Deselect all entries.
    fn select_none(&mut self) {
        for entry in &mut self.entries {
            entry.selected = false;
        }
        self.selected_entries.clear();
    }

    /// Toggle selection of an entry.
    fn toggle_selection(&mut self, path: &PathBuf) {
        if let Some(entry) = self.entries.iter_mut().find(|e| &e.path == path) {
            entry.selected = !entry.selected;
            if entry.selected {
                self.selected_entries.insert(path.clone());
            } else {
                self.selected_entries.remove(path);
            }
        }
    }

    /// Select only a single entry.
    fn select_single(&mut self, path: &PathBuf) {
        self.select_none();
        self.toggle_selection(path);
    }

    /// Get all selected entries.
    fn get_selected_entries(&self) -> Vec<FileEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.selected)
            .cloned()
            .collect()
    }

    /// Copy selected entries to clipboard.
    fn copy_to_clipboard(&mut self) {
        let selected = self.get_selected_entries();
        if !selected.is_empty() {
            self.clipboard = selected.into_iter().map(|e| e.path).collect();
            self.clipboard_operation = Some(ClipboardOperation::Copy);
        }
    }

    /// Cut selected entries to clipboard.
    fn cut_to_clipboard(&mut self) {
        let selected = self.get_selected_entries();
        if !selected.is_empty() {
            self.clipboard = selected.into_iter().map(|e| e.path).collect();
            self.clipboard_operation = Some(ClipboardOperation::Cut);
        }
    }

    /// Initiate paste operation.
    fn paste(&mut self) {
        if self.clipboard.is_empty() || self.clipboard_operation.is_none() {
            return;
        }

        match self.clipboard_operation.as_ref().unwrap() {
            ClipboardOperation::Copy => {
                self.dialog =
                    DialogType::ConfirmCopy(self.clipboard.clone(), self.current_path.clone());
            }
            ClipboardOperation::Cut => {
                self.dialog =
                    DialogType::ConfirmMove(self.clipboard.clone(), self.current_path.clone());
            }
        }
    }

    /// Toggle sort by the given criteria.
    fn toggle_sort(&mut self, new_sort_by: SortBy) {
        if self.sort_by == new_sort_by {
            self.sort_order = self.sort_order.toggle();
        } else {
            self.sort_by = new_sort_by;
            self.sort_order = SortOrder::Ascending;
        }
        sort_entries(&mut self.entries, &self.sort_by, &self.sort_order);
    }

    // Handle keyboard shortcuts.
}
