//! Frequently changing Storage Explorer UI over the headless core scanner.
use super::*;
use anyhow::Context;

#[derive(Default)]
pub(super) struct StorageExplorer {
    pub open: bool,
    root: PathBuf,
    root_input: String,
    job: Option<fyle::core::storage::StorageScanJob>,
    progress: fyle::core::storage::StorageProgress,
    summary: Option<fyle::core::storage::StorageSummary>,
    selected: Option<PathBuf>,
    error: Option<String>,
}

pub(super) enum StorageAction {
    #[cfg(feature = "trash")]
    Trash(PathBuf),
    Preferences,
}

impl StorageExplorer {
    pub(super) fn automation_state(&self) -> serde_json::Value {
        let summary = self.summary.as_ref().map(|summary| {
            serde_json::json!({
                "root": summary.root,
                "apparent_bytes": summary.apparent_bytes,
                "file_count": summary.file_count,
                "directory_count": summary.directory_count,
                "unreadable_entries": summary.unreadable_entries,
                "omitted_root_entries": summary.omitted_root_entries,
                "entries": summary.entries.iter().take(200).map(|entry| serde_json::json!({
                    "name": entry.name,
                    "path": entry.path,
                    "kind": format!("{:?}", entry.kind).to_lowercase(),
                    "apparent_bytes": entry.apparent_bytes,
                })).collect::<Vec<_>>(),
            })
        });
        serde_json::json!({
            "open": self.open,
            "root": self.root_input,
            "scanning": self.is_scanning(),
            "progress": self.progress,
            "summary": summary,
            "selected": self.selected,
            "error": self.error,
        })
    }

