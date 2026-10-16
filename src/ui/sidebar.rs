//! Navigation with consistent font-independent icons and bounded storage probes.
use crate::theme::colors;
use crate::types::{get_quick_access_locations, Bookmark};
use crate::ui::icons::{self, Icon};
use eframe::egui::{self, RichText, Sense, Ui};
use std::path::PathBuf;
use sysinfo::Disks;

#[derive(Debug, Clone)]
pub enum SidebarAction {
    Navigate(PathBuf),
    RemoveBookmark(usize),
    AddBookmark(String),
}
fn location_icon(name: &str) -> Icon {
    match name {
        "Home" => Icon::Home,
        "Desktop" => Icon::Desktop,
        "Documents" => Icon::Documents,
        "Downloads" => Icon::Downloads,
        "Pictures" => Icon::Image,
        "Music" => Icon::Music,
        "Videos" => Icon::Video,
        _ => Icon::Folder,
    }
}
fn nav_row(ui: &mut Ui, id: &str, label: &str, icon: Icon, active: bool) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 36.0), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, active, label)
    });
    if ui.is_rect_visible(rect) {
        let fill = if active {
            egui::Color32::from_rgb(8, 45, 60)
        } else if response.hovered() {
            colors::MIDNIGHT_PURPLE
        } else {
            egui::Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, 2.0, fill);
        let color = match icon {
            Icon::Home | Icon::Desktop | Icon::Documents => colors::ELECTRIC_BLUE,
            Icon::Downloads => colors::CYBER_GREEN,
            Icon::Image | Icon::Music | Icon::Video => colors::MAGENTA,
            Icon::Bookmark => colors::AMBER,
            _ => colors::MUTED,
        };
        icons::paint(
            ui.painter(),
            egui::Rect::from_min_size(rect.min + egui::vec2(10.0, 8.0), egui::vec2(20.0, 20.0)),
            icon,
            color,
        );
        ui.painter().text(
            rect.min + egui::vec2(42.0, 18.0),
            egui::Align2::LEFT_CENTER,
            crate::ui::truncate_name(label, 22),
            egui::FontId::proportional(14.0),
            if active { colors::TEXT } else { colors::MUTED },
        );
    }
    crate::ui::expose_control(ui, id, label, &response);
    response
}
pub fn render_sidebar(
    ui: &mut Ui,
    bookmarks: &[Bookmark],
    current_path: &PathBuf,
    storage: &mut StorageCache,
) -> Option<SidebarAction> {
    let mut action = None;
    egui::ScrollArea::vertical()
        .id_source("places_scroll")
        .show(ui, |ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::new("QUICK ACCESS //")
                    .size(11.0)
                    .strong()
                    .color(colors::MUTED),
            );
            ui.add_space(8.0);
            for location in get_quick_access_locations() {
                let path = (location.get_path)();
                ui.add_enabled_ui(path.is_some(), |ui| {
                    let response = nav_row(
                        ui,
                        &format!("place:{}", location.name.to_lowercase()),
                        location.name,
                        location_icon(location.name),
                        path.as_ref() == Some(current_path),
                    );
                    if let Some(path) = &path {
                        let response = response.on_hover_text(path.display().to_string());
                        if response.clicked() {
                            action = Some(SidebarAction::Navigate(path.clone()));
                        }
                    } else {
                        response.on_disabled_hover_text(
                            "This location is not configured for your user",
                        );
                    }
                });
            }
            ui.add_space(18.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("PINNED //")
                        .size(11.0)
                        .strong()
                        .color(colors::MUTED),
                );
                let response = ui.small_button("+").on_hover_text("Pin this folder");
                crate::ui::expose_control(ui, "pin_folder", "Pin this folder", &response);
                if response.clicked() {
                    action = Some(SidebarAction::AddBookmark(
                        current_path
                            .file_name()
                            .unwrap_or(current_path.as_os_str())
                            .to_string_lossy()
                            .into_owned(),
                    ));
                }
            });
            if bookmarks.is_empty() {
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Keep your places close.\nPin a folder with +")
                        .size(12.0)
                        .color(colors::MUTED),
                );
            }
            for (index, bookmark) in bookmarks.iter().enumerate() {
                let response = nav_row(
                    ui,
                    &format!("bookmark:{index}"),
                    &bookmark.name,
                    Icon::Bookmark,
                    bookmark.path == *current_path,
                )
                .on_hover_text(bookmark.path.display().to_string());
                if response.clicked() {
                    action = Some(SidebarAction::Navigate(bookmark.path.clone()));
                }
                response.context_menu(|ui| {
                    if ui.button("Unpin folder").clicked() {
                        action = Some(SidebarAction::RemoveBookmark(index));
                        ui.close_menu();
                    }
                });
            }
            ui.add_space(20.0);
            icons::show(ui, Icon::Drive, 18.0, colors::MUTED);
            egui::CollapsingHeader::new(RichText::new("Storage").color(colors::MUTED))
                .default_open(false)
                .show(ui, |ui| {
                    for info in storage.entries() {
                        ui.add(egui::Label::new(RichText::new(&info.name).small()).truncate(true))
                            .on_hover_text(&info.name);
                        ui.add(
                            egui::ProgressBar::new(info.usage_percent / 100.0)
                                .desired_height(4.0)
                                .fill(if info.usage_percent > 90.0 {
                                    colors::CRIMSON
                                } else {
                                    colors::ELECTRIC_BLUE
                                }),
                        );
                        ui.small(format!(
                            "{} of {}",
                            format_bytes(info.used),
                            format_bytes(info.total)
                        ));
                        ui.add_space(8.0);
                    }
                });
        });
    action
}

/// Storage info for display.
#[allow(dead_code)]
pub struct StorageInfo {
    pub name: String,
    pub usage_percent: f32,
    pub used: u64,
    pub total: u64,
}

/// Get storage information for all disks.
#[must_use]
pub fn get_storage_info() -> Vec<StorageInfo> {
    let disks = Disks::new_with_refreshed_list();

    disks
        .list()
        .iter()
        .map(|disk| {
            let total = disk.total_space();
            let available = disk.available_space();
            let used = total.saturating_sub(available);
            let usage_percent = if total > 0 {
                (used as f64 / total as f64 * 100.0) as f32
            } else {
                0.0
            };

            StorageInfo {
                name: format!(
                    "{} ({})",
                    disk.name().to_string_lossy(),
                    disk.mount_point().display()
                ),
                usage_percent,
                used,
                total,
            }
        })
        .collect()
}

/// Format bytes to human readable string.
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", size as u64, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

/// Disk probes are expensive; refresh at most once every five seconds.
#[derive(Default)]
pub struct StorageCache {
    refreshed: Option<std::time::Instant>,
    entries: Vec<StorageInfo>,
}
impl StorageCache {
    fn entries(&mut self) -> &[StorageInfo] {
        if self
            .refreshed
            .is_none_or(|last| last.elapsed() >= std::time::Duration::from_secs(5))
        {
            self.entries = get_storage_info();
            self.refreshed = Some(std::time::Instant::now());
        }
        &self.entries
    }
}
