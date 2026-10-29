//! Local discovery interactions: command palette, lenses and asynchronous peek.
use super::*;
use fyle::core::{discovery::fuzzy_score, preview::Preview};
use std::time::Duration;

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
                    close = crate::ui::buttons::compact(ui, "Close").clicked();
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
        let selected_path = self.preview_selection_path();
        #[cfg(target_os = "macos")]
        crate::native_preview::selection_changed(selected_path.as_deref());

        let path = if self.preview_open {
            selected_path
        } else {
            None
        };
        if path != self.preview_path {
            self.audio_preview.stop();
            self.preview_path = path.clone();
            self.preview_result = None;
            self.preview_texture = None;
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

    pub(super) fn preview_selection_path(&self) -> Option<std::path::PathBuf> {
        if self.storage.open {
            self.storage.selected_preview_path()
        } else if self.selected_entries.len() == 1 {
            self.entries
                .iter()
                .find(|entry| entry.selected && !entry.is_dir)
                .map(|entry| entry.path.clone())
        } else {
            None
        }
    }
    pub(super) fn render_preview(&mut self, ctx: &egui::Context) {
        if !self.preview_open {
            return;
        }
        if self.preview_texture.as_ref().map(|(path, _)| path) != self.preview_path.as_ref() {
            if let (
                Some(path),
                Some(Ok(Preview::Image {
                    rgba,
                    width,
                    height,
                })),
            ) = (&self.preview_path, &self.preview_result)
            {
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [*width as usize, *height as usize],
                    rgba,
                );
                self.preview_texture = Some((
                    path.clone(),
                    ctx.load_texture("quick-look-preview", image, egui::TextureOptions::LINEAR),
                ));
            }
        }

        let mut close = false;
        // Use a fresh egui area id so existing installations with the older,
        // larger Peek rectangle pick up this more compact default once.
        egui::Window::new("quick_look_preview_compact")
            .id(egui::Id::new("quick_look_preview_compact"))
            .title_bar(false)
            .collapsible(false)
            .resizable(true)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .default_size(egui::vec2(500.0, 360.0))
            .min_size(egui::vec2(320.0, 240.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("FYLE  /  QUICK LOOK")
                            .color(colors::CYBER_GREEN)
                            .monospace(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        close = crate::ui::buttons::compact(ui, "Close  ×").clicked();
                        if let Some(path) = &self.preview_path {
                            ui.label(
                                RichText::new(
                                    path.file_name().unwrap_or_default().to_string_lossy(),
                                )
                                .color(colors::MUTED)
                                .small(),
                            )
                            .on_hover_text(path.display().to_string());
                        }
                    });
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.small(
                        RichText::new("SPACE or Esc to close  ·  Local preview")
                            .color(colors::MUTED),
                    );
                    if matches!(&self.preview_result, Some(Ok(Preview::Text { .. }))) {
                        let response = ui.checkbox(&mut self.preview_wrap, "Wrap");
                        crate::ui::expose_control(
                            ui,
                            "preview_wrap",
                            "Wrap preview text",
                            &response,
                        );
                    }
                });
                ui.separator();
                let available = ui.available_size();
                if self.preview_loading {
                    ui.vertical_centered(|ui| {
                        ui.add_space(available.y * 0.35);
                        ui.spinner();
                        ui.label("Preparing preview…");
                    });
                }
                match &self.preview_result {
                    Some(Ok(Preview::Image { width, height, .. })) => {
                        if let Some((_, texture)) = &self.preview_texture {
                            let size = egui::vec2(*width as f32, *height as f32);
                            let scale = (available.x / size.x).min(available.y / size.y);
                            let display_size = size * scale.max(0.01);
                            ui.vertical_centered(|ui| {
                                ui.add_space(((available.y - display_size.y).max(0.0)) * 0.5);
                                ui.add(egui::Image::new(texture).fit_to_exact_size(display_size));
                            });
                        } else {
                            ui.spinner();
                        }
                    }
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
                                        .wrap(self.preview_wrap)
                                        .selectable(true),
                                );
                            });
                    }
                    Some(Ok(Preview::Audio)) => {
                        let status = self.audio_preview.status();
                        let path = self.preview_path.clone();
                        ui.vertical_centered(|ui| {
                            ui.add_space((available.y * 0.08).max(8.0));
                            ui.label(RichText::new("♫").size(38.0).color(colors::CYBER_GREEN));
                            if let Some(path) = &path {
                                ui.heading(path.file_name().unwrap_or_default().to_string_lossy());
                            }
                            ui.label(
                                RichText::new("LOCAL AUDIO PREVIEW")
                                    .small()
                                    .monospace()
                                    .color(colors::MUTED),
                            );
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                if crate::ui::buttons::accent(
                                    ui,
                                    if status.playing { "Pause" } else { "Play" },
                                    colors::CYBER_GREEN,
                                )
                                .clicked()
                                {
                                    if let Some(path) = &path {
                                        self.audio_preview.toggle(path.clone());
                                    }
                                }
                                if crate::ui::buttons::compact(ui, "Stop").clicked() {
                                    self.audio_preview.stop();
                                }
                            });
                            if let Some(duration) = status.duration {
                                let total = duration.as_secs_f32().max(0.01);
                                let mut position = status.position.as_secs_f32().clamp(0.0, total);
                                let seek = ui.add_sized(
                                    [ui.available_width().min(420.0), 20.0],
                                    egui::Slider::new(&mut position, 0.0..=total).show_value(false),
                                );
                                if seek.changed() {
                                    self.audio_preview.seek(Duration::from_secs_f32(position));
                                }
                                ui.label(format!(
                                    "{}  /  {}",
                                    format_audio_time(status.position),
                                    format_audio_time(duration)
                                ));
                            } else {
                                ui.label("Duration unavailable");
                            }
                            let mut volume = status.volume;
                            ui.horizontal(|ui| {
                                ui.label("Volume");
                                if ui
                                    .add_sized(
                                        [140.0, 20.0],
                                        egui::Slider::new(&mut volume, 0.0..=1.0).show_value(false),
                                    )
                                    .changed()
                                {
                                    self.audio_preview.set_volume(volume);
                                }
                            });
                            if let Some(error) = status.error {
                                ui.colored_label(colors::AMBER, error);
                            }
                        });
                    }
                    Some(Ok(Preview::Unsupported)) => {
                        ui.vertical_centered(|ui| {
                            ui.add_space(available.y * 0.3);
                            ui.heading("Preview unavailable");
                            ui.label(
                                "This file format is not supported by Fyle’s built-in preview.",
                            );
                        });
                    }
                    Some(Err(error)) => {
                        ui.colored_label(colors::AMBER, format!("{error:#}"));
                    }
                    None if !self.preview_loading => {
                        ui.label("Select one file to preview.");
                    }
                    None => {}
                }
            });
        if close {
            self.preview_open = false;
        }
    }
}

fn format_audio_time(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    format!("{:02}:{:02}", total_seconds / 60, total_seconds % 60)
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