    pub(super) fn select_name(&mut self, name: &str) -> anyhow::Result<()> {
        let matches = self
            .summary
            .as_ref()
            .map(|summary| {
                summary
                    .entries
                    .iter()
                    .filter(|entry| entry.name == name)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        anyhow::ensure!(
            matches.len() == 1,
            "Storage entry is absent or ambiguous: {name}"
        );
        self.selected = Some(matches[0].path.clone());
        Ok(())
    }

    pub(super) fn drill_selected(&mut self) -> anyhow::Result<()> {
        let path = self.selected.clone().context("Select a folder first")?;
        let is_directory = self.summary.as_ref().is_some_and(|summary| {
            summary.entries.iter().any(|entry| {
                entry.path == path && entry.kind == fyle::core::storage::StorageEntryKind::Directory
            })
        });
        anyhow::ensure!(is_directory, "Only scanned folders can be opened");
        self.scan(path);
        Ok(())
    }

    pub(super) fn is_scanning(&self) -> bool {
        self.job.is_some()
    }

    pub(super) fn scan(&mut self, root: PathBuf) {
        self.root_input = root.display().to_string();
        self.root = root.clone();
        self.job = Some(fyle::core::storage::StorageScanJob::start(root));
        self.progress = Default::default();
        self.summary = None;
        self.selected = None;
        self.error = None;
    }

    pub(super) fn poll(&mut self) {
        if let Some(job) = &self.job {
            if let Some(progress) = job.poll_progress() {
                self.progress = progress;
            }
            if let Some(result) = job.poll_result() {
                self.job = None;
                match result {
                    Ok(summary) => self.summary = Some(summary),
                    Err(error) => self.error = Some(format!("Storage scan failed: {error:#}")),
                }
            }
        }
    }

    pub(super) fn refresh(&mut self) {
        if !self.root.as_os_str().is_empty() {
            self.scan(self.root.clone());
        }
    }

    pub(super) fn render(&mut self, ui: &mut egui::Ui) -> Option<StorageAction> {
        let mut action = None;
        let mut drill = None;
        self.poll();
        ui.label(
            RichText::new("STORAGE // APPARENT USAGE")
                .monospace()
                .size(10.0)
                .color(colors::ELECTRIC_BLUE),
        );
        ui.horizontal(|ui| {
            ui.heading("Storage Explorer");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let response = ui.button("Preferences…");
                crate::ui::expose_control(ui, "storage_preferences", "Preferences", &response);
                if response.clicked() {
                    action = Some(StorageAction::Preferences);
                }
            });
        });
        ui.label("Scan a folder, find large items, then move selected items to Trash.");
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.root_input)
                    .desired_width(ui.available_width() - 80.0)
                    .hint_text("Folder path"),
            );
            let response = ui.button("Scan");
            crate::ui::expose_control(ui, "storage_scan", "Scan folder", &response);
            if response.clicked() {
                self.scan(PathBuf::from(self.root_input.clone()));
            }
        });

        if self.job.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(format!(
                    "Scanning {} entries · {} found unreadable",
                    self.progress.visited_entries, self.progress.unreadable_entries
                ));
                let response = ui.small_button("Cancel");
                crate::ui::expose_control(ui, "storage_cancel", "Cancel scan", &response);
                if response.clicked() {
                    if let Some(job) = &self.job {
                        job.cancel();
                    }
                    self.job = None;
                }
            });
        }
        if let Some(error) = &self.error {
            ui.colored_label(colors::CRIMSON, error);
        }

        if let Some(summary) = &self.summary {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(format_size(summary.apparent_bytes)).color(colors::CYBER_GREEN),
                );
                ui.label(format!("{} files", summary.file_count));
                ui.label(format!("{} folders", summary.directory_count));
                if summary.unreadable_entries > 0 {
                    ui.colored_label(
                        colors::AMBER,
                        format!("{} unreadable", summary.unreadable_entries),
                    );
                }
                if summary.omitted_root_entries > 0 {
                    ui.colored_label(
                        colors::AMBER,
                        format!(
                            "{} root items omitted from map",
                            summary.omitted_root_entries
                        ),
                    );
                }
            });
            ui.small("Apparent sizes count logical file lengths. Symlinks are skipped; unreadable paths are excluded.");
            Self::render_treemap(ui, &summary.entries, &mut self.selected);

            if let Some(path) = self.selected.as_ref() {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(path.display().to_string())
                            .monospace()
                            .color(colors::TEXT),
                    );
                    if summary.entries.iter().any(|entry| {
                        entry.path == *path
                            && entry.kind == fyle::core::storage::StorageEntryKind::Directory
                    }) {
                        let response = ui.button("Open folder");
                        crate::ui::expose_control(
                            ui,
                            "storage_drill",
                            "Open selected folder",
                            &response,
                        );
                        if response.clicked() {
                            drill = Some(path.clone());
                        }
                    }
                    #[cfg(feature = "trash")]
                    {
                        let response = ui.button("Move to Trash…");
                        crate::ui::expose_control(
                            ui,
                            "storage_trash",
                            "Move selected item to Trash",
                            &response,
                        );
                        if response.clicked() {
                            action = Some(StorageAction::Trash(path.clone()));
                        }
                    }
                });
            }

            let entries = &summary.entries;
            egui::ScrollArea::vertical()
                .id_source("storage_entries")
                .auto_shrink([false, false])
                .show_rows(ui, 25.0, entries.len(), |ui, rows| {
                    for index in rows {
                        let entry = &entries[index];
                        let kind = match entry.kind {
                            fyle::core::storage::StorageEntryKind::Directory => "Folder",
                            fyle::core::storage::StorageEntryKind::File => "File",
                        };
                        let response = ui.selectable_label(
                            self.selected.as_ref() == Some(&entry.path),
                            format!(
                                "{kind:<7}  {:>10}  {}",
                                format_size(entry.apparent_bytes),
                                entry.name
                            ),
                        );
                        crate::ui::expose_control(
                            ui,
                            &format!("storage_entry:{index}"),
                            &entry.name,
                            &response,
                        );
                        if response.clicked() {
                            self.selected = Some(entry.path.clone());
                        }
                        if response.double_clicked()
                            && entry.kind == fyle::core::storage::StorageEntryKind::Directory
                        {
                            drill = Some(entry.path.clone());
                        }
                    }
                });
        }
        if let Some(path) = drill {
            self.scan(path);
        }
        action
    }

    fn render_treemap(
        ui: &mut egui::Ui,
        entries: &[fyle::core::storage::StorageEntry],
        selected: &mut Option<PathBuf>,
    ) {
        const MAP_ITEMS: usize = 20;
        let width = ui.available_width().max(1.0);
        let height = 126.0;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
        ui.painter().rect_filled(rect, 6.0, colors::DARK_SLATE);
        if entries.is_empty() {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "No visible items",
                egui::FontId::proportional(14.0),
                colors::MUTED,
            );
            return;
        }

        let shown = entries.len().min(MAP_ITEMS);
        let mut items: Vec<(String, u64, Option<PathBuf>)> = entries
            .iter()
            .take(shown)
            .map(|entry| {
                (
                    entry.name.clone(),
                    entry.apparent_bytes,
                    Some(entry.path.clone()),
                )
            })
            .collect();
        let other_bytes = entries
            .iter()
            .skip(shown)
            .map(|entry| entry.apparent_bytes)
            .sum::<u64>();
        if entries.len() > shown {
            items.push(("Other items".into(), other_bytes, None));
        }
        let weights: Vec<f64> = items.iter().map(|item| item.1.max(1) as f64).collect();
        let mut blocks = vec![egui::Rect::NOTHING; items.len()];
        split_treemap(&weights, 0, weights.len(), rect, &mut blocks);
        for (index, ((name, bytes, path), block)) in items.iter().zip(blocks).enumerate() {
            let hue = [
                colors::ELECTRIC_BLUE,
                colors::CYBER_GREEN,
                colors::AMBER,
                colors::MAGENTA,
            ][index % 4];
            ui.painter()
                .rect_filled(block.shrink(1.0), 3.0, hue.gamma_multiply(0.68));
            if block.width() >= 52.0 && block.height() >= 28.0 {
                ui.painter().text(
                    block.center(),
                    egui::Align2::CENTER_CENTER,
                    crate::ui::truncate_name(name, 16),
                    egui::FontId::proportional(11.0),
                    colors::TEXT,
                );
            }
            if let Some(path) = path {
                let response = ui
                    .interact(
                        block,
                        egui::Id::new(("storage_map", index)),
                        egui::Sense::click(),
                    )
                    .on_hover_text(format!("{} · {}", name, format_size(*bytes)));
                if response.clicked() {
                    *selected = Some(path.clone());
                }
            }
        }
    }

    pub(super) fn render_status(&self, ui: &mut egui::Ui) {
        if let Some(summary) = &self.summary {
            ui.label(
                RichText::new(format!(
                    "{}  ·  {} files, {} folders",
                    format_size(summary.apparent_bytes),
                    summary.file_count,
                    summary.directory_count
                ))
                .size(12.0)
                .color(colors::MUTED),
            );
        } else if self.job.is_some() {
            ui.label(
                RichText::new(format!(
                    "Scanning {} entries…",
                    self.progress.visited_entries
                ))
                .size(12.0)
                .color(colors::MUTED),
            );
        } else {
            ui.label(
                RichText::new("Choose a folder to scan")
                    .size(12.0)
                    .color(colors::MUTED),
            );
        }
    }

    #[cfg(feature = "trash")]
    pub(super) fn selected(&self) -> Option<&PathBuf> {
        self.selected.as_ref()
    }
}

