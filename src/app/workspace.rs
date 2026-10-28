//! Session-local tabs. Inactive tabs store navigation policy, not worker threads.
use super::*;
#[derive(Clone)]
pub(super) struct Tab {
    pub path: PathBuf,
    pub query: String,
    view: ViewMode,
    sort: SortBy,
    order: SortOrder,
    hidden: bool,
    history: Vec<PathBuf>,
    forward: Vec<PathBuf>,
}
impl FileManager {
    /// Consume workspace Tab chords before egui starts its focus traversal.
    pub(super) fn workspace_input(&mut self, input: &mut egui::RawInput) {
        if self.operation.is_some()
            || self.dialog != DialogType::None
            || self.appearance_open
            || self.palette.open
            || self.location_open
        {
            return;
        }
        let direction = input.events.iter().find_map(|event| match event {
            egui::Event::Key {
                key: egui::Key::Tab,
                pressed: true,
                modifiers,
                ..
            } if modifiers.ctrl => Some(if modifiers.shift { -1_i32 } else { 1 }),
            _ => None,
        });
        if let Some(direction) = direction {
            let index = (self.active_tab as i32 + self.tabs.len() as i32 + direction) as usize
                % self.tabs.len();
            self.switch_tab(index);
            input.events.retain(|event| !matches!(event,egui::Event::Key{key:egui::Key::Tab,modifiers,..} if modifiers.ctrl));
        }
    }

