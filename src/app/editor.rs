//! Fyle's native text and source editor.
use eframe::egui::{self, text::LayoutJob, Color32, RichText, TextFormat};
use std::{collections::HashSet, path::PathBuf, sync::OnceLock};
use syntect::{
    easy::HighlightLines,
    highlighting::{Theme, ThemeSet},
    parsing::{SyntaxReference, SyntaxSet},
};

struct SyntaxAssets {
    syntaxes: SyntaxSet,
    themes: ThemeSet,
}
static SYNTAX: OnceLock<SyntaxAssets> = OnceLock::new();
const MAX_HIGHLIGHT_BYTES: usize = 256 * 1024;

fn syntax_assets() -> &'static SyntaxAssets {
    SYNTAX.get_or_init(|| SyntaxAssets {
        syntaxes: SyntaxSet::load_defaults_newlines(),
        themes: ThemeSet::load_defaults(),
    })
}

fn selected_syntax<'a>(assets: &'a SyntaxAssets, path: &std::path::Path) -> &'a SyntaxReference {
    assets
        .syntaxes
        .find_syntax_for_file(path)
        .ok()
        .flatten()
        .unwrap_or_else(|| assets.syntaxes.find_syntax_plain_text())
}

#[derive(Default)]
pub(super) struct Editor {
    documents: Vec<fyle::core::editor::Document>,
    worker: fyle::core::editor::EditorWorker,
    pending_loads: HashSet<PathBuf>,
    pending_saves: HashSet<PathBuf>,
    active: usize,
    find: String,
    match_index: usize,
    pending_match: Option<(egui::Id, usize, usize)>,
    highlight_cache: Option<(PathBuf, String, LayoutJob)>,
    error: Option<String>,
    discard_confirmation: bool,
    visible: bool,
}

impl Editor {
    pub(super) fn open(&mut self, path: PathBuf) -> anyhow::Result<()> {
        if let Some(index) = self
            .documents
            .iter()
            .position(|document| document.path() == path)
        {
            self.active = index;
        } else if self.pending_loads.contains(&path) {
            self.visible = true;
            return Ok(());
        } else {
            self.pending_loads.insert(path.clone());
            if let Err(error) = self.worker.load(path.clone()) {
                self.pending_loads.remove(&path);
                return Err(error);
            }
        }
        self.visible = true;
        self.error = None;
        self.discard_confirmation = false;
        self.match_index = 0;
        self.pending_match = None;
        Ok(())
    }

    pub(super) fn is_open(&self) -> bool {
        self.visible
    }

    pub(super) fn is_loading(&self) -> bool {
        !self.pending_loads.is_empty()
    }

    pub(super) fn is_saving(&self) -> bool {
        !self.pending_saves.is_empty()
    }

    pub(super) fn poll_worker(&mut self) {
        for task in self.worker.poll() {
            match task {
                fyle::core::editor::EditorTask::Loaded { path, result } => {
                    self.pending_loads.remove(&path);
                    match result {
                        Ok(document) if self.visible => {
                            if let Some(index) =
                                self.documents.iter().position(|item| item.path() == path)
                            {
                                self.active = index;
                            } else {
                                self.documents.push(document);
                                self.active = self.documents.len() - 1;
                            }
                            self.error = None;
                        }
                        Ok(_) => {}
                        Err(error) => {
                            self.error = Some(format!("Cannot open in Fyle editor: {error:#}"))
                        }
                    }
                }
                fyle::core::editor::EditorTask::Saved { path, result } => {
                    self.pending_saves.remove(&path);
                    match result {
                        Ok(document) => {
                            if let Some(index) =
                                self.documents.iter().position(|item| item.path() == path)
                            {
                                self.documents[index] = document;
                            }
                            self.error = None;
                        }
                        Err(error) => self.error = Some(error.to_string()),
                    }
                }
            }
        }
    }

    pub(super) fn save(&mut self) {
        self.save_active();
    }

    pub(super) fn request_close(&mut self) {
        if !self.pending_saves.is_empty() {
            self.error =
                Some("Wait for the current save to finish before closing the editor.".into());
            return;
        }
        if self.documents.iter().any(|document| document.is_dirty()) {
            self.discard_confirmation = true;
        } else {
            self.documents.clear();
            self.visible = false;
        }
    }

