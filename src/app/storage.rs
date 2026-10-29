//! Frequently changing Storage Explorer UI over the headless core scanner.
use super::*;
use anyhow::Context;
use std::path::Path;

#[derive(Default)]
pub(super) struct StorageExplorer {
    pub open: bool,
    root: PathBuf,
    root_input: String,
    tree_job: Option<fyle::core::storage::TreeScanJob>,
    progress: fyle::core::storage::StorageProgress,
    summary: Option<fyle::core::storage::StorageSummary>,
    tree: Option<fyle::core::storage::DiskNode>,
    selected: Option<PathBuf>,
    error: Option<String>,
}

pub(super) enum StorageAction {
    #[cfg(feature = "trash")]
    Trash(PathBuf),
    Preferences,
}

impl StorageExplorer {
    pub(super) fn toggle(&mut self, current_path: &Path) {
        self.open = !self.open;
        if self.open && self.root.as_os_str().is_empty() {
            self.root_input = current_path.display().to_string();
        }
    }

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
        self.tree_job.is_some()
    }

    pub(super) fn scan(&mut self, root: PathBuf) {
        self.root_input = root.display().to_string();
        self.root = root.clone();
        self.tree_job = Some(fyle::core::storage::TreeScanJob::start(root));
        self.progress = Default::default();
        self.summary = None;
        self.tree = None;
        self.selected = None;
        self.error = None;
    }

    pub(super) fn poll(&mut self) {
        if let Some(job) = &self.tree_job {
            if let Some(progress) = job.poll_progress() {
                self.progress = progress;
            }
            if let Some(result) = job.poll_result() {
                self.tree_job = None;
                match result {
                    Ok(tree) => {
                        self.summary =
                            Some(summary_from_tree(&tree, self.progress.unreadable_entries));
                        self.tree = Some(tree);
                    }
                    Err(error) => {
                        if !error.to_string().contains("cancelled") {
                            self.error = Some(format!("Storage scan failed: {error:#}"));
                        }
                    }
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
                let response = storage_button(ui, "Preferences…", colors::ELECTRIC_BLUE);
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
            let response = storage_button(ui, "Scan", colors::CYBER_GREEN);
            crate::ui::expose_control(ui, "storage_scan", "Scan folder", &response);
            if response.clicked() {
                self.scan(PathBuf::from(self.root_input.clone()));
            }
        });

        if self.tree_job.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(format!(
                    "Scanning {} entries · {} found unreadable",
                    self.progress.visited_entries, self.progress.unreadable_entries
                ));
                let response = crate::ui::buttons::compact(ui, "Cancel");
                crate::ui::expose_control(ui, "storage_cancel", "Cancel scan", &response);
                if response.clicked() {
                    if let Some(job) = &self.tree_job {
                        job.cancel();
                    }
                    self.tree_job = None;
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
            let pie_drill = render_disk_pie(ui, self.tree.as_ref(), &mut self.selected);
            if drill.is_none() {
                drill = pie_drill;
            }

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
                        let response = storage_button(ui, "Open folder", colors::ELECTRIC_BLUE);
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
                        let response = storage_button(ui, "Move to Trash…", colors::CRIMSON);
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
        } else if self.tree_job.is_some() {
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

    pub(super) fn selected_preview_path(&self) -> Option<PathBuf> {
        let selected = self.selected.as_ref()?;
        self.summary
            .as_ref()?
            .entries
            .iter()
            .find(|entry| entry.path == *selected)
            .filter(|entry| entry.kind == fyle::core::storage::StorageEntryKind::File)
            .map(|entry| entry.path.clone())
    }
}

fn storage_button(ui: &mut egui::Ui, label: &str, accent: egui::Color32) -> egui::Response {
    crate::ui::buttons::accent(ui, label, accent)
}

fn summary_from_tree(
    tree: &fyle::core::storage::DiskNode,
    unreadable_entries: u64,
) -> fyle::core::storage::StorageSummary {
    use fyle::core::storage::{StorageEntry, StorageEntryKind, StorageSummary};

    fn counts(node: &fyle::core::storage::DiskNode, include_directory: bool) -> (u64, u64) {
        if node.kind == StorageEntryKind::File {
            return (1, 0);
        }
        let mut files = 0;
        let mut directories = u64::from(include_directory);
        for child in &node.children {
            let (child_files, child_directories) = counts(child, true);
            files += child_files;
            directories += child_directories;
        }
        (files, directories)
    }

    let (file_count, directory_count) = counts(tree, false);
    let entries = tree
        .children
        .iter()
        .map(|child| {
            let (file_count, directory_count) = counts(child, false);
            StorageEntry {
                path: child.path.clone(),
                name: child.name.clone(),
                kind: child.kind,
                apparent_bytes: child.apparent_bytes,
                file_count,
                directory_count,
            }
        })
        .collect();

    StorageSummary {
        root: tree.path.clone(),
        apparent_bytes: tree.apparent_bytes,
        file_count,
        directory_count,
        unreadable_entries,
        omitted_root_entries: 0,
        entries,
    }
}

/// Sunburst donut over the scanned tree: one ring per directory level,
/// hover tooltips, click to select, click a selected folder again to drill.
fn render_disk_pie(
    ui: &mut egui::Ui,
    tree: Option<&fyle::core::storage::DiskNode>,
    selected: &mut Option<PathBuf>,
) -> Option<PathBuf> {
    const PIE_HEIGHT: f32 = 280.0;
    let width = ui.available_width();
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, PIE_HEIGHT), egui::Sense::click());
    let painter = ui.painter();
    painter.rect_filled(rect, 6.0, colors::DARK_SLATE);

    let Some(tree) = tree else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Building disk map…",
            egui::FontId::proportional(13.0),
            colors::MUTED,
        );
        return None;
    };

    let radius = (PIE_HEIGHT * 0.5 - 14.0).min(width * 0.5 - 14.0);
    let center = rect.center();
    let hole = radius * 0.34;
    let ring = (radius - hole) / PIE_RINGS as f32;

    // Neon glow behind the donut.
    for (extra, alpha) in [(10.0, 0.05), (6.0, 0.10), (3.0, 0.18)] {
        painter.circle_stroke(
            center,
            radius + extra,
            egui::Stroke::new(extra * 0.8, colors::ELECTRIC_BLUE.gamma_multiply(alpha)),
        );
    }

    let mut segments: Vec<PieSegment> = Vec::new();
    collect_pie_segments(tree, 0.0, 1.0, hole, hole + ring, 0, &mut segments);
    for segment in &segments {
        paint_pie_segment(ui, center, segment, false);
    }

    // Center readout: total usage, or the selected item.
    if let Some(path) = selected.as_ref() {
        if let Some(segment) = segments
            .iter()
            .find(|segment| segment.path.as_ref() == Some(path))
        {
            let percent = if tree.apparent_bytes > 0 {
                segment.bytes as f64 / tree.apparent_bytes as f64 * 100.0
            } else {
                0.0
            };
            painter.text(
                center + egui::vec2(0.0, -9.0),
                egui::Align2::CENTER_CENTER,
                crate::ui::truncate_name(&segment.name, 18),
                egui::FontId::proportional(12.0),
                colors::TEXT,
            );
            painter.text(
                center + egui::vec2(0.0, 8.0),
                egui::Align2::CENTER_CENTER,
                format!("{} · {percent:.1}%", format_size(segment.bytes)),
                egui::FontId::proportional(11.0),
                colors::CYBER_GREEN,
            );
        }
    } else if tree.children.is_empty() {
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            "Empty folder",
            egui::FontId::proportional(12.0),
            colors::MUTED,
        );
    } else {
        painter.text(
            center + egui::vec2(0.0, -9.0),
            egui::Align2::CENTER_CENTER,
            format_size(tree.apparent_bytes),
            egui::FontId::proportional(14.0),
            colors::CYBER_GREEN,
        );
        painter.text(
            center + egui::vec2(0.0, 9.0),
            egui::Align2::CENTER_CENTER,
            "APPARENT USAGE",
            egui::FontId::proportional(9.0),
            colors::MUTED,
        );
    }

    // Hover highlight, tooltip and click handling.
    let mut drill = None;
    let hovered = response
        .hover_pos()
        .and_then(|pos| hit_test_pie(center, radius, &segments, pos));
    if let Some(segment) = hovered {
        paint_pie_segment(ui, center, segment, true);
        let percent = if tree.apparent_bytes > 0 {
            segment.bytes as f64 / tree.apparent_bytes as f64 * 100.0
        } else {
            0.0
        };
        let kind = if segment.is_dir { "Folder" } else { "File" };
        let _ = response.clone().on_hover_text(format!(
            "{kind} · {}\n{} · {percent:.1}% of scan\n{}",
            format_size(segment.bytes),
            segment.name,
            segment
                .path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "Aggregated smaller items".to_string()),
        ));
    }
    if response.clicked() {
        match response
            .interact_pointer_pos()
            .and_then(|pos| hit_test_pie(center, radius, &segments, pos))
        {
            Some(segment) => {
                let already_selected =
                    segment.path.is_some() && selected.as_ref() == segment.path.as_ref();
                if already_selected && segment.is_dir {
                    drill = segment.path.clone();
                } else if let Some(path) = &segment.path {
                    *selected = Some(path.clone());
                }
            }
            None => {
                if let Some(pos) = response.interact_pointer_pos() {
                    if pos.distance(center) < hole {
                        *selected = None;
                    }
                }
            }
        }
    }
    drill
}

