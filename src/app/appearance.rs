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
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .default_width(500.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(
                            self.preferences_section == super::PreferencesSection::General,
                            "General",
                        )
                        .clicked()
                    {
                        self.preferences_section = super::PreferencesSection::General;
                    }
                    if ui
                        .selectable_label(
                            self.preferences_section == super::PreferencesSection::Appearance,
                            "Appearance",
                        )
                        .clicked()
                    {
                        self.preferences_section = super::PreferencesSection::Appearance;
                    }
                    if ui
                        .selectable_label(
                            self.preferences_section == super::PreferencesSection::Terminal,
                            "Terminal",
                        )
                        .clicked()
                    {
                        self.preferences_section = super::PreferencesSection::Terminal;
                    }
                });
                ui.separator();
                if self.preferences_section == super::PreferencesSection::General {
                    ui.heading("General");
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
                } else if self.preferences_section == super::PreferencesSection::Appearance {
                    ui.label(
                        RichText::new("CYAN / GREEN / MAGENTA / AMBER")
                            .monospace()
                            .color(colors::ELECTRIC_BLUE),
                    );
                    ui.label(
                        "Original file colors stay intact. Tune highlights, effects and scale.",
                    );
                    ui.separator();
                    let response = ui.checkbox(
                        &mut self.appearance.effects,
                        "OpenGL field, haze and scanlines",
                    );
                    crate::ui::expose_control(
                        ui,
                        "appearance_effects",
                        "OpenGL effects",
                        &response,
                    );
                    let response =
                        ui.checkbox(&mut self.appearance.animate, "Animate while focused");
                    crate::ui::expose_control(
                        ui,
                        "appearance_motion",
                        "Animate effects",
                        &response,
                    );
                    ui.add(
                        egui::Slider::new(&mut self.appearance.intensity, 0.0..=1.0)
                            .text("Effect intensity"),
                    );
                    ui.add(
                        egui::Slider::new(&mut self.appearance.scale, 0.85..=1.5).text("UI scale"),
                    );
                    ui.horizontal(|ui| {
                        ui.label("Highlight");
                        ui.color_edit_button_srgb(&mut self.appearance.highlight);
                    });
                    if let Some(error) = &self.shader_error {
                        ui.colored_label(colors::AMBER, format!("Static fallback: {error}"));
                    }
                } else {
                    ui.heading("Terminal");
                    ui.label("Choose how the built-in terminal looks and behaves.");
                    ui.separator();
                    egui::ComboBox::from_id_source("terminal_font")
                        .selected_text(match self.appearance.terminal_font {
                            fyle::appearance::TerminalFont::JetBrainsMonoNerd => {
                                "JetBrains Mono Nerd Font"
                            }
                            fyle::appearance::TerminalFont::SystemMonospace => "System Monospace",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.appearance.terminal_font,
                                fyle::appearance::TerminalFont::JetBrainsMonoNerd,
                                "JetBrains Mono Nerd Font",
                            );
                            ui.selectable_value(
                                &mut self.appearance.terminal_font,
                                fyle::appearance::TerminalFont::SystemMonospace,
                                "System Monospace",
                            );
                        });
                    ui.add(
                        egui::Slider::new(&mut self.appearance.terminal_font_size, 10.0..=28.0)
                            .text("Font size"),
                    );
                    ui.add(
                        egui::Slider::new(&mut self.appearance.terminal_line_spacing, 0.9..=1.8)
                            .text("Line spacing"),
                    );
                    egui::ComboBox::from_id_source("terminal_theme")
                        .selected_text(match self.appearance.terminal_theme {
                            fyle::appearance::TerminalTheme::FyleMidnight => "Fyle Midnight",
                            fyle::appearance::TerminalTheme::Nord => "Nord",
                            fyle::appearance::TerminalTheme::SolarizedDark => "Solarized Dark",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.appearance.terminal_theme,
                                fyle::appearance::TerminalTheme::FyleMidnight,
                                "Fyle Midnight",
                            );
                            ui.selectable_value(
                                &mut self.appearance.terminal_theme,
                                fyle::appearance::TerminalTheme::Nord,
                                "Nord",
                            );
                            ui.selectable_value(
                                &mut self.appearance.terminal_theme,
                                fyle::appearance::TerminalTheme::SolarizedDark,
                                "Solarized Dark",
                            );
                        });
                    egui::ComboBox::from_id_source("terminal_cursor")
                        .selected_text(match self.appearance.terminal_cursor {
                            fyle::appearance::TerminalCursor::Block => "Block cursor",
                            fyle::appearance::TerminalCursor::Bar => "Bar cursor",
                            fyle::appearance::TerminalCursor::Underline => "Underline cursor",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.appearance.terminal_cursor,
                                fyle::appearance::TerminalCursor::Block,
                                "Block cursor",
                            );
                            ui.selectable_value(
                                &mut self.appearance.terminal_cursor,
                                fyle::appearance::TerminalCursor::Bar,
                                "Bar cursor",
                            );
                            ui.selectable_value(
                                &mut self.appearance.terminal_cursor,
                                fyle::appearance::TerminalCursor::Underline,
                                "Underline cursor",
                            );
                        });
                    let response = ui.checkbox(
                        &mut self.appearance.allow_external_terminals,
                        "Allow Foot / cmux integration",
                    );
                    crate::ui::expose_control(
                        ui,
                        "preferences_external_terminals",
                        "Allow external terminal integration",
                        &response,
                    );
                    if self.appearance.allow_external_terminals {
                        let status = super::terminal::external_terminal_name()
                            .map(|name| format!("Available: {name}"))
                            .unwrap_or_else(|| {
                                "Foot/cmux CLI not found; built-in terminal remains available."
                                    .into()
                            });
                        ui.small(status);
                    } else {
                        ui.small("External terminal integration is disabled.");
                    }
                }
                if let Some(error) = &self.appearance_error {
                    ui.colored_label(colors::AMBER, error);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if self.preferences_section == super::PreferencesSection::Appearance {
                        let response = ui.button("Original palette");
                        crate::ui::expose_control(
                            ui,
                            "appearance_reset",
                            "Original palette",
                            &response,
                        );
                        if response.clicked() {
                            self.appearance.highlight =
                                fyle::appearance::Appearance::default().highlight;
                        }
                    }
                    let response = ui.button("Save");
                    crate::ui::expose_control(ui, "appearance_save", "Save preferences", &response);
                    save = response.clicked();
                    let response = ui.button("Close");
                    crate::ui::expose_control(
                        ui,
                        "preferences_close",
                        "Close Preferences",
                        &response,
                    );
                    close = response.clicked();
                });
                ui.small("Live preview. Save keeps these settings for your next launch.");
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