fn split_treemap(
    weights: &[f64],
    start: usize,
    end: usize,
    rect: egui::Rect,
    output: &mut [egui::Rect],
) {
    if end.saturating_sub(start) <= 1 {
        if let Some(slot) = output.get_mut(start) {
            *slot = rect;
        }
        return;
    }
    let total: f64 = weights[start..end].iter().sum();
    let mut left_total = 0.0;
    let mut split = start + 1;
    for (index, weight) in weights.iter().enumerate().take(end - 1).skip(start) {
        left_total += weight;
        split = index + 1;
        if left_total >= total / 2.0 {
            break;
        }
    }
    let ratio = (left_total / total).clamp(0.08, 0.92) as f32;
    if rect.width() >= rect.height() {
        let x = rect.left() + rect.width() * ratio;
        split_treemap(
            weights,
            start,
            split,
            egui::Rect::from_min_max(rect.min, egui::pos2(x, rect.bottom())),
            output,
        );
        split_treemap(
            weights,
            split,
            end,
            egui::Rect::from_min_max(egui::pos2(x, rect.top()), rect.max),
            output,
        );
    } else {
        let y = rect.top() + rect.height() * ratio;
        split_treemap(
            weights,
            start,
            split,
            egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), y)),
            output,
        );
        split_treemap(
            weights,
            split,
            end,
            egui::Rect::from_min_max(egui::pos2(rect.left(), y), rect.max),
            output,
        );
    }
}
