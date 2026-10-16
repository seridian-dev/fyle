//! Session-local read-only Git change list and diff viewer.
use eframe::egui::{self, Color32, RichText};
use std::path::PathBuf;

#[derive(Default)]
pub(super) struct Review {
    worker: fyle::core::git_review::GitReviewWorker,
    snapshot: Option<fyle::core::git_review::Snapshot>,
    selected: Option<PathBuf>,
    layer: fyle::core::git_review::DiffLayer,
    diff: Option<fyle::core::git_review::Diff>,
    diff_loading: bool,
    loading: bool,
    visible: bool,
    error: Option<String>,
}

impl Review {
    pub(super) fn open(&mut self, directory: PathBuf) {
        self.visible = true;
        self.loading = true;
        self.error = None;
        self.diff = None;
        if let Err(error) = self.worker.refresh(directory) {
            self.loading = false;
            self.error = Some(error.to_string());
        }
    }

    pub(super) fn poll(&mut self) {
        for task in self.worker.poll() {
            match task {
                fyle::core::git_review::ReviewTask::Snapshot(Ok(snapshot)) => {
                    self.loading = false;
                    self.error = None;
                    let still_present = self.selected.as_ref().is_some_and(|path| {
                        snapshot.changes.iter().any(|change| &change.path == path)
                    });
                    if !still_present {
                        self.selected = snapshot.changes.first().map(|change| change.path.clone());
                        self.diff = None;
                    }
                    self.snapshot = Some(snapshot);
                    if self.diff.is_none() {
                        self.request_diff();
                    }
                }
                fyle::core::git_review::ReviewTask::Snapshot(Err(error)) => {
                    self.loading = false;
                    self.snapshot = None;
                    self.selected = None;
                    self.diff = None;
                    self.error = Some(error.to_string());
                }
                fyle::core::git_review::ReviewTask::Diff {
                    path,
                    layer,
                    result,
                } => {
                    if self.selected.as_ref() == Some(&path) && self.layer == layer {
                        self.diff_loading = false;
                        match result {
                            Ok(diff) => {
                                self.diff = Some(diff);
                                self.error = None;
                            }
                            Err(error) => {
                                self.diff = None;
                                self.error = Some(error.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    pub(super) fn is_busy(&self) -> bool {
        self.loading || self.diff_loading
    }

    pub(super) fn automation_state(&self) -> serde_json::Value {
        serde_json::json!({
            "open": self.visible,
            "loading": self.loading || self.diff_loading,
            "root": self.snapshot.as_ref().map(|s| s.root.to_string_lossy()),
            "selected": self.selected.as_ref().map(|p| p.to_string_lossy()),
            "layer": self.layer.label(),
            "files": self.snapshot.as_ref().map(|s| s.changes.iter().map(|c| serde_json::json!({
                "path": c.path.to_string_lossy(), "old_path": c.old_path.as_ref().map(|p|p.to_string_lossy()),
                "kind": c.kind.label(), "staged": c.staged, "unstaged": c.unstaged,
            })).collect::<Vec<_>>()).unwrap_or_default(),
            "diff": self.diff.as_ref().map(|d| &d.text),
            "diff_truncated": self.diff.as_ref().is_some_and(|d| d.truncated),
            "error": self.error,
        })
    }

    pub(super) fn select(&mut self, path: PathBuf) -> anyhow::Result<()> {
        let snapshot = self
            .snapshot
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Wait for Git status to load"))?;
        anyhow::ensure!(
            snapshot.changes.iter().any(|change| change.path == path),
            "File is not in the current Git change list"
        );
        self.selected = Some(path);
        self.request_diff();
        Ok(())
    }

    pub(super) fn set_layer(&mut self, layer: fyle::core::git_review::DiffLayer) {
        if self.layer != layer {
            self.layer = layer;
            self.request_diff();
        }
    }

    pub(super) fn refresh(&mut self, directory: PathBuf) {
        self.open(directory);
    }

    fn request_diff(&mut self) {
        let (Some(snapshot), Some(path)) = (&self.snapshot, &self.selected) else {
            return;
        };
        self.diff = None;
        self.diff_loading = true;
        self.error = None;
        if let Err(error) = self
            .worker
            .diff(snapshot.root.clone(), path.clone(), self.layer)
        {
            self.diff_loading = false;
            self.error = Some(error.to_string());
        }
    }

    pub(super) fn render(&mut self, ctx: &egui::Context, directory: PathBuf) {
        if !self.visible {
            return;
        }
        let mut open = true;
        egui::Window::new("F Y L E   //   CODE REVIEW")
            .id(egui::Id::new("fyle_git_review"))
            .default_size([980.0, 650.0])
            .min_size([650.0, 400.0])
            .resizable(true)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(
                            self.snapshot
                                .as_ref()
                                .map(|s| s.root.display().to_string())
                                .unwrap_or_else(|| directory.display().to_string()),
                        )
                        .small()
                        .color(crate::theme::colors::MUTED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("↻  REFRESH").clicked() {
                            self.refresh(directory.clone());
                        }
                        if ui
                            .selectable_label(
                                self.layer == fyle::core::git_review::DiffLayer::Working,
                                "WORKING",
                            )
                            .clicked()
                        {
                            self.set_layer(fyle::core::git_review::DiffLayer::Working);
                        }
                        if ui
                            .selectable_label(
                                self.layer == fyle::core::git_review::DiffLayer::Staged,
                                "STAGED",
                            )
                            .clicked()
                        {
                            self.set_layer(fyle::core::git_review::DiffLayer::Staged);
                        }
                    });
                });
                ui.separator();
                ui.columns(2, |columns| {
                    let list = &mut columns[0];
                    if self.loading {
                        list.horizontal(|ui| {
                            ui.spinner();
                            ui.label("READING GIT STATUS…");
                        });
                    } else if let Some(snapshot) = &self.snapshot {
                        if snapshot.changes.is_empty() {
                            list.vertical_centered(|ui| {
                                ui.add_space(60.0);
                                ui.label(
                                    RichText::new("WORKTREE CLEAN")
                                        .monospace()
                                        .color(crate::theme::colors::CYBER_GREEN),
                                );
                                ui.label("No local changes to review.");
                            });
                        } else {
                            let changes = snapshot.changes.clone();
                            let mut clicked_path = None;
                            egui::ScrollArea::vertical()
                                .id_source("git_review_files")
                                .show(list, |ui| {
                                    for (index, change) in changes.iter().enumerate() {
                                        let selected = self.selected.as_ref() == Some(&change.path);
                                        let icon = match change.kind {
                                            fyle::core::git_review::ChangeKind::Added
                                            | fyle::core::git_review::ChangeKind::Untracked => "+",
                                            fyle::core::git_review::ChangeKind::Deleted => "−",
                                            fyle::core::git_review::ChangeKind::Renamed => "→",
                                            fyle::core::git_review::ChangeKind::Conflicted => "!",
                                            _ => "~",
                                        };
                                        let response = ui.selectable_label(
                                            selected,
                                            RichText::new(format!(
                                                "{icon}  {}",
                                                change.path.display()
                                            ))
                                            .monospace(),
                                        );
                                        crate::ui::expose_control(
                                            ui,
                                            &format!("review_file:{index}"),
                                            &change.path.display().to_string(),
                                            &response,
                                        );
                                        if response.clicked() {
                                            clicked_path = Some(change.path.clone());
                                        }
                                        ui.label(
                                            RichText::new(format!(
                                                "    {}  ·  {}{}",
                                                change.kind.label(),
                                                if change.staged { "INDEX " } else { "" },
                                                if change.unstaged { "WORKTREE" } else { "" }
                                            ))
                                            .small()
                                            .color(crate::theme::colors::MUTED),
                                        );
                                        ui.add_space(3.0);
                                    }
                                });
                            if let Some(path) = clicked_path {
                                self.selected = Some(path);
                                self.request_diff();
                            }
                        }
                    }
                    let diff_ui = &mut columns[1];
                    if let Some(path) = &self.selected {
                        diff_ui.label(
                            RichText::new(path.display().to_string())
                                .monospace()
                                .strong(),
                        );
                    }
                    diff_ui.separator();
                    if self.diff_loading {
                        diff_ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("READING DIFF…");
                        });
                    } else if let Some(diff) = &self.diff {
                        let lines: Vec<&str> = diff.text.lines().collect();
                        egui::ScrollArea::both()
                            .id_source("git_review_diff")
                            .show_rows(diff_ui, 18.0, lines.len(), |ui, rows| {
                                for (row, line) in lines[rows.clone()].iter().enumerate() {
                                    let color = if line.starts_with('+') {
                                        Color32::from_rgb(111, 240, 166)
                                    } else if line.starts_with('-') {
                                        Color32::from_rgb(255, 112, 137)
                                    } else if line.starts_with('@') {
                                        crate::theme::colors::ELECTRIC_BLUE
                                    } else {
                                        crate::theme::colors::TEXT
                                    };
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(format!("{:>5} ", rows.start + row + 1))
                                                .monospace()
                                                .color(crate::theme::colors::MUTED),
                                        );
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(*line).monospace().color(color),
                                            )
                                            .wrap(false),
                                        );
                                    });
                                }
                            });
                    } else if !self.loading && !self.diff_loading {
                        diff_ui.label(
                            RichText::new("Select a changed file to inspect its diff.")
                                .color(crate::theme::colors::MUTED),
                        );
                    }
                });
                if let Some(error) = &self.error {
                    ui.colored_label(crate::theme::colors::CRIMSON, error);
                }
            });
        self.visible = open;
        if self.is_busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(40));
        }
    }
}
