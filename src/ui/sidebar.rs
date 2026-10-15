//! Sidebar component with quick access and bookmarks.

use crate::theme::colors;
use crate::types::{get_quick_access_locations, Bookmark};
use eframe::egui::{self, Color32, RichText, Rounding, Sense, Stroke, Ui, Vec2};
use std::path::PathBuf;
use sysinfo::Disks;

/// Actions from the sidebar.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum SidebarAction {
    /// Navigate to a path.
    Navigate(PathBuf),
    /// Remove a bookmark by index.
    RemoveBookmark(usize),
    /// Add bookmark for current path.
    AddBookmark(String),
}

fn get_location_icon(name: &str) -> &'static str {
    match name {
        "Home" => "🏠",
        "Desktop" => "🖥️",
        "Documents" => "📄",
        "Downloads" => "📥",
        "Pictures" => "🖼️",
        "Music" => "🎵",
        "Videos" => "🎬",
        _ => "📁",
    }
}

/// Styled section header for sidebar.
fn render_section_header<R>(
    ui: &mut Ui,
    title: &str,
    icon: &str,
    color: Color32,
    add_body: impl FnOnce(&mut Ui) -> R,
) -> egui::CollapsingResponse<R> {
    let header_text = format!("{} {}", icon, title);

    egui::CollapsingHeader::new(RichText::new(header_text).color(color).strong().size(14.0))
        .default_open(true)
        .show(ui, add_body)
}

/// Render a styled navigation button.
fn render_nav_button(ui: &mut Ui, icon: &str, label: &str, is_active: bool) -> bool {
    let button_height = 30.0;
    let available_width = ui.available_width();

    let bg_color = if is_active {
        colors::ELECTRIC_BLUE.linear_multiply(0.2)
    } else {
        Color32::TRANSPARENT
    };

    let text_color = if is_active {
        colors::CYBER_GREEN
    } else {
        colors::ELECTRIC_BLUE
    };

    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(available_width, button_height), Sense::click());

    if ui.is_rect_visible(rect) {
        // Background
        let bg = if response.hovered() {
            colors::ELECTRIC_BLUE.linear_multiply(0.15)
        } else {
            bg_color
        };

        ui.painter().rect(
            rect,
            Rounding::same(4.0),
            bg,
            if response.hovered() || is_active {
                Stroke::new(1.0_f32, colors::ELECTRIC_BLUE.linear_multiply(0.5))
            } else {
                Stroke::NONE
            },
        );

        // Left accent bar for active item
        if is_active {
            let accent_rect = egui::Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height()));
            ui.painter()
                .rect_filled(accent_rect, Rounding::ZERO, colors::CYBER_GREEN);
        }

        // Icon and text
        let icon_pos = rect.min + Vec2::new(12.0, (button_height - 16.0) / 2.0);
        ui.painter().text(
            icon_pos,
            egui::Align2::LEFT_TOP,
            icon,
            egui::FontId::proportional(14.0),
            text_color,
        );

        let text_pos = rect.min + Vec2::new(32.0, (button_height - 14.0) / 2.0);
        ui.painter().text(
            text_pos,
            egui::Align2::LEFT_TOP,
            label,
            egui::FontId::proportional(13.0),
            text_color,
        );

        // Hover arrow indicator
        if response.hovered() && !is_active {
            let arrow_pos = rect.right_center() - Vec2::new(16.0, 0.0);
            ui.painter().text(
                arrow_pos,
                egui::Align2::CENTER_CENTER,
                "→",
                egui::FontId::proportional(12.0),
                colors::CYBER_GREEN,
            );
        }
    }

    response.clicked()
}

/// Render a styled bookmark button with remove option.
fn render_bookmark_button(ui: &mut Ui, name: &str, is_active: bool) -> (bool, bool) {
    let button_height = 28.0;
    let available_width = ui.available_width();

    let mut clicked = false;
    let mut remove_clicked = false;

    ui.horizontal(|ui| {
        let button_width = available_width - 28.0;

        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(button_width, button_height), Sense::click());

        if ui.is_rect_visible(rect) {
            let bg = if is_active {
                colors::AMBER.linear_multiply(0.2)
            } else if response.hovered() {
                colors::AMBER.linear_multiply(0.1)
            } else {
                Color32::TRANSPARENT
            };

            ui.painter().rect(
                rect,
                Rounding::same(4.0),
                bg,
                if response.hovered() || is_active {
                    Stroke::new(1.0_f32, colors::AMBER.linear_multiply(0.5))
                } else {
                    Stroke::NONE
                },
            );

            // Left accent bar for active item
            if is_active {
                let accent_rect =
                    egui::Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height()));
                ui.painter()
                    .rect_filled(accent_rect, Rounding::ZERO, colors::AMBER);
            }

            // Bookmark icon
            let icon_pos = rect.min + Vec2::new(8.0, (button_height - 14.0) / 2.0);
            ui.painter().text(
                icon_pos,
                egui::Align2::LEFT_TOP,
                "★",
                egui::FontId::proportional(12.0),
                colors::AMBER,
            );

            // Bookmark name
            let text_pos = rect.min + Vec2::new(24.0, (button_height - 13.0) / 2.0);
            let display_name = if name.chars().count() > 18 {
                format!("{}...", name.chars().take(15).collect::<String>())
            } else {
                name.to_string()
            };
            ui.painter().text(
                text_pos,
                egui::Align2::LEFT_TOP,
                &display_name,
                egui::FontId::proportional(12.0),
                colors::AMBER,
            );
        }

        clicked = response.clicked();

        // Remove button
        let remove_response = ui
            .add(
                egui::Button::new(RichText::new("×").color(colors::CRIMSON).size(14.0))
                    .fill(Color32::TRANSPARENT)
                    .stroke(Stroke::NONE)
                    .min_size(Vec2::new(24.0, button_height)),
            )
            .on_hover_text("Remove bookmark");

        remove_clicked = remove_response.clicked();
    });

    (clicked, remove_clicked)
}

