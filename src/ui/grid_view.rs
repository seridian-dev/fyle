//! Virtualized file cards with a full-card target and font-independent icons.
use crate::core::format_size;
use crate::theme::colors;
use crate::types::FileEntry;
use crate::ui::{icons, ViewAction};
use eframe::egui::{self, RichText, Sense, Stroke, Ui};
const WIDTH: f32 = 152.0;
const HEIGHT: f32 = 144.0;
fn card(ui: &mut Ui, entry: &FileEntry) -> Option<ViewAction> {
    #[cfg(test)]
    crate::ui::RENDERED_ENTRIES.with(|count| count.set(count.get() + 1));
    let (rect, response) = ui.allocate_exact_size(egui::vec2(WIDTH - 8.0, HEIGHT), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            entry.selected,
            &entry.name,
        )
    });
    let mut action = None;
    let (icon, color) = icons::file_icon(entry);
    if ui.is_rect_visible(rect) {
        let fill = if entry.selected {
            crate::theme::get_selection_bg_color(true)
        } else if response.hovered() {
            egui::Color32::from_rgb(11, 40, 53)
        } else {
            colors::DARK_SLATE
        };
        ui.painter().rect(
            rect,
            3.0,
            fill,
            Stroke::new(
                1.0_f32,
                if entry.selected {
                    crate::theme::get_selection_stroke_color(true)
                } else {
                    colors::BORDER
                },
            ),
        );
        let bracket_color = if entry.selected || response.hovered() {
            colors::ELECTRIC_BLUE
        } else {
            color.linear_multiply(0.6)
        };
        for corner in [
            rect.left_top(),
            rect.right_top(),
            rect.left_bottom(),
            rect.right_bottom(),
        ] {
            let dx = if corner.x == rect.left() { 1.0 } else { -1.0 };
            let dy = if corner.y == rect.top() { 1.0 } else { -1.0 };
            ui.painter().add(egui::Shape::line(
                vec![
                    corner + egui::vec2(dx * 10.0, 0.0),
                    corner,
                    corner + egui::vec2(0.0, dy * 10.0),
                ],
                Stroke::new(1.4_f32, bracket_color),
            ));
        }
        let icon_rect = egui::Rect::from_center_size(
            egui::pos2(rect.center().x, rect.top() + 41.0),
            egui::vec2(42.0, 42.0),
        );
        icons::paint(ui.painter(), icon_rect, icon, color);
        let mut content = ui.child_ui(
            egui::Rect::from_min_max(
                rect.min + egui::vec2(10.0, 75.0),
                rect.max - egui::vec2(10.0, 8.0),
            ),
            egui::Layout::top_down(egui::Align::Center),
        );
        content.add(
            egui::Label::new(RichText::new(&entry.name).size(13.0).color(colors::TEXT))
                .truncate(true),
        );
        content.label(
            RichText::new(if entry.is_dir {
                "Folder".into()
            } else {
                format!("{} · {}", entry.file_type, format_size(entry.size))
            })
            .size(11.0)
            .color(color),
        );
    }
    let mut toggled = false;
    let mut selection_ui = ui.child_ui(
        egui::Rect::from_min_size(rect.min + egui::vec2(8.0, 7.0), egui::vec2(22.0, 22.0)),
        egui::Layout::top_down(egui::Align::Min),
    );
    if entry.selected || response.hovered() {
        let mut selected = entry.selected;
        toggled = selection_ui.checkbox(&mut selected, "").changed();
    }
    crate::ui::expose_control(ui, &format!("entry:{}", entry.name), &entry.name, &response);
    if toggled {
        action = Some(ViewAction::ToggleSelection(entry.path.clone()));
    } else if response.double_clicked() {
        if entry.is_dir {
            action = Some(ViewAction::Navigate(entry.path.clone()));
        } else {
            action = Some(ViewAction::OpenFile(entry.path.clone()));
        }
    } else if response.secondary_clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            action = Some(ViewAction::ShowContextMenu(pos, entry.clone()));
        }
    } else if response.clicked() {
        action = Some(if ui.input(|i| i.modifiers.ctrl || i.modifiers.command) {
            ViewAction::ToggleSelection(entry.path.clone())
        } else {
            ViewAction::SelectSingle(entry.path.clone())
        });
    }
    response.on_hover_text(&entry.name);
    action
}
pub fn render_grid_view(ui: &mut Ui, entries: &[FileEntry]) -> Vec<ViewAction> {
    let mut actions = Vec::new();
    let columns = ((ui.available_width() - 16.0) / (WIDTH)).floor().max(1.0) as usize;
    egui::ScrollArea::vertical()
        .id_source("file_grid_scroll")
        .auto_shrink([false, false])
        .show_rows(ui, HEIGHT, entries.len().div_ceil(columns), |ui, rows| {
            for row in rows {
                ui.horizontal(|ui| {
                    for entry in &entries[row * columns..((row + 1) * columns).min(entries.len())] {
                        if let Some(action) = card(ui, entry) {
                            actions.push(action);
                        }
                    }
                });
            }
        });
    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adjacent_cards_have_non_overlapping_hit_targets() {
        let ctx = egui::Context::default();
        let controls = crate::ui::ControlMap::default();
        ctx.data_mut(|data| {
            data.insert_temp(
                egui::Id::new(crate::ui::HARNESS_CONTROLS_ID),
                controls.clone(),
            )
        });
        let entries: Vec<_> = (0..3)
            .map(|n| FileEntry {
                name: format!("file{n}"),
                path: format!("file{n}").into(),
                size: 12,
                is_dir: false,
                modified: chrono::Local::now(),
                is_hidden: false,
                file_type: "TXT".into(),
                selected: false,
            })
            .collect();
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 600.0),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render_grid_view(ui, &entries);
                });
            },
        );
        let controls = controls.lock().unwrap();
        for n in 0..2 {
            let a = &controls[&format!("entry:file{n}")]["rect_points"];
            let b = &controls[&format!("entry:file{}", n + 1)]["rect_points"];
            assert!(
                b[0].as_f64().unwrap() >= a[0].as_f64().unwrap() + a[2].as_f64().unwrap(),
                "Cards overlap: {a} {b}"
            );
        }
    }
}
