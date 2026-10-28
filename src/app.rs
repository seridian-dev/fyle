mod actions;
mod appearance;
mod automation;
mod discovery;
mod editor;
mod render;
mod review;
mod storage;
mod terminal;
mod workspace;
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
    add_bookmark, filter_entries, format_size, load_bookmarks, remove_bookmark, save_bookmarks,
    sort_entries,
};
use fyle::types::{
    Bookmark, ClipboardOperation, DialogType, FileEntry, SortBy, SortOrder, ViewMode,
};

use eframe::egui::{self, RichText};
use std::collections::HashSet;
use std::path::PathBuf;

pub(crate) const TERMINAL_FONT_FAMILY: &str = "fyle_terminal_nerd";
pub(crate) const SYSTEM_TERMINAL_FONT_FAMILY: &str = "fyle_terminal_system";

pub(crate) fn register_terminal_font(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        TERMINAL_FONT_FAMILY.to_owned(),
        egui::FontData::from_static(include_bytes!(
            "../assets/fonts/JetBrainsMonoNerdFont-Regular.ttf"
        )),
    );
    if let Some(system_font) = system_monospace_font() {
        fonts.font_data.insert(
            SYSTEM_TERMINAL_FONT_FAMILY.to_owned(),
            egui::FontData::from_owned(system_font),
        );
    }
    let mut family = fonts
        .families
        .get(&egui::FontFamily::Monospace)
        .cloned()
        .unwrap_or_default();
    family.insert(0, TERMINAL_FONT_FAMILY.to_owned());
    fonts.families.insert(
        egui::FontFamily::Name(TERMINAL_FONT_FAMILY.into()),
        family.clone(),
    );
    let mut system_family = family;
    system_family.retain(|font| font != TERMINAL_FONT_FAMILY);
    if fonts.font_data.contains_key(SYSTEM_TERMINAL_FONT_FAMILY) {
        system_family.insert(0, SYSTEM_TERMINAL_FONT_FAMILY.to_owned());
    }
    system_family.push(TERMINAL_FONT_FAMILY.to_owned());
    fonts.families.insert(
        egui::FontFamily::Name(SYSTEM_TERMINAL_FONT_FAMILY.into()),
        system_family,
    );
    ctx.set_fonts(fonts);
}

fn system_monospace_font() -> Option<Vec<u8>> {
    #[cfg(target_os = "macos")]
    let candidates = [
        "/System/Library/Fonts/SFNSMono.ttf",
        "/System/Library/Fonts/Supplemental/Andale Mono.ttf",
    ];
    #[cfg(target_os = "linux")]
    let candidates = [
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        "/usr/share/fonts/truetype/liberation2/LiberationMono-Regular.ttf",
        "/usr/share/fonts/truetype/noto/NotoSansMono-Regular.ttf",
    ];
    #[cfg(target_os = "windows")]
    let candidates = ["C:\\Windows\\Fonts\\consola.ttf"];
    candidates.iter().find_map(|path| std::fs::read(path).ok())
}

pub(crate) struct FileManager {
    automation: Option<automation::Automation>,
    appearance: fyle::appearance::Appearance,
    appearance_store: fyle::appearance::SettingsStore,
    appearance_open: bool,
    preferences_section: PreferencesSection,
    appearance_error: Option<String>,
    shader: Option<std::sync::Arc<crate::theme::shader::HudShader>>,
    shader_error: Option<String>,
    current_path: PathBuf,
    tabs: Vec<workspace::Tab>,
    active_tab: usize,
    location_open: bool,
    location_input: String,
    location_focus: bool,
    location_error: Option<String>,
    entries: Vec<FileEntry>,
    cached_entries: Vec<FileEntry>,
    loader: fyle::core::loader::DirectoryLoader,
    loading: bool,
    operation: Option<fyle::core::job::OperationJob>,
    storage_cache: crate::ui::sidebar::StorageCache,
    view_mode: ViewMode,
    sort_by: SortBy,
    sort_order: SortOrder,
    show_hidden: bool,
    search_query: String,
    query_error: Option<String>,
    palette: discovery::Palette,
    editor: editor::Editor,
    review: review::Review,
    storage: storage::StorageExplorer,
    terminal: terminal::TerminalPanel,
    preview_open: bool,
    preview_wrap: bool,
    preview_path: Option<PathBuf>,
    preview_loader: fyle::core::preview::PreviewLoader,
    preview_loading: bool,
    preview_result: Option<anyhow::Result<fyle::core::preview::Preview>>,
    selected_entries: HashSet<PathBuf>,
    bookmarks: Vec<Bookmark>,
    dialog: DialogType,
    error_message: Option<String>,
    clipboard: Vec<PathBuf>,
    clipboard_operation: Option<ClipboardOperation>,
    context_menu: ContextMenuState,
    toolbar_more_open: bool,
    rename_state: Option<RenameState>,
    create_folder_state: CreateFolderState,
    create_file_state: CreateFileState,
    history: Vec<PathBuf>,
    forward_history: Vec<PathBuf>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum PreferencesSection {
    #[default]
    General,
    Appearance,
    Terminal,
}

impl FileManager {
    pub(crate) fn new_with_target(target: fyle::startup::LaunchTarget) -> Self {
        let mut app = Self::new(target.directory);
        if let Some(file) = target.file {
            app.open_file(file);
        }
        app
    }