const PIE_RINGS: usize = 4;
const PIE_MAX_CHILDREN: usize = 24;
const PIE_MAX_SEGMENTS: usize = 4000;
const PIE_MIN_SPAN: f64 = 1e-4;

struct PieSegment {
    path: Option<PathBuf>,
    name: String,
    bytes: u64,
    is_dir: bool,
    start: f64,
    span: f64,
    inner: f32,
    outer: f32,
    depth: usize,
}

fn collect_pie_segments(
    node: &fyle::core::storage::DiskNode,
    start: f64,
    span: f64,
    inner: f32,
    outer: f32,
    depth: usize,
    out: &mut Vec<PieSegment>,
) {
    if node.children.is_empty() || node.apparent_bytes == 0 || span <= 0.0 {
        return;
    }
    if out.len() >= PIE_MAX_SEGMENTS {
        return;
    }
    let shown = node.children.len().min(PIE_MAX_CHILDREN);
    let mut cursor = start;
    for child in &node.children[..shown] {
        let child_span = span * child.apparent_bytes as f64 / node.apparent_bytes as f64;
        if child_span >= PIE_MIN_SPAN {
            out.push(PieSegment {
                path: Some(child.path.clone()),
                name: child.name.clone(),
                bytes: child.apparent_bytes,
                is_dir: child.kind == fyle::core::storage::StorageEntryKind::Directory,
                start: cursor,
                span: child_span,
                inner,
                outer,
                depth,
            });
            if depth + 1 < PIE_RINGS
                && child.kind == fyle::core::storage::StorageEntryKind::Directory
            {
                collect_pie_segments(
                    child,
                    cursor,
                    child_span,
                    outer,
                    outer + (outer - inner),
                    depth + 1,
                    out,
                );
            }
        }
        cursor += child_span;
    }
    if shown < node.children.len() {
        let rest: u64 = node.children[shown..]
            .iter()
            .map(|child| child.apparent_bytes)
            .sum();
        if rest > 0 {
            out.push(PieSegment {
                path: None,
                name: format!("{} more items", node.children.len() - shown),
                bytes: rest,
                is_dir: false,
                start: cursor,
                span: span * rest as f64 / node.apparent_bytes as f64,
                inner,
                outer,
                depth,
            });
        }
    }
}