    pub(super) fn automation_state(&self) -> serde_json::Value {
        let active_document = self.documents.get(self.active);
        serde_json::json!({
            "open": self.is_open(),
            "loading": !self.pending_loads.is_empty(),
            "saving": !self.pending_saves.is_empty(),
            "active": self.active,
            "line_numbers": true,
            "find": self.find,
            "match_index": self.match_index,
            "match_count": active_document.map_or(0, |document| match_positions(document.text(), &self.find).len()),
            "items": self.documents.iter().enumerate().map(|(index, document)| serde_json::json!({
                "path": document.path().to_string_lossy(),
                "name": document.path().file_name().unwrap_or_default().to_string_lossy(),
                "dirty": document.is_dirty(),
                "language": selected_syntax(syntax_assets(), document.path()).name,
                "active": index == self.active,
            })).collect::<Vec<_>>(),
            "error": self.error,
            "discard_confirmation": self.discard_confirmation,
        })
    }

    pub(super) fn render(&mut self, ctx: &egui::Context) {
        if !self.is_open() {
            return;
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::CTRL, egui::Key::S)) {
            self.save_active();
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::CTRL, egui::Key::F)) {
            ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("editor_find")));
        }
        let mut close_requested = false;
        let mut save_requested = false;
        let mut requested_match = None;
        egui::Window::new("F Y L E   //   SOURCE EDITOR")
            .id(egui::Id::new("fyle_source_editor"))
            .default_size([820.0, 610.0])
            .min_size([520.0, 340.0])
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                self.render_tabs(ui);
                ui.separator();
                if self.documents.is_empty() && self.is_loading() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(80.0);
                        ui.spinner();
                        ui.label("READING LOCAL DOCUMENT…");
                    });
                }
                let is_saving = self.is_saving();
                if let Some(document) = self.documents.get_mut(self.active) {
                    let path = document.path().to_path_buf();
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(path.display().to_string())
                                .small()
                                .color(crate::theme::colors::MUTED),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            close_requested = ui.button("Close editor").clicked();
                            let save = ui
                                .add_enabled(
                                    !is_saving,
                                    egui::Button::new(if document.is_dirty() {
                                        "SAVE *"
                                    } else {
                                        "SAVE"
                                    }),
                                )
                                .on_hover_text("Ctrl+S");
                            crate::ui::expose_control(ui, "editor_save", "Save document", &save);
                            if save.clicked() {
                                save_requested = true;
                            }
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("FIND")
                                .monospace()
                                .color(crate::theme::colors::ELECTRIC_BLUE),
                        );
                        let old_find = self.find.clone();
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut self.find)
                                .id_source("editor_find")
                                .desired_width(260.0)
                                .hint_text("Search this document · Ctrl+F"),
                        );
                        crate::ui::expose_control(ui, "editor_find", "Find in document", &response);
                        if old_find != self.find {
                            self.match_index = 0;
                        }
                        let matches = match_positions(document.text(), &self.find);
                        if !matches.is_empty() {
                            let index = self.match_index.min(matches.len() - 1);
                            let previous = ui.small_button("◂").on_hover_text("Previous match");
                            crate::ui::expose_control(
                                ui,
                                "editor_find_previous",
                                "Previous match",
                                &previous,
                            );
                            if previous.clicked() {
                                let previous_index = (index + matches.len() - 1) % matches.len();
                                requested_match = Some((path.clone(), matches[previous_index]));
                                self.match_index = previous_index;
                            }
                            let next_button = ui.small_button("▸").on_hover_text("Next match");
                            crate::ui::expose_control(
                                ui,
                                "editor_find_next",
                                "Next match",
                                &next_button,
                            );
                            if next_button.clicked() {
                                let next = (index + 1) % matches.len();
                                requested_match = Some((path.clone(), matches[next]));
                                self.match_index = next;
                            }
                            ui.label(
                                RichText::new(format!("{} / {}", index + 1, matches.len()))
                                    .small()
                                    .color(crate::theme::colors::CYBER_GREEN),
                            );
                        }
                    });
                    ui.separator();
                    let assets = syntax_assets();
                    let syntax = selected_syntax(assets, &path);
                    let theme = assets
                        .themes
                        .themes
                        .get("base16-ocean.dark")
                        .or_else(|| assets.themes.themes.values().next())
                        .expect("syntect supplies a default theme");
                    let cached = self
                        .highlight_cache
                        .take()
                        .filter(|(cached_path, cached_text, _)| {
                            cached_path == &path && cached_text == document.text()
                        })
                        .unwrap_or_else(|| {
                            let source = document.text().to_owned();
                            (
                                path.clone(),
                                source.clone(),
                                highlighted_job(&source, syntax, theme, f32::INFINITY, ui.style()),
                            )
                        });
                    let cache = std::cell::RefCell::new(cached);
                    let text = document.text_mut();
                    let mut layouter = |ui: &egui::Ui, source: &str, wrap_width: f32| {
                        let mut cached = cache.borrow_mut();
                        if cached.1 != source {
                            *cached = (
                                path.clone(),
                                source.to_owned(),
                                highlighted_job(source, syntax, theme, f32::INFINITY, ui.style()),
                            );
                        }
                        let mut job = cached.2.clone();
                        job.wrap.max_width = wrap_width;
                        ui.fonts(|fonts| fonts.layout_job(job))
                    };
                    egui::ScrollArea::both()
                        .id_source(("fyle_editor_scroll", &path))
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let edit_id = ui.make_persistent_id(("editor_text", &path));
                            let line_count = text.split('\n').count().max(1);
                            let font_id = egui::TextStyle::Monospace.resolve(ui.style());
                            let digits = line_count.to_string();
                            let gutter_width = ui.fonts(|fonts| {
                                fonts
                                    .layout_no_wrap(
                                        digits,
                                        font_id.clone(),
                                        crate::theme::colors::MUTED,
                                    )
                                    .size()
                                    .x
                            }) + 16.0;
                            let output = ui
                                .add_enabled_ui(!is_saving, |ui| {
                                    let mut margin = egui::Margin::same(8.0);
                                    margin.left += gutter_width;
                                    egui::TextEdit::multiline(text)
                                        .id(edit_id)
                                        .font(egui::TextStyle::Monospace)
                                        .desired_width(f32::INFINITY)
                                        .desired_rows(24)
                                        .code_editor()
                                        .margin(margin)
                                        .layouter(&mut layouter)
                                        .show(ui)
                                })
                                .inner;
                            let response = output.response.clone();
                            paint_line_numbers(ui, &output, gutter_width);
                            crate::ui::expose_control(ui, "editor_text", "Source text", &response);
                            if let Some((requested_path, byte_index)) = &requested_match {
                                if requested_path == &path {
                                    let char_start = text[..*byte_index].chars().count();
                                    requested_match = None;
                                    self.pending_match = Some((
                                        edit_id,
                                        char_start,
                                        char_start + self.find.chars().count(),
                                    ));
                                }
                            }
                        });
                    self.highlight_cache = Some(cache.into_inner());
                    let lines = document.text().lines().count().max(1);
                    let dirty = if document.is_dirty() {
                        "UNSAVED"
                    } else {
                        "SAVED"
                    };
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("{}  ·  {lines} LINES  ·  {dirty}", syntax.name))
                                .small()
                                .color(crate::theme::colors::ELECTRIC_BLUE),
                        );
                        if document.text().len() > MAX_HIGHLIGHT_BYTES {
                            ui.label(
                                RichText::new(
                                    "syntax colors paused above 256 KiB for responsiveness",
                                )
                                .small()
                                .color(crate::theme::colors::AMBER),
                            );
                        }
                    });
                } else if ui.button("Close editor").clicked() {
                    close_requested = true;
                }
                if let Some(error) = &self.error {
                    ui.colored_label(crate::theme::colors::CRIMSON, error);
                }
                if self.discard_confirmation {
                    ui.horizontal(|ui| {
                        ui.colored_label(
                            crate::theme::colors::AMBER,
                            "Unsaved changes will be lost.",
                        );
                        if ui.button("Discard").clicked() {
                            self.documents.clear();
                            self.visible = false;
                            self.discard_confirmation = false;
                        }
                        if ui.button("Keep editing").clicked() {
                            self.discard_confirmation = false;
                        }
                    });
                }
            });
        if close_requested {
            self.request_close();
        }
        if save_requested {
            self.save_active();
        }
        if let Some((id, start, end)) = self.pending_match.take() {
            let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::new(start),
                    egui::text::CCursor::new(end),
                )));
            state.store(ctx, id);
            ctx.memory_mut(|memory| memory.request_focus(id));
        }
        if self.is_loading() || self.is_saving() {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
    }

    fn render_tabs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for (index, document) in self.documents.iter().enumerate() {
                let name = document
                    .path()
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy();
                let label = if document.is_dirty() {
                    format!("● {name}")
                } else {
                    name.to_string()
                };
                let response = ui
                    .selectable_label(index == self.active, label)
                    .on_hover_text(document.path().display().to_string());
                crate::ui::expose_control(ui, &format!("editor_tab:{index}"), &name, &response);
                if response.clicked() {
                    self.active = index;
                }
            }
        });
    }

    fn save_active(&mut self) {
        if let Some(document) = self.documents.get_mut(self.active) {
            if self.pending_saves.contains(&document.path().to_path_buf()) {
                return;
            }
            let path = document.path().to_path_buf();
            match self.worker.save(document.clone()) {
                Ok(()) => {
                    self.pending_saves.insert(path);
                    self.error = None;
                }
                Err(error) => self.error = Some(error.to_string()),
            }
        }
    }
}

