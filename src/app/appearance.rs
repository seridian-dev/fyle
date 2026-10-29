//! Appearance controls emit settings changes; GL resources stay in theme/shader.
use super::*;
impl FileManager {
    pub(crate) fn initialize_appearance(
        &mut self,
        ctx: &egui::Context,
        gl: Option<&eframe::glow::Context>,
    ) {
        crate::theme::apply_appearance(ctx, &self.appearance);
        if let Some(gl) = gl {
            match crate::theme::shader::HudShader::new(gl) {
                Ok(shader) => self.shader = Some(std::sync::Arc::new(shader)),
                Err(error) => {
                    log::warn!("HUD shader unavailable: {error}");
                    self.shader_error = Some(error);
                }
            }
        }
    }
    pub(super) fn render_background(&self, ui: &egui::Ui) {
        if !self.appearance.effects {
            return;
        }
        if let Some(shader) = &self.shader {
            let moving =
                self.appearance.animate && self.automation.is_none() && ui.input(|i| i.focused);
            let time = if moving {
                ui.input(|i| i.time) as f32
            } else {
                0.0
            };
            shader.paint(ui, ui.max_rect(), time, &self.appearance);
            if moving {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(50));
            }
        }
    }
    pub(super) fn render_appearance(&mut self, ctx: &egui::Context) {
        if !self.appearance_open {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.appearance_open = false;
            return;
        }
        let before = self.appearance.clone();
        let mut save = false;
        let mut close = false;
        egui::Window::new("Fyle Preferences")
            .title_bar(false)
            .frame(egui::Frame::none())
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .default_size(egui::vec2(760.0, 560.0))
            .max_size(egui::vec2(900.0, 680.0))
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(18, 22, 34))
                    .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(42, 68, 82)))
                    .rounding(egui::Rounding::same(12.0))
                    .inner_margin(egui::Margin::same(0.0))
                    .show(ui, |ui| {
                        ui.set_min_size(egui::vec2(700.0, 520.0));
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.add_space(20.0);
                                ui.vertical(|ui| {
                                    ui.add_space(13.0);
                                    ui.label(
                                        RichText::new("FYLE SETTINGS")
                                            .monospace()
                                            .size(10.0)
                                            .strong()
                                            .color(colors::ELECTRIC_BLUE),
                                    );
                                    ui.heading("Preferences");
                                });
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let close_button = crate::ui::buttons::compact(
                                            ui,
                                            RichText::new("×").size(20.0),
                                        )
                                            .on_hover_text("Close Preferences");
                                        crate::ui::expose_control(
                                            ui,
                                            "preferences_close",
                                            "Close Preferences",
                                            &close_button,
                                        );
                                        close = close_button.clicked();
                                        ui.add_space(10.0);
                                    },
                                );
                                ui.add_space(8.0);
                            });
                            ui.add_space(12.0);
                            ui.separator();
                            ui.horizontal(|ui| {
                                ui.allocate_ui_with_layout(
                                    egui::vec2(184.0, ui.available_height()),
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| {
                                        ui.add_space(16.0);
                                        ui.add_space(8.0);
                                        for (section, id, title, description) in [
                                            (super::PreferencesSection::General, "preferences_general", "General", "Workspace"),
                                            (super::PreferencesSection::Appearance, "preferences_appearance_section", "Appearance", "Color & motion"),
                                            (super::PreferencesSection::Terminal, "preferences_terminal_section", "Terminal", "Font & shell"),
                                        ] {
                                            let selected = self.preferences_section == section;
                                            let button = egui::Button::new(
                                                egui::RichText::new(format!("{title}\n{description}"))
                                                    .size(13.0)
                                                    .color(if selected { colors::CYBER_GREEN } else { colors::TEXT }),
                                            )
                                            .fill(if selected {
                                                egui::Color32::from_rgb(15, 55, 63)
                                            } else {
                                                egui::Color32::TRANSPARENT
                                            })
                                            .rounding(egui::Rounding::same(7.0));
                                            let response = ui.add_sized(
                                                egui::vec2(158.0, 52.0),
                                                button,
                                            );
                                            crate::ui::expose_control(ui, id, title, &response);
                                            if response.clicked() {
                                                self.preferences_section = section;
                                            }
                                            ui.add_space(4.0);
                                        }
                                    },
                                );
                                ui.separator();
                                egui::ScrollArea::vertical()
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.vertical(|ui| {
                                            ui.add_space(18.0);
                                            ui.add_space(12.0);
                                            match self.preferences_section {
                                            super::PreferencesSection::General => {
                                                ui.heading("General");
                                                ui.label("Choose what Fyle keeps in view while you work.");
                                                ui.add_space(18.0);
                                                preference_group(ui, "WINDOW");
                                                let response = ui.checkbox(
                                                    &mut self.appearance.show_status_bar,
                                                    "Show status bar at bottom",
                                                );
                                                crate::ui::expose_control(
                                                    ui,
                                                    "preferences_status_bar",
                                                    "Show status bar",
                                                    &response,
                                                );
                                                ui.small("Shows the current folder, item count and selection summary.");
                                            }
                                            super::PreferencesSection::Appearance => {
                                                ui.heading("Appearance");
                                                ui.label("Tune Fyle’s colors, motion and visual effects.");
                                                ui.add_space(18.0);
                                                preference_group(ui, "VISUAL EFFECTS");
                                                let response = ui.checkbox(
                                                    &mut self.appearance.effects,
                                                    "OpenGL field, haze and scanlines",
                                                );
                                                crate::ui::expose_control(ui, "appearance_effects", "OpenGL effects", &response);
                                                let response = ui.checkbox(&mut self.appearance.animate, "Animate while focused");
                                                crate::ui::expose_control(ui, "appearance_motion", "Animate effects", &response);
                                                preference_row(ui, "Effect intensity", |ui| {
                                                    ui.add_sized(
                                                        egui::vec2(300.0, 26.0),
                                                        egui::Slider::new(&mut self.appearance.intensity, 0.0..=1.0),
                                                    );
                                                });
                                                preference_row(ui, "Interface scale", |ui| {
                                                    ui.add_sized(
                                                        egui::vec2(300.0, 26.0),
                                                        egui::Slider::new(&mut self.appearance.scale, 0.85..=1.5),
                                                    );
                                                });
                                                preference_row(ui, "Highlight color", |ui| {
                                                    ui.color_edit_button_srgb(&mut self.appearance.highlight);
                                                });
                                                if let Some(error) = &self.shader_error {
                                                    ui.colored_label(colors::AMBER, format!("Static fallback: {error}"));
                                                }
                                            }
                                            super::PreferencesSection::Terminal => {
                                                ui.heading("Terminal");
                                                ui.label("Shape your shell workspace. Changes preview live.");
                                                ui.add_space(18.0);
                                                preference_group(ui, "INTEGRATIONS");
                                                let response = ui.checkbox(
                                                    &mut self.appearance.allow_external_terminals,
                                                    "Allow Foot / cmux integration",
                                                );
                                                crate::ui::expose_control(ui, "preferences_external_terminals", "Allow external terminal integration", &response);
                                                let status = if self.appearance.allow_external_terminals {
                                                    super::terminal::external_terminal_name().map(|name| format!("Detected {name}; external handoff is enabled.")).unwrap_or_else(|| "No supported external terminal found. The built-in terminal remains available.".into())
                                                } else {
                                                    "Disabled. Fyle will keep terminal sessions in its own popout.".into()
                                                };
                                                ui.small(status);
                                                ui.add_space(14.0);
                                                preference_group(ui, "TYPOGRAPHY");
                                                preference_row(ui, "Font family", |ui| {
                                                    egui::ComboBox::from_id_source("terminal_font")
                                                        .width(290.0)
                                                        .selected_text(match self.appearance.terminal_font {
                                                            fyle::appearance::TerminalFont::JetBrainsMonoNerd => "JetBrains Mono Nerd Font",
                                                            fyle::appearance::TerminalFont::SystemMonospace => "System Monospace",
                                                        })
                                                        .show_ui(ui, |ui| {
                                                            ui.selectable_value(&mut self.appearance.terminal_font, fyle::appearance::TerminalFont::JetBrainsMonoNerd, "JetBrains Mono Nerd Font");
                                                            ui.selectable_value(&mut self.appearance.terminal_font, fyle::appearance::TerminalFont::SystemMonospace, "System Monospace");
                                                        });
                                                });
                                                preference_row(ui, "Font size", |ui| {
                                                    ui.add_sized(egui::vec2(300.0, 26.0), egui::Slider::new(&mut self.appearance.terminal_font_size, 10.0..=28.0));
                                                });
                                                preference_row(ui, "Line spacing", |ui| {
                                                    ui.add_sized(egui::vec2(300.0, 26.0), egui::Slider::new(&mut self.appearance.terminal_line_spacing, 0.9..=1.8));
                                                });
                                                preference_group(ui, "COLOR AND CURSOR");
                                                preference_row(ui, "Color theme", |ui| {
                                                    egui::ComboBox::from_id_source("terminal_theme")
                                                        .width(290.0)
                                                        .selected_text(match self.appearance.terminal_theme {
                                                            fyle::appearance::TerminalTheme::FyleMidnight => "Fyle Midnight",
                                                            fyle::appearance::TerminalTheme::Nord => "Nord",
                                                            fyle::appearance::TerminalTheme::SolarizedDark => "Solarized Dark",
                                                        })
                                                        .show_ui(ui, |ui| {
                                                            ui.selectable_value(&mut self.appearance.terminal_theme, fyle::appearance::TerminalTheme::FyleMidnight, "Fyle Midnight");
                                                            ui.selectable_value(&mut self.appearance.terminal_theme, fyle::appearance::TerminalTheme::Nord, "Nord");
                                                            ui.selectable_value(&mut self.appearance.terminal_theme, fyle::appearance::TerminalTheme::SolarizedDark, "Solarized Dark");
                                                        });
                                                });
                                                preference_row(ui, "Cursor", |ui| {
                                                    egui::ComboBox::from_id_source("terminal_cursor")
                                                        .width(290.0)
                                                        .selected_text(match self.appearance.terminal_cursor {
                                                            fyle::appearance::TerminalCursor::Block => "Block",
                                                            fyle::appearance::TerminalCursor::Bar => "Bar",
                                                            fyle::appearance::TerminalCursor::Underline => "Underline",
                                                        })
                                                        .show_ui(ui, |ui| {
                                                            ui.selectable_value(&mut self.appearance.terminal_cursor, fyle::appearance::TerminalCursor::Block, "Block");
                                                            ui.selectable_value(&mut self.appearance.terminal_cursor, fyle::appearance::TerminalCursor::Bar, "Bar");
                                                            ui.selectable_value(&mut self.appearance.terminal_cursor, fyle::appearance::TerminalCursor::Underline, "Underline");
                                                        });
                                                });
                                                egui::Frame::group(ui.style())
                                                    .fill(egui::Color32::from_rgb(8, 16, 24))
                                                    .show(ui, |ui| {
                                                        ui.label(egui::RichText::new("❯ ~/Projects/fyle   main  ✦ ready").monospace().color(colors::CYBER_GREEN));
                                                    });
                                            }
                                            }
                                        });
                                    });
                            });
                            ui.separator();
                            ui.horizontal(|ui| {
                                ui.add_space(16.0);
                                if self.preferences_section == super::PreferencesSection::Appearance {
                                    let response = crate::ui::buttons::subtle(ui, "Reset highlight");
                                    crate::ui::expose_control(ui, "appearance_reset", "Reset highlight color", &response);
                                    if response.clicked() {
                                        self.appearance.highlight = fyle::appearance::Appearance::default().highlight;
                                    }
                                }
                                if let Some(error) = &self.appearance_error {
                                    ui.colored_label(colors::AMBER, error);
                                }
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let close_button = crate::ui::buttons::subtle(ui, "Done");
                                    crate::ui::expose_control(ui, "preferences_close", "Close Preferences", &close_button);
                                    close |= close_button.clicked();
                                    let save_button = crate::ui::buttons::accent(ui, "Save changes", colors::CYBER_GREEN);
                                    crate::ui::expose_control(ui, "appearance_save", "Save preferences", &save_button);
                                    save = save_button.clicked();
                                });
                                ui.add_space(8.0);
                            });
                            ui.add_space(8.0);
                        });
                    });
            });
        if before != self.appearance {
            crate::theme::apply_appearance(ctx, &self.appearance);
        }
        if save {
            self.appearance_error = self
                .appearance_store
                .save(&self.appearance)
                .err()
                .map(|e| e.to_string());
        }
        if close {
            self.appearance_open = false;
        }
    }
}