fn pie_hue(index: usize, depth: usize) -> egui::Color32 {
    const HUES: [egui::Color32; 4] = [
        colors::ELECTRIC_BLUE,
        colors::CYBER_GREEN,
        colors::AMBER,
        colors::MAGENTA,
    ];
    HUES[index % 4].gamma_multiply((1.0 - 0.22 * depth as f32).max(0.45))
}

fn lighten(color: egui::Color32, amount: f32) -> egui::Color32 {
    let mix = |channel: u8| (channel as f32 + (255.0 - channel as f32) * amount).round() as u8;
    egui::Color32::from_rgb(mix(color.r()), mix(color.g()), mix(color.b()))
}

fn paint_pie_segment(ui: &egui::Ui, center: egui::Pos2, segment: &PieSegment, emphasized: bool) {
    let painter = ui.painter();
    let steps = ((segment.span * 360.0).ceil() as usize).clamp(2, 90);
    let mut points = Vec::with_capacity(steps * 2 + 2);
    for step in 0..=steps {
        let fraction = segment.start + segment.span * step as f64 / steps as f64;
        points.push(pie_point(center, segment.outer, fraction));
    }
    for step in (0..=steps).rev() {
        let fraction = segment.start + segment.span * step as f64 / steps as f64;
        points.push(pie_point(center, segment.inner, fraction));
    }
    let base = pie_hue(
        // Stable per-name color so items do not flicker between frames.
        segment
            .path
            .as_ref()
            .map(|path| simple_hash(&path.display().to_string()))
            .unwrap_or(3),
        segment.depth,
    );
    let fill = if emphasized {
        lighten(base, 0.35)
    } else {
        base
    };
    let stroke = if emphasized {
        egui::Stroke::new(2.0_f32, colors::TEXT)
    } else {
        egui::Stroke::new(1.0_f32, colors::DARK_SLATE)
    };
    painter.add(egui::Shape::Path(egui::epaint::PathShape {
        points,
        closed: true,
        fill,
        stroke,
    }));
}

fn pie_point(center: egui::Pos2, radius: f32, fraction: f64) -> egui::Pos2 {
    use std::f64::consts::TAU;
    let angle = fraction * TAU;
    egui::pos2(
        center.x + (radius as f64 * angle.sin()) as f32,
        center.y - (radius as f64 * angle.cos()) as f32,
    )
}

fn hit_test_pie(
    center: egui::Pos2,
    max_radius: f32,
    segments: &[PieSegment],
    pos: egui::Pos2,
) -> Option<&PieSegment> {
    use std::f64::consts::TAU;
    let dx = (pos.x - center.x) as f64;
    let dy = (pos.y - center.y) as f64;
    let distance = (dx * dx + dy * dy).sqrt() as f32;
    if distance > max_radius {
        return None;
    }
    let mut fraction = dx.atan2(-dy) / TAU;
    if fraction < 0.0 {
        fraction += 1.0;
    }
    segments.iter().find(|segment| {
        distance >= segment.inner
            && distance < segment.outer
            && fraction >= segment.start
            && fraction < segment.start + segment.span
    })
}

fn simple_hash(text: &str) -> usize {
    text.bytes().fold(0_usize, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as usize)
    })
}