    pub(super) fn snapshot_tab(&self) -> Tab {
        Tab {
            path: self.current_path.clone(),
            query: self.search_query.clone(),
            view: self.view_mode.clone(),
            sort: self.sort_by.clone(),
            order: self.sort_order.clone(),
            hidden: self.show_hidden,
            history: self.history.clone(),
            forward: self.forward_history.clone(),
        }
    }
    pub(super) fn new_tab(&mut self) {
        if self.tabs.len() >= 24 || self.operation.is_some() || self.dialog != DialogType::None {
            return;
        }
        self.tabs[self.active_tab] = self.snapshot_tab();
        self.storage.open = false;
        self.tabs.push(self.snapshot_tab());
        self.active_tab = self.tabs.len() - 1;
        self.load_entries();
    }
    pub(super) fn switch_tab(&mut self, index: usize) {
        if index >= self.tabs.len()
            || index == self.active_tab
            || self.operation.is_some()
            || self.dialog != DialogType::None
        {
            return;
        }
        self.storage.open = false;
        self.tabs[self.active_tab] = self.snapshot_tab();
        self.restore_tab(index);
    }
    fn restore_tab(&mut self, index: usize) {
        self.active_tab = index;
        let tab = self.tabs[index].clone();
        self.current_path = tab.path;
        self.search_query = tab.query;
        self.view_mode = tab.view;
        self.sort_by = tab.sort;
        self.sort_order = tab.order;
        self.show_hidden = tab.hidden;
        self.history = tab.history;
        self.forward_history = tab.forward;
        self.query_error = None;
        self.error_message = None;
        self.context_menu.hide();
        self.load_entries();
    }
    pub(super) fn close_tab(&mut self, index: usize) {
        if self.tabs.len() <= 1
            || index >= self.tabs.len()
            || self.operation.is_some()
            || self.dialog != DialogType::None
        {
            return;
        }
        self.tabs[self.active_tab] = self.snapshot_tab();
        self.tabs.remove(index);
        if index == self.active_tab {
            self.restore_tab(index.min(self.tabs.len() - 1));
        } else if index < self.active_tab {
            self.active_tab -= 1;
        }
    }
    pub(super) fn render_tabs(&mut self, ui: &mut egui::Ui) {
        let mut switch = None;
        let mut close = None;
        let mut new = false;
        let mut open_storage = false;
        egui::ScrollArea::horizontal()
            .id_source("workspace_tabs")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for (index, tab) in self.tabs.iter().enumerate() {
                        let path = if index == self.active_tab {
                            &self.current_path
                        } else {
                            &tab.path
                        };
                        let label = path
                            .file_name()
                            .filter(|name| !name.is_empty())
                            .unwrap_or(path.as_os_str())
                            .to_string_lossy();
                        ui.push_id(index, |ui| {
                            let response = ui
                                .selectable_label(
                                    index == self.active_tab,
                                    crate::ui::truncate_name(&label, 22),
                                )
                                .on_hover_text(path.display().to_string());
                            crate::ui::expose_control(
                                ui,
                                &format!("tab:{index}"),
                                &label,
                                &response,
                            );
                            if response.clicked() {
                                switch = Some(index);
                            }
                            if self.tabs.len() > 1 {
                                let response =
                                    ui.small_button("×").on_hover_text("Close tab · Ctrl+W");
                                crate::ui::expose_control(
                                    ui,
                                    &format!("tab_close:{index}"),
                                    "Close tab",
                                    &response,
                                );
                                if response.clicked() {
                                    close = Some(index);
                                }
                            }
                        });
                    }
                    let response = ui
                        .selectable_label(self.storage.open, "Storage")
                        .on_hover_text("Analyze folder sizes");
                    crate::ui::expose_control(ui, "storage_tab", "Storage Explorer", &response);
                    open_storage = response.clicked();
                    let response = ui.button("+").on_hover_text("New tab · Ctrl+T");
                    crate::ui::expose_control(ui, "new_tab", "New tab", &response);
                    new = response.clicked();
                });
            });
        if let Some(index) = close {
            self.storage.open = false;
            self.close_tab(index);
        } else if let Some(index) = switch {
            self.storage.open = false;
            self.switch_tab(index);
        } else if new {
            self.storage.open = false;
            self.new_tab();
        } else if open_storage {
            self.storage.open = true;
            self.storage.scan(self.current_path.clone());
        }
    }
    pub(super) fn open_location(&mut self) {
        self.location_open = true;
        self.location_input = self.current_path.display().to_string();
        self.location_focus = true;
        self.location_error = None;
    }
    fn go_to_location(&mut self) {
        let input = self.location_input.trim();
        let path = if input == "~" {
            dirs::home_dir().unwrap_or_default()
        } else if let Some(rest) = input.strip_prefix("~/") {
            dirs::home_dir().unwrap_or_default().join(rest)
        } else {
            let path = PathBuf::from(input);
            if path.is_absolute() || input.starts_with("file:") {
                path
            } else {
                self.current_path.join(path)
            }
        };
        match fyle::startup::resolve_location(path.as_os_str()) {
            Ok(path) => {
                self.location_open = false;
                self.navigate_to(path);
            }
            Err(error) => self.location_error = Some(format!("{error:#}")),
        }
    }
    pub(super) fn render_location(&mut self, ctx: &egui::Context) {
        if !self.location_open {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.location_open = false;
            return;
        }
        let mut go = false;
        let mut cancel = false;
        egui::Window::new("Go to location")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .default_width(440.0)
            .show(ctx, |ui| {
                ui.label("A folder, local file URL, or path relative to this tab");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.location_input)
                        .desired_width(f32::INFINITY),
                );
                if self.location_focus {
                    response.request_focus();
                    self.location_focus = false;
                }
                crate::ui::expose_control(ui, "location_input", "Location", &response);
                if let Some(error) = &self.location_error {
                    ui.colored_label(colors::AMBER, error);
                }
                ui.horizontal(|ui| {
                    go = ui.button("Go").clicked();
                    cancel = ui.button("Cancel").clicked();
                });
                go |= ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
            });
        if go {
            self.go_to_location();
        } else if cancel {
            self.location_open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn workspace_shortcuts_do_not_reach_egui_focus_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = FileManager::new(dir.path().into());
        app.new_tab();
        let mut input = egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::Tab,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
            }],
            ..Default::default()
        };
        app.workspace_input(&mut input);
        assert_eq!(app.active_tab, 0);
        assert!(input.events.is_empty());
    }
}