    pub(crate) fn new(initial_path: PathBuf) -> Self {
        let current_path = if initial_path.is_dir() {
            initial_path
        } else {
            dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
        };

        let appearance_store = fyle::appearance::SettingsStore::default();
        let (appearance, appearance_error) = match appearance_store.load() {
            Ok(settings) => (settings, None),
            Err(error) => (Default::default(), Some(error.to_string())),
        };
        let mut app = Self {
            automation: None,
            appearance,
            appearance_store,
            appearance_open: false,
            preferences_section: PreferencesSection::General,
            appearance_error,
            shader: None,
            shader_error: None,
            current_path: current_path.clone(),
            tabs: Vec::new(),
            active_tab: 0,
            location_open: false,
            location_input: String::new(),
            location_focus: false,
            location_error: None,
            entries: Vec::new(),
            cached_entries: Vec::new(),
            loader: Default::default(),
            loading: false,
            operation: None,
            storage_cache: Default::default(),
            view_mode: ViewMode::List,
            sort_by: SortBy::Name,
            sort_order: SortOrder::Ascending,
            show_hidden: true,
            search_query: String::new(),
            query_error: None,
            palette: Default::default(),
            editor: editor::Editor::default(),
            review: review::Review::default(),
            storage: storage::StorageExplorer::default(),
            terminal: terminal::TerminalPanel::default(),
            preview_open: false,
            preview_wrap: true,
            preview_path: None,
            preview_loader: Default::default(),
            preview_loading: false,
            preview_result: None,
            selected_entries: HashSet::new(),
            bookmarks: load_bookmarks(),
            dialog: DialogType::None,
            error_message: None,
            clipboard: Vec::new(),
            clipboard_operation: None,
            context_menu: ContextMenuState::default(),
            toolbar_more_open: false,
            rename_state: None,
            create_folder_state: CreateFolderState::default(),
            create_file_state: CreateFileState::default(),
            history: Vec::new(),
            forward_history: Vec::new(),
        };
        app.tabs.push(app.snapshot_tab());
        app.load_entries();
        app
    }

    /// Reload entries from the current directory.
    fn load_entries(&mut self) {
        self.selected_entries.clear();
        self.preview_loader.clear();
        self.preview_path = None;
        self.preview_result = None;
        self.preview_loading = false;

        self.entries.clear();
        self.cached_entries.clear();
        self.loading = true;
        self.loader
            .request(self.current_path.clone(), self.show_hidden);
    }

    fn poll_directory(&mut self) {
        if let Some(result) = self.loader.poll() {
            self.loading = false;
            match result {
                Ok(entries) => {
                    self.cached_entries = entries;
                    self.apply_filter();
                }
                Err(e) => {
                    log::error!("Directory scan failed: {e:#}");
                    self.error_message = Some(format!("Failed to load directory: {e:#}"));
                }
            }
        }
    }

    fn poll_operation(&mut self) {
        if let Some(job) = &self.operation {
            if let Some(result) = job.poll() {
                if result.is_ok() && job.clears_cut_clipboard {
                    self.clipboard.clear();
                    self.clipboard_operation = None;
                }
                if let Err(error) = result {
                    log::error!("File operation failed: {error:#}");
                    self.error_message = Some(format!("File operation failed: {error:#}"));
                }
                self.operation = None;
                if self.storage.open {
                    self.storage.refresh();
                } else {
                    self.load_entries();
                }
            }
        }
    }

    fn apply_filter(&mut self) {
        self.query_error = fyle::core::discovery::Query::parse(&self.search_query)
            .err()
            .map(|error| error.to_string());
        self.entries = self.cached_entries.clone();
        filter_entries(&mut self.entries, &self.search_query);
        for entry in &mut self.entries {
            entry.selected = self.selected_entries.contains(&entry.path);
        }
        sort_entries(&mut self.entries, &self.sort_by, &self.sort_order);
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
            self.navigate_to(parent.to_path_buf());
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_uses_cached_entries_and_preserves_selection() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Notes.txt");
        std::fs::write(&path, "test").unwrap();
        let mut app = FileManager::new(dir.path().into());
        app.cached_entries =
            fyle::core::load_directory_entries(dir.path(), true, &HashSet::new()).unwrap();
        std::fs::remove_file(&path).unwrap();
        app.selected_entries.insert(path.clone());
        app.search_query = "NOTES".into();
        app.apply_filter();
        assert_eq!(app.entries.len(), 1);
        assert!(app.entries[0].selected);
        app.search_query = "missing".into();
        app.apply_filter();
        assert!(app.entries.is_empty());
        app.search_query.clear();
        app.apply_filter();
        assert_eq!(app.entries[0].path, path);
    }

    #[test]
    fn opening_source_files_stays_inside_fyle() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("main.rs");
        std::fs::write(&path, "fn main() {}\n").unwrap();
        let mut app = FileManager::new(dir.path().into());
        app.open_file(path);
        assert!(app.editor.is_open());
        assert!(app.error_message.is_none());
    }
}

#[cfg(test)]
mod workspace_regression_tests {
    use super::*;
    #[test]
    fn tabs_keep_independent_folder_filter_and_navigation_history() {
        let dir = tempfile::tempdir().unwrap();
        let child = dir.path().join("child");
        std::fs::create_dir(&child).unwrap();
        let mut app = FileManager::new(dir.path().into());
        app.search_query = "ext:rs".into();
        app.new_tab();
        app.navigate_to(child.clone());
        app.search_query = "ext:py".into();
        app.switch_tab(0);
        assert_eq!(app.current_path, dir.path());
        assert_eq!(app.search_query, "ext:rs");
        app.switch_tab(1);
        assert_eq!(app.current_path, child);
        assert_eq!(app.search_query, "ext:py");
        app.go_back();
        assert_eq!(app.current_path, dir.path());
        app.close_tab(1);
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.search_query, "ext:rs");
        app.close_tab(0);
        assert_eq!(app.tabs.len(), 1);
    }
}
