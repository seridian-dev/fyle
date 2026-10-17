//! GUI ownership and rendering for the embedded shell session.
use super::*;
use eframe::egui::Event;

#[derive(Default)]
pub(super) struct TerminalPanel {
    sessions: Vec<TerminalTab>,
    active: usize,
    next_id: u32,
    open: bool,
    error: Option<String>,
}

struct TerminalTab {
    id: u32,
    label: String,
    session: fyle::core::terminal::TerminalSession,
}

impl TerminalPanel {
    pub(super) fn is_open(&self) -> bool {
        self.open
    }

    pub(super) fn close(&mut self) {
        self.open = false;
        self.sessions.clear();
        self.active = 0;
    }

    pub(super) fn open(&mut self, directory: &std::path::Path) {
        if self.sessions.is_empty() {
            self.new_session(directory);
        } else {
            self.open = true;
        }
    }

    pub(super) fn new_session(&mut self, directory: &std::path::Path) {
        if self.sessions.len() >= 8 {
            self.error = Some("Fyle supports up to 8 terminal sessions per window".into());
            self.open = true;
            return;
        }
        match fyle::core::terminal::TerminalSession::spawn(directory) {
            Ok(session) => {
                self.next_id = self.next_id.wrapping_add(1);
                let label = directory
                    .file_name()
                    .unwrap_or(directory.as_os_str())
                    .to_string_lossy()
                    .into_owned();
                self.sessions.push(TerminalTab {
                    id: self.next_id,
                    label,
                    session,
                });
                self.active = self.sessions.len() - 1;
                self.open = true;
                self.error = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    pub(super) fn close_active(&mut self) {
        if self.active < self.sessions.len() {
            self.sessions.remove(self.active);
        }
        self.active = self.active.min(self.sessions.len().saturating_sub(1));
        if self.sessions.is_empty() {
            self.open = false;
        }
    }

    pub(super) fn select(&mut self, index: usize) -> anyhow::Result<()> {
        anyhow::ensure!(
            index < self.sessions.len(),
            "Terminal session index is out of range"
        );
        self.active = index;
        self.open = true;
        Ok(())
    }

    pub(super) fn send_command(&mut self, command: &str) -> anyhow::Result<()> {
        anyhow::ensure!(command.len() <= 4096, "Terminal command is too long");
        anyhow::ensure!(
            !command.contains('\n') && !command.contains('\r'),
            "Send one terminal command at a time"
        );
        let session = self
            .sessions
            .get_mut(self.active)
            .map(|tab| &mut tab.session)
            .ok_or_else(|| anyhow::anyhow!("Open the terminal before sending a command"))?;
        let mut bytes = command.as_bytes().to_vec();
        bytes.push(b'\n');
        session.send(&bytes)
    }

    pub(super) fn toggle(&mut self, directory: &std::path::Path) {
        if self.open {
            self.open = false;
            return;
        }
        if self.sessions.is_empty() {
            self.new_session(directory);
            return;
        }
        self.open = true;
    }

    pub(super) fn state(&self) -> serde_json::Value {
        let snapshot = self
            .sessions
            .get(self.active)
            .map(|tab| tab.session.snapshot());
        serde_json::json!({
            "open": self.open,
            "running": !self.sessions.is_empty(),
            "active": self.active,
            "sessions": self.sessions.iter().enumerate().map(|(index, tab)| serde_json::json!({"id":tab.id,"label":tab.label,"pid":tab.session.process_id(),"active":index==self.active})).collect::<Vec<_>>(),
            "pid": self.sessions.get(self.active).and_then(|tab| tab.session.process_id()),
            "rows": snapshot.as_ref().map(|screen| screen.rows),
            "cols": snapshot.as_ref().map(|screen| screen.cols),
            "screen": snapshot.map(|screen| {
                let mut tail: String = screen.text.chars().rev().take(32768).collect();
                if screen.text.chars().count() > 32768 { tail.push('…'); }
                tail.chars().rev().collect::<String>()
            }),
            "error": self.error,
        })
    }

    pub(super) fn render(&mut self, ctx: &egui::Context, current_path: &std::path::Path) {
        if !self.open {
            return;
        }
        let mut open = self.open;
        egui::Window::new("FYLE // LOCAL SHELL")
            .id(egui::Id::new("fyle_embedded_terminal"))
            .default_size(egui::vec2(720.0, 420.0))
            .min_size(egui::vec2(420.0, 220.0))
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("PTY · LOCAL PROCESS")
                            .monospace()
                            .color(crate::theme::colors::CYBER_GREEN),
                    );
                    if let Some(pid) = self
                        .sessions
                        .get(self.active)
                        .and_then(|tab| tab.session.process_id())
                    {
                        ui.label(
                            egui::RichText::new(format!("PID {pid}"))
                                .monospace()
                                .color(crate::theme::colors::ELECTRIC_BLUE),
                        );
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("End session").clicked() {
                            self.close_active();
                        }
                    });
                });
                ui.horizontal(|ui| {
                    let mut selected = None;
                    for (index, tab) in self.sessions.iter().enumerate() {
                        let label = format!("{} · {}", tab.label, tab.id);
                        let response = ui.selectable_label(index == self.active, &label);
                        crate::ui::expose_control(
                            ui,
                            &format!("terminal_tab:{index}"),
                            &label,
                            &response,
                        );
                        if response.clicked() {
                            selected = Some(index);
                        }
                    }
                    if let Some(index) = selected {
                        self.active = index;
                    }
                    if ui.button("+ Shell").clicked() {
                        self.new_session(current_path);
                    }
                });
                ui.separator();
                let available = (ui.available_height() - 36.0).max(80.0);
                if let Some(session) = self
                    .sessions
                    .get_mut(self.active)
                    .map(|tab| &mut tab.session)
                {
                    let rows = (available / 17.0).round() as u16;
                    let cols = (ui.available_width() / 8.0).round() as u16;
                    if let Err(error) = session.resize(rows, cols) {
                        self.error = Some(format!("Could not resize PTY: {error}"));
                    }
                }
                let (screen_rect, response) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), available),
                    egui::Sense::click().union(egui::Sense::focusable_noninteractive()),
                );
                let snapshot = self
                    .sessions
                    .get(self.active)
                    .map(|tab| tab.session.snapshot());
                let painter = ui.painter().with_clip_rect(screen_rect);
                painter.rect_filled(screen_rect, 2.0, egui::Color32::from_rgb(4, 10, 16));
                if let Some(snapshot) = &snapshot {
                    let cell_width = 8.0;
                    let cell_height = 17.0;
                    for run in &snapshot.runs {
                        let position = screen_rect.min
                            + egui::vec2(run.col as f32 * cell_width, run.row as f32 * cell_height);
                        if let Some([r, g, b]) = run.background {
                            painter.rect_filled(
                                egui::Rect::from_min_size(
                                    position,
                                    egui::vec2(run.width as f32 * cell_width, cell_height),
                                ),
                                0.0,
                                egui::Color32::from_rgb(r, g, b),
                            );
                        }
                        let [r, g, b] = run.foreground;
                        painter.text(
                            position,
                            egui::Align2::LEFT_TOP,
                            &run.text,
                            egui::FontId::monospace(if run.bold { 13.5 } else { 13.0 }),
                            egui::Color32::from_rgb(r, g, b),
                        );
                    }
                    let (row, col) = snapshot.cursor;
                    if row < snapshot.rows && col < snapshot.cols {
                        let position = screen_rect.min
                            + egui::vec2(col as f32 * cell_width, row as f32 * cell_height);
                        painter.rect_stroke(
                            egui::Rect::from_min_size(
                                position,
                                egui::vec2(cell_width, cell_height),
                            ),
                            0.0,
                            egui::Stroke::new(1.0_f32, crate::theme::colors::CYBER_GREEN),
                        );
                    }
                }
                crate::ui::expose_control(ui, "terminal_screen", "Terminal screen", &response);
                if response.clicked() {
                    response.request_focus();
                }
                let focused = response.has_focus();
                if focused {
                    let events = ui.input(|input| input.events.clone());
                    let mut bytes = Vec::new();
                    let mut scroll_lines = 0;
                    for event in events {
                        match event {
                            Event::Text(text) => bytes.extend_from_slice(text.as_bytes()),
                            Event::Paste(text) => bytes.extend_from_slice(text.as_bytes()),
                            Event::Key {
                                key,
                                pressed: true,
                                modifiers,
                                ..
                            } => {
                                bytes.extend_from_slice(&encode_key(key, modifiers));
                            }
                            Event::Scroll(delta) => scroll_lines -= (delta.y / 16.0).round() as i32,
                            _ => {}
                        }
                    }
                    if let Some(session) = self
                        .sessions
                        .get_mut(self.active)
                        .map(|tab| &mut tab.session)
                    {
                        if !bytes.is_empty() {
                            if let Err(error) = session.send(&bytes) {
                                self.error = Some(error.to_string());
                            }
                        }
                        if scroll_lines != 0 {
                            session.scroll(scroll_lines);
                        }
                    }
                }
                ui.label(
                    egui::RichText::new(if focused {
                        "PTY input focused · click elsewhere to release"
                    } else {
                        "Click terminal output to send keys · Ctrl+C interrupts"
                    })
                    .small()
                    .color(crate::theme::colors::MUTED),
                );
                if let Some(error) = &self.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
            });
        self.open = open;
        if !self.sessions.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
    }
}