fn paint_line_numbers(ui: &egui::Ui, output: &egui::text_edit::TextEditOutput, gutter_width: f32) {
    let gutter = egui::Rect::from_min_max(
        egui::pos2(
            output.galley_pos.x - gutter_width,
            output.response.rect.top(),
        ),
        egui::pos2(output.galley_pos.x - 2.0, output.response.rect.bottom()),
    );
    ui.painter()
        .rect_filled(gutter, 0.0, crate::theme::colors::DARK_SLATE);
    let clip = ui.clip_rect();
    let painter = ui.painter().with_clip_rect(clip);
    let mut line_number = 1usize;
    let mut start_of_line = true;
    let font = egui::FontId::monospace(egui::TextStyle::Monospace.resolve(ui.style()).size);
    for row in &output.galley.rows {
        if start_of_line {
            let y = output.galley_pos.y + row.rect.center().y;
            if y >= clip.top() - row.rect.height() && y <= clip.bottom() + row.rect.height() {
                painter.text(
                    egui::pos2(gutter.right() - 6.0, y),
                    egui::Align2::RIGHT_CENTER,
                    line_number.to_string(),
                    font.clone(),
                    crate::theme::colors::MUTED,
                );
            }
        }
        start_of_line = row.ends_with_newline;
        if row.ends_with_newline {
            line_number += 1;
        }
    }
}