fn preference_group(ui: &mut egui::Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(
        RichText::new(title)
            .monospace()
            .size(10.0)
            .strong()
            .color(colors::ELECTRIC_BLUE),
    );
    ui.add_space(4.0);
}

fn preference_row(ui: &mut egui::Ui, label: &str, add_control: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.add_sized(
            egui::vec2(128.0, 28.0),
            egui::Label::new(RichText::new(label).color(colors::MUTED)),
        );
        ui.scope(|ui| {
            let visuals = ui.visuals_mut();
            visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(10, 31, 44);
            visuals.widgets.inactive.weak_bg_fill = egui::Color32::from_rgb(10, 31, 44);
            visuals.widgets.inactive.bg_stroke =
                egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(25, 105, 122));
            visuals.widgets.inactive.fg_stroke.color = colors::TEXT;
            visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(13, 48, 61);
            visuals.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(13, 48, 61);
            visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, colors::ELECTRIC_BLUE);
            visuals.widgets.hovered.fg_stroke.color = colors::TEXT;
            visuals.widgets.active.bg_fill = egui::Color32::from_rgb(5, 113, 137);
            visuals.widgets.active.fg_stroke.color = colors::TEXT;
            visuals.widgets.open.bg_fill = egui::Color32::from_rgb(13, 48, 61);
            visuals.widgets.open.weak_bg_fill = egui::Color32::from_rgb(13, 48, 61);
            visuals.widgets.open.bg_stroke = egui::Stroke::new(1.0_f32, colors::ELECTRIC_BLUE);
            visuals.widgets.open.fg_stroke.color = colors::TEXT;
            add_control(ui);
        });
    });
}