/// Render quick access section.
pub fn render_quick_access(ui: &mut Ui, current_path: &PathBuf) -> Option<SidebarAction> {
    let mut action = None;

    ui.add_space(4.0);

    render_section_header(ui, "Quick Access", "⚡", colors::ELECTRIC_BLUE, |ui| {
        ui.add_space(4.0);

        for location in get_quick_access_locations() {
            let icon = get_location_icon(location.name);
            let path = (location.get_path)();
            let is_active = path.as_ref() == Some(current_path);

            if render_nav_button(ui, icon, location.name, is_active) {
                if let Some(p) = path {
                    action = Some(SidebarAction::Navigate(p));
                }
            }
            ui.add_space(2.0);
        }
    });

    action
}

/// Render bookmarks section.
pub fn render_bookmarks(
    ui: &mut Ui,
    bookmarks: &[Bookmark],
    current_path: &PathBuf,
) -> Option<SidebarAction> {
    let mut action = None;

    ui.add_space(4.0);

    render_section_header(ui, "Bookmarks", "★", colors::AMBER, |ui| {
        ui.add_space(4.0);

        if bookmarks.is_empty() {
            ui.add_space(8.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("No bookmarks yet")
                        .italics()
                        .color(colors::AMBER.linear_multiply(0.5))
                        .size(12.0),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Right-click a folder to add")
                        .color(colors::AMBER.linear_multiply(0.4))
                        .size(10.0),
                );
            });
            ui.add_space(8.0);
        } else {
            for (index, bookmark) in bookmarks.iter().enumerate() {
                let is_active = &bookmark.path == current_path;
                let (clicked, remove_clicked) =
                    render_bookmark_button(ui, &bookmark.name, is_active);

                if clicked {
                    action = Some(SidebarAction::Navigate(bookmark.path.clone()));
                }
                if remove_clicked {
                    action = Some(SidebarAction::RemoveBookmark(index));
                }
                ui.add_space(2.0);
            }
        }
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
    let mut disks = Disks::new_with_refreshed_list();
    disks.refresh();

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

/// Render storage section.
pub fn render_storage(ui: &mut Ui) {
    ui.add_space(4.0);

    render_section_header(ui, "Storage", "💾", colors::MAGENTA, |ui| {
        ui.add_space(4.0);

        for info in get_storage_info() {
            // Disk name
            ui.label(
                RichText::new(&info.name)
                    .color(colors::ELECTRIC_BLUE)
                    .size(11.0),
            );

            // Custom styled progress bar
            let bar_height = 8.0;
            let available_width = ui.available_width();

            let (rect, _response) =
                ui.allocate_exact_size(Vec2::new(available_width, bar_height), Sense::hover());

            if ui.is_rect_visible(rect) {
                // Background
                ui.painter()
                    .rect_filled(rect, Rounding::same(4.0), colors::MIDNIGHT_PURPLE);

                // Progress fill
                let fill_width = rect.width() * (info.usage_percent / 100.0);
                let fill_rect =
                    egui::Rect::from_min_size(rect.min, Vec2::new(fill_width, bar_height));

                // Color based on usage
                let fill_color = if info.usage_percent > 90.0 {
                    colors::CRIMSON
                } else if info.usage_percent > 70.0 {
                    colors::AMBER
                } else {
                    colors::CYBER_GREEN
                };

                ui.painter()
                    .rect_filled(fill_rect, Rounding::same(4.0), fill_color);

                // Border
                ui.painter().rect_stroke(
                    rect,
                    Rounding::same(4.0),
                    Stroke::new(1.0_f32, colors::ELECTRIC_BLUE.linear_multiply(0.3)),
                );
            }

            // Usage text
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{:.1}%", info.usage_percent))
                        .color(colors::ELECTRIC_BLUE)
                        .size(10.0),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!(
                            "{} / {}",
                            format_bytes(info.used),
                            format_bytes(info.total)
                        ))
                        .color(colors::ELECTRIC_BLUE.linear_multiply(0.6))
                        .size(10.0),
                    );
                });
            });

            ui.add_space(8.0);
        }
    });
}

/// Render complete sidebar.
pub fn render_sidebar(
    ui: &mut Ui,
    bookmarks: &[Bookmark],
    current_path: &PathBuf,
) -> Option<SidebarAction> {
    // Sidebar header
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(
            RichText::new("Navigation")
                .color(colors::ELECTRIC_BLUE)
                .strong()
                .size(16.0),
        );
    });
    ui.add_space(4.0);

    // Separator with gradient effect
    let sep_rect = ui.available_rect_before_wrap();
    let sep_rect = egui::Rect::from_min_size(sep_rect.min, Vec2::new(sep_rect.width(), 1.0));
    ui.painter().rect_filled(
        sep_rect,
        Rounding::ZERO,
        colors::ELECTRIC_BLUE.linear_multiply(0.3),
    );
    ui.add_space(8.0);

    // Quick access
    let mut action = render_quick_access(ui, current_path);

    ui.add_space(4.0);

    // Bookmarks
    if let Some(bookmark_action) = render_bookmarks(ui, bookmarks, current_path) {
        action = Some(bookmark_action);
    }

    ui.add_space(4.0);

    // Storage
    render_storage(ui);

    action
}