fn highlighted_job(
    text: &str,
    syntax: &SyntaxReference,
    theme: &Theme,
    wrap_width: f32,
    style: &egui::Style,
) -> LayoutJob {
    let font_id = egui::TextStyle::Monospace.resolve(style);
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;
    if text.len() > MAX_HIGHLIGHT_BYTES || syntax.name.contains("Plain Text") {
        return plain_job(text, font_id, &mut job, syntax.name.contains("Plain Text"));
    }
    let assets = syntax_assets();
    let mut highlighter = HighlightLines::new(syntax, theme);
    for line in text.split_inclusive('\n') {
        if let Ok(parts) = highlighter.highlight_line(line, &assets.syntaxes) {
            for (highlight, fragment) in parts {
                let color = highlight.foreground;
                job.append(
                    fragment,
                    0.0,
                    TextFormat {
                        font_id: font_id.clone(),
                        color: Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a),
                        ..Default::default()
                    },
                );
            }
        } else {
            job.append(
                line,
                0.0,
                TextFormat::simple(font_id.clone(), Color32::LIGHT_GRAY),
            );
        }
    }
    if text.is_empty() {
        job.append("", 0.0, TextFormat::simple(font_id, Color32::LIGHT_GRAY));
    }
    job
}

fn plain_job(text: &str, font_id: egui::FontId, job: &mut LayoutJob, plain: bool) -> LayoutJob {
    job.append(
        text,
        0.0,
        TextFormat::simple(
            font_id,
            if plain {
                Color32::from_rgb(201, 210, 220)
            } else {
                Color32::from_rgb(165, 176, 190)
            },
        ),
    );
    std::mem::take(job)
}

fn match_positions(text: &str, query: &str) -> Vec<usize> {
    if query.is_empty() {
        return Vec::new();
    }
    text.match_indices(query).map(|(index, _)| index).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn detects_common_source_languages_and_plain_text_fallback() {
        let assets = super::syntax_assets();
        assert!(super::selected_syntax(assets, "main.rs".as_ref())
            .name
            .contains("Rust"));
        assert!(super::selected_syntax(assets, "app.py".as_ref())
            .name
            .contains("Python"));
        assert!(super::selected_syntax(assets, "readme.txt".as_ref())
            .name
            .contains("Plain Text"));
    }

    #[test]
    fn syntax_job_assigns_distinct_token_colors() {
        let assets = super::syntax_assets();
        let syntax = super::selected_syntax(assets, "main.rs".as_ref());
        let theme = &assets.themes.themes["base16-ocean.dark"];
        let job = super::highlighted_job(
            "fn main() { let answer = 42; }\n",
            syntax,
            theme,
            f32::INFINITY,
            &eframe::egui::Style::default(),
        );
        let colors: std::collections::HashSet<_> = job
            .sections
            .iter()
            .map(|section| section.format.color)
            .collect();
        assert!(colors.len() > 1, "Rust syntax should color distinct tokens");
    }

    #[test]
    fn search_finds_all_literal_matches_and_keeps_unicode_byte_boundaries() {
        assert_eq!(super::match_positions("λ value λ", "λ"), vec![0, 9]);
        assert_eq!(super::match_positions("one one", "one"), vec![0, 4]);
        assert!(super::match_positions("anything", "").is_empty());
    }
}