fn encode_key(key: egui::Key, modifiers: egui::Modifiers) -> Vec<u8> {
    if modifiers.ctrl || modifiers.command {
        let name = format!("{key:?}");
        if name.len() == 1 {
            let value = name.as_bytes()[0].to_ascii_uppercase();
            if value.is_ascii_uppercase() {
                return vec![value - b'A' + 1];
            }
        }
        return match key {
            egui::Key::Space => vec![0],
            _ => Vec::new(),
        };
    }
    let sequence: &[u8] = match key {
        egui::Key::Enter => b"\r",
        egui::Key::Tab => b"\t",
        egui::Key::Backspace => b"\x7f",
        egui::Key::Escape => b"\x1b",
        egui::Key::ArrowUp => b"\x1b[A",
        egui::Key::ArrowDown => b"\x1b[B",
        egui::Key::ArrowRight => b"\x1b[C",
        egui::Key::ArrowLeft => b"\x1b[D",
        egui::Key::Home => b"\x1b[H",
        egui::Key::End => b"\x1b[F",
        egui::Key::Insert => b"\x1b[2~",
        egui::Key::Delete => b"\x1b[3~",
        egui::Key::PageUp => b"\x1b[5~",
        egui::Key::PageDown => b"\x1b[6~",
        _ => return Vec::new(),
    };
    if modifiers.alt {
        [b"\x1b".as_slice(), sequence].concat()
    } else {
        sequence.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn terminal_panel_keeps_shell_output_after_being_hidden() {
        let dir = tempfile::tempdir().unwrap();
        let mut panel = TerminalPanel::default();
        panel.toggle(dir.path());
        panel.send_command("printf FYLE_PANEL_OK").unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if panel.state()["screen"]
                .as_str()
                .unwrap_or_default()
                .contains("FYLE_PANEL_OK")
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(panel.state()["screen"]
            .as_str()
            .unwrap()
            .contains("FYLE_PANEL_OK"));
        panel.toggle(dir.path());
        assert!(!panel.is_open());
        assert!(panel.state()["running"].as_bool().unwrap());
        panel.close();
        assert!(!panel.state()["running"].as_bool().unwrap());
    }

    #[test]
    fn terminal_tabs_keep_independent_live_processes_and_screen_buffers() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let mut panel = TerminalPanel::default();
        panel.new_session(first.path());
        panel.new_session(second.path());
        assert_eq!(panel.sessions.len(), 2);
        panel.send_command("printf SECOND_SESSION_OK").unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if panel.state()["screen"]
                .as_str()
                .unwrap_or_default()
                .contains("SECOND_SESSION_OK")
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(panel.state()["screen"]
            .as_str()
            .unwrap()
            .contains("SECOND_SESSION_OK"));
        panel.active = 0;
        panel.send_command("printf FIRST_SESSION_OK").unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if panel.state()["screen"]
                .as_str()
                .unwrap_or_default()
                .contains("FIRST_SESSION_OK")
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(panel.state()["screen"]
            .as_str()
            .unwrap()
            .contains("FIRST_SESSION_OK"));
        panel.toggle(first.path());
        assert_eq!(panel.state()["sessions"].as_array().unwrap().len(), 2);
        panel.close_active();
        assert_eq!(panel.state()["sessions"].as_array().unwrap().len(), 1);
        assert!(panel.state()["running"].as_bool().unwrap());
    }

    #[test]
    fn harness_commands_reject_multiline_and_oversized_input() {
        let mut panel = TerminalPanel::default();
        assert!(panel.send_command("one\ntwo").is_err());
        assert!(panel.send_command(&"x".repeat(4097)).is_err());
    }

    #[test]
    fn terminal_key_encoding_covers_shell_interrupt_and_tui_navigation() {
        assert_eq!(encode_key(egui::Key::C, egui::Modifiers::CTRL), [3]);
        assert_eq!(
            encode_key(egui::Key::ArrowUp, egui::Modifiers::default()),
            b"\x1b[A"
        );
        assert_eq!(
            encode_key(egui::Key::Enter, egui::Modifiers::default()),
            b"\r"
        );
    }
}
