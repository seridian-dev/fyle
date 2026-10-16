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
        egui::Window::new("Appearance // original signal")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .default_width(430.0)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("CYAN / GREEN / MAGENTA / AMBER")
                        .monospace()
                        .color(colors::ELECTRIC_BLUE),
                );
                ui.label("Original file colors stay intact. Tune highlights, effects and scale.");
                ui.separator();
                let response = ui.checkbox(
                    &mut self.appearance.effects,
                    "OpenGL field, haze and scanlines",
                );
                crate::ui::expose_control(ui, "appearance_effects", "OpenGL effects", &response);
                let response = ui.checkbox(&mut self.appearance.animate, "Animate while focused");
                crate::ui::expose_control(ui, "appearance_motion", "Animate effects", &response);
                ui.add(
                    egui::Slider::new(&mut self.appearance.intensity, 0.0..=1.0)
                        .text("Effect intensity"),
                );
                ui.add(egui::Slider::new(&mut self.appearance.scale, 0.85..=1.5).text("UI scale"));
                ui.horizontal(|ui| {
                    ui.label("Highlight");
                    ui.color_edit_button_srgb(&mut self.appearance.highlight);
                });
                if let Some(error) = &self.shader_error {
                    ui.colored_label(colors::AMBER, format!("Static fallback: {error}"));
                }
                if let Some(error) = &self.appearance_error {
                    ui.colored_label(colors::AMBER, error);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    let response = ui.button("Original palette");
                    crate::ui::expose_control(
                        ui,
                        "appearance_reset",
                        "Original palette",
                        &response,
                    );
                    if response.clicked() {
                        self.appearance = fyle::appearance::Appearance::default();
                    }
                    let response = ui.button("Save");
                    crate::ui::expose_control(ui, "appearance_save", "Save appearance", &response);
                    save = response.clicked();
                    close = ui.button("Close").clicked();
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
