//! Local discovery interactions: command palette, lenses and asynchronous peek.
use super::*;
use fyle::core::{discovery::fuzzy_score, preview::Preview};

#[derive(Clone)]
enum Target {
    Folder(PathBuf),
    File(PathBuf),
    Command(ToolbarAction),
}
#[derive(Clone)]
struct Candidate {
    label: String,
    detail: String,
    target: Target,
}
#[derive(Default)]
pub(super) struct Palette {
    pub open: bool,
    pub query: String,
    candidates: Vec<Candidate>,
    matches: Vec<usize>,
    selected: usize,
    focus: bool,
}
impl Palette {
    fn rank(&mut self) {
        let commands_only = self.query.starts_with('>');
        let query = self.query.trim_start_matches('>').trim();
        let mut ranked: Vec<_> = self
            .candidates
            .iter()
            .enumerate()
            .filter(|(_, item)| !commands_only || matches!(item.target, Target::Command(_)))
            .filter_map(|(index, item)| fuzzy_score(query, &item.label).map(|score| (index, score)))
            .collect();
        ranked.sort_by_key(|(index, score)| (std::cmp::Reverse(*score), *index));
        self.matches = ranked
            .into_iter()
            .take(10)
            .map(|(index, _)| index)
            .collect();
        self.selected = 0;
    }
}
impl FileManager {
    pub(super) fn open_palette(&mut self) {
        let mut candidates = Vec::new();
        for (label, action) in [
            ("Refresh folder", ToolbarAction::Refresh),
            ("New folder", ToolbarAction::NewFolder),
            ("New file", ToolbarAction::NewFile),
            ("Toggle hidden files", ToolbarAction::ToggleHidden),
            ("Grid view", ToolbarAction::SetViewMode(ViewMode::Grid)),
            ("List view", ToolbarAction::SetViewMode(ViewMode::List)),
            ("Toggle local peek", ToolbarAction::TogglePreview),
            ("Open terminal here", ToolbarAction::OpenTerminal),
        ] {
            candidates.push(Candidate {
                label: label.into(),
                detail: "Command".into(),
                target: Target::Command(action),
            });
        }
        let mut seen = HashSet::new();
        for (path, label, detail) in self
            .bookmarks
            .iter()
            .map(|b| (b.path.clone(), b.name.clone(), "Bookmark"))
            .chain(self.history.iter().rev().take(12).map(|p| {
                (
                    p.clone(),
                    p.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    "Recent folder",
                )
            }))
        {
            if seen.insert(path.clone()) {
                candidates.push(Candidate {
                    label,
                    detail: detail.into(),
                    target: Target::Folder(path),
                });
            }
        }
        for entry in &self.cached_entries {
            candidates.push(Candidate {
                label: entry.name.clone(),
                detail: if entry.is_dir {
                    "Folder"
                } else {
                    "File · select and peek"
                }
                .into(),
                target: if entry.is_dir {
                    Target::Folder(entry.path.clone())
                } else {
                    Target::File(entry.path.clone())
                },
            });
        }
        self.palette = Palette {
            open: true,
            candidates,
            focus: true,
            ..Default::default()
        };
        self.palette.rank();
    }
    fn activate_candidate(&mut self, target: Target) {
        self.palette.open = false;
        match target {
            Target::Folder(path) => self.navigate_to(path),
            Target::File(path) => {
                self.search_query.clear();
                self.apply_filter();
                self.select_single(&path);
                self.preview_open = true;
            }
            Target::Command(action) => self.handle_toolbar_action(action),
        }
    }
    pub(super) fn render_palette(&mut self, ctx: &egui::Context) {
        if !self.palette.open {
            return;
        }
        let escape = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        if escape {
            self.palette.open = false;
            return;
        }
        let mut target = None;
        let mut close = false;
        egui::Window::new("Jump · Ctrl+P")
            .id(egui::Id::new("jump_palette"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 110.0])
            .default_width((ctx.screen_rect().width() - 48.0).min(560.0))
            .show(ctx, |ui| {
                ui.label("Find here, jump to a bookmark, or type > for commands");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.palette.query)
                        .desired_width(f32::INFINITY)
                        .hint_text("Fuzzy name or > command"),
                );
                if self.palette.focus {
                    response.request_focus();
                    self.palette.focus = false;
                }
                crate::ui::expose_control(ui, "palette_query", "Jump query", &response);
                if response.changed() {
                    self.palette.rank();
                }
                if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown))
                    && !self.palette.matches.is_empty()
                {
                    self.palette.selected =
                        (self.palette.selected + 1) % self.palette.matches.len();
                }
                if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp))
                    && !self.palette.matches.is_empty()
                {
                    self.palette.selected = (self.palette.selected + self.palette.matches.len()
                        - 1)
                        % self.palette.matches.len();
                }
                let enter =
                    ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                ui.separator();
                if self.palette.matches.is_empty() {
                    ui.label("No matches in this folder. Try a shorter name or > command.");
                }
                for (row, &index) in self.palette.matches.iter().enumerate() {
                    let candidate = &self.palette.candidates[index];
                    let response = ui.selectable_label(
                        self.palette.selected == row,
                        format!("{}    {}", candidate.label, candidate.detail),
                    );
                    crate::ui::expose_control(
                        ui,
                        &format!("palette_result:{row}"),
                        &candidate.label,
                        &response,
                    );
                    if response.clicked() || (enter && row == self.palette.selected) {
                        target = Some(candidate.target.clone());
                    }
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.small("Up/Down choose · Enter jump · Esc close");
                    close = ui.small_button("Close").clicked();
                });
            });
        if close {
            self.palette.open = false;
        }
        if let Some(target) = target {
            self.activate_candidate(target);
        }
    }
    pub(super) fn render_lenses(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.small("Lenses");
            for (label, query) in [("All", ""), ("Folders", "kind:folder"), ("Code", "ext:rs,py,js,ts,tsx,jsx,go,c,cpp,h,sh"), ("Big files", "size:>100mb"), ("Empty files", "kind:file size:0")] {
                let response = ui.selectable_label(self.search_query == query, label);
                crate::ui::expose_control(ui, &format!("lens:{}", label.to_lowercase().replace(' ', "_")), label, &response);
                if response.clicked() { self.search_query = query.into(); self.apply_filter(); }
            }
            ui.label("?").on_hover_text("Combine name terms, ext:rs, kind:folder, size:>10mb and -exclude. Quote a phrase. Units use powers of 1024. Current folder only.");
        });
        if let Some(error) = &self.query_error {
            ui.colored_label(colors::AMBER, error);
        }
    }
    pub(super) fn poll_preview(&mut self) {
        let path = if self.preview_open && self.selected_entries.len() == 1 {
            self.entries
                .iter()
                .find(|entry| entry.selected && !entry.is_dir)
                .map(|entry| entry.path.clone())
        } else {
            None
        };
        if path != self.preview_path {
            self.preview_path = path.clone();
            self.preview_result = None;
            self.preview_loading = path.is_some();
            if let Some(path) = path {
                self.preview_loader.request(path);
            } else {
                self.preview_loader.clear();
            }
        }
        if let Some(result) = self.preview_loader.poll() {
            self.preview_loading = false;
            self.preview_result = Some(result);
        }
    }
    pub(super) fn render_preview(&mut self, ctx: &egui::Context) {
        if !self.preview_open {
            return;
        }
        egui::SidePanel::right("peek_panel")
            .default_width(320.0)
            .min_width(180.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Local peek");
                    if ui.small_button("Close").clicked() {
                        self.preview_open = false;
                    }
                });
                ui.small("Read only · plain text · 32 KiB limit");
                ui.separator();
                if let Some(path) = &self.preview_path {
                    ui.label(path.file_name().unwrap_or_default().to_string_lossy())
                        .on_hover_text(path.display().to_string());
                }
                if self.preview_loading {
                    ui.spinner();
                }
                match &self.preview_result {
                    Some(Ok(Preview::Text { text, truncated })) => {
                        if *truncated {
                            ui.colored_label(colors::AMBER, "Showing the first 32 KiB");
                        }
                        if text.is_empty() {
                            ui.label("Empty text file");
                        }
                        egui::ScrollArea::both()
                            .id_source(("peek_scroll", &self.preview_path))
                            .show(ui, |ui| {
                                ui.add(
                                    egui::Label::new(RichText::new(text).monospace())
                                        .wrap(false)
                                        .selectable(true),
                                );
                            });
                    }
                    Some(Ok(Preview::Unsupported)) => {
                        ui.label(
                            "Binary or unsupported encoding. Use your preferred app to open it.",
                        );
                    }
                    Some(Err(error)) => {
                        ui.colored_label(colors::AMBER, format!("{error:#}"));
                    }
                    None if !self.preview_loading => {
                        ui.label("Select one text or source file to peek inside.");
                    }
                    None => {}
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn jumping_to_a_file_clears_lenses_selects_it_and_opens_peek() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.txt");
        std::fs::write(&path, "local fixture").unwrap();
        let mut app = FileManager::new(dir.path().into());
        app.cached_entries =
            fyle::core::load_directory_entries(dir.path(), true, &HashSet::new()).unwrap();
        app.search_query = "ext:rs".into();
        app.apply_filter();
        assert!(app.entries.is_empty());
        app.activate_candidate(Target::File(path.clone()));
        assert!(app.preview_open);
        assert!(app.search_query.is_empty());
        assert!(app.selected_entries.contains(&path));
        app.poll_preview();
        assert_eq!(app.preview_path, Some(path));
        app.select_none();
        app.poll_preview();
        assert!(!app.preview_loading);
        assert!(app.preview_path.is_none());
    }
    #[test]
    fn parent_navigation_is_part_of_back_and_forward_history() {
        let dir = tempfile::tempdir().unwrap();
        let child = dir.path().join("child");
        std::fs::create_dir(&child).unwrap();
        let mut app = FileManager::new(child.clone());
        app.go_up();
        assert_eq!(app.current_path, dir.path());
        app.go_back();
        assert_eq!(app.current_path, child);
        app.go_forward();
        assert_eq!(app.current_path, dir.path());
    }
}
