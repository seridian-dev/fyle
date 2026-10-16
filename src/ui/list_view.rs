//! Responsive, virtualized file list with shared header and row geometry.
use crate::core::{format_size, get_file_icon, open_with_recommended};
use crate::theme::{colors, get_entry_color};
use crate::types::{FileEntry, SortBy, SortOrder};
use crate::ui::ViewAction;
use eframe::egui::{self, RichText, Sense, Ui};

struct Columns {
    name: f32,
    size: f32,
    kind: f32,
    modified: f32,
}
impl Columns {
    fn new(width: f32) -> Self {
        let modified = if width >= 620.0 { 140.0 } else { 0.0 };
        let kind = if width >= 480.0 { 90.0 } else { 0.0 };
        let size = if width >= 280.0 { 70.0 } else { 0.0 };
        Self {
            name: (width - 48.0 - size - kind - modified).max(40.0),
            size,
            kind,
            modified,
        }
    }
}
fn cell(ui: &mut Ui, width: f32, add: impl FnOnce(&mut Ui)) {
    if width > 0.0 {
        ui.allocate_ui_with_layout(
            egui::vec2(width, 30.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_width(width);
                ui.set_max_width(width);
                add(ui);
            },
        );
    }
}
fn metadata(ui: &mut Ui, width: f32, text: String) {
    cell(ui, width, |ui| {
        ui.add(
            egui::Label::new(RichText::new(text).color(egui::Color32::from_gray(175)))
                .truncate(true),
        );
    });
}

pub fn render_list_view(
    ui: &mut Ui,
    entries: &[FileEntry],
    current_sort: &SortBy,
    sort_order: &SortOrder,
) -> Vec<ViewAction> {
    let mut actions = Vec::new();
    // Reserve scrollbar width even before it appears so headings never shift.
    let width = (ui.available_width() - 16.0).max(80.0);
    let columns = Columns::new(width);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        cell(ui, 48.0, |_| {});
        for (label, sort, width) in [
            ("Name", SortBy::Name, columns.name),
            ("Size", SortBy::Size, columns.size),
            ("Type", SortBy::Type, columns.kind),
            ("Modified", SortBy::DateModified, columns.modified),
        ] {
            cell(ui, width, |ui| {
                let arrow = if *current_sort == sort {
                    if *sort_order == SortOrder::Ascending {
                        " +"
                    } else {
                        " -"
                    }
                } else {
                    ""
                };
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new(format!("{label}{arrow}")).color(colors::ELECTRIC_BLUE),
                        )
                        .frame(false),
                    )
                    .clicked()
                {
                    actions.push(ViewAction::ChangeSort(sort));
                }
            });
        }
    });
    ui.separator();
    egui::ScrollArea::vertical()
        .id_source("file_list_scroll")
        .auto_shrink([false, false])
        .show_rows(ui, 32.0, entries.len(), |ui, rows| {
            for index in rows {
                let entry = &entries[index];
                #[cfg(test)]
                crate::ui::RENDERED_ENTRIES.with(|count| count.set(count.get() + 1));
                let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 32.0), Sense::hover());
                let row = ui.interact(rect, ui.id().with(("file", &entry.path)), Sense::click());
                let fill = if entry.selected {
                    ui.visuals().selection.bg_fill
                } else if row.hovered() {
                    ui.visuals().widgets.hovered.bg_fill
                } else if index % 2 == 1 {
                    egui::Color32::from_rgb(19, 23, 30)
                } else {
                    egui::Color32::TRANSPARENT
                };
                ui.painter().rect_filled(rect, 4.0, fill);
                let mut toggled = false;
                ui.allocate_ui_at_rect(rect, |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.horizontal(|ui| {
                        cell(ui, 24.0, |ui| {
                            let mut selected = entry.selected;
                            toggled = ui.checkbox(&mut selected, "").changed();
                        });
                        cell(ui, 24.0, |ui| {
                            ui.label(get_file_icon(entry));
                        });
                        cell(ui, columns.name, |ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&entry.name).color(get_entry_color(entry.is_dir)),
                                )
                                .truncate(true),
                            )
                            .on_hover_text(&entry.name);
                        });
                        metadata(
                            ui,
                            columns.size,
                            if entry.is_dir {
                                "—".into()
                            } else {
                                format_size(entry.size)
                            },
                        );
                        metadata(ui, columns.kind, entry.file_type.clone());
                        metadata(
                            ui,
                            columns.modified,
                            entry.modified.format("%Y-%m-%d %H:%M").to_string(),
                        );
                    });
                });
                crate::ui::expose_control(ui, &format!("entry:{}", entry.name), &entry.name, &row);
                if toggled {
                    actions.push(ViewAction::ToggleSelection(entry.path.clone()));
                } else if row.double_clicked() {
                    if entry.is_dir {
                        actions.push(ViewAction::Navigate(entry.path.clone()));
                    } else if let Err(error) = open_with_recommended(&entry.path) {
                        actions.push(ViewAction::OpenError(format!(
                            "Failed to open file: {error}"
                        )));
                    }
                } else if row.secondary_clicked() {
                    if let Some(pos) = row.interact_pointer_pos() {
                        actions.push(ViewAction::ShowContextMenu(pos, entry.clone()));
                    }
                } else if row.clicked() {
                    actions.push(ViewAction::SelectSingle(entry.path.clone()));
                }
            }
        });
    actions
}

#[cfg(test)]
mod layout_tests {
    #[test]
    fn cells_reserve_width_even_for_short_labels() {
        let ctx = eframe::egui::Context::default();
        let mut starts = Vec::new();
        let _ = ctx.run(Default::default(), |ctx| {
            eframe::egui::CentralPanel::default().show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    for width in [24.0, 300.0, 70.0] {
                        super::cell(ui, width, |ui| {
                            starts.push(ui.min_rect().left());
                            ui.label("x");
                        });
                    }
                });
            });
        });
        assert!((starts[1] - starts[0] - 24.0).abs() < 1.0);
        assert!((starts[2] - starts[1] - 300.0).abs() < 1.0);
    }

    #[test]
    fn filenames_get_the_remaining_width_and_metadata_collapses_on_small_windows() {
        let wide = super::Columns::new(720.0);
        assert!(wide.name >= 300.0);
        let narrow = super::Columns::new(320.0);
        assert!(narrow.name >= 200.0);
        assert_eq!(narrow.modified, 0.0);
        assert_eq!(narrow.kind, 0.0);
    }
}
