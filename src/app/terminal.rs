//! GUI ownership and rendering for the embedded shell session.
use super::*;
use eframe::egui::Event;
use std::process::{Command, Stdio};

#[derive(Default)]
pub(super) struct TerminalPanel {
    sessions: Vec<TerminalTab>,
    active: usize,
    split: Option<(usize, usize)>,
    next_id: u32,
    open: bool,
    error: Option<String>,
}

struct TerminalTab {
    id: u32,
    label: String,
    launch_directory: PathBuf,
    session: fyle::core::terminal::TerminalSession,
    selection: Option<TerminalSelection>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TerminalSelection {
    anchor: (u16, u16),
    extent: (u16, u16),
}

impl TerminalPanel {
    pub(super) fn is_open(&self) -> bool {
        self.open
    }

    pub(super) fn close(&mut self) {
        self.open = false;
        self.sessions.clear();
        self.active = 0;
        self.split = None;
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
                    launch_directory: directory.to_path_buf(),
                    session,
                    selection: None,
                });
                self.active = self.sessions.len() - 1;
                self.open = true;
                self.error = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    pub(super) fn close_active(&mut self) {
        let removed = self.active;
        if self.active < self.sessions.len() {
            self.sessions.remove(self.active);
        }
        if let Some((first, second)) = self.split {
            let adjust = |index: usize| {
                if index == removed {
                    None
                } else {
                    Some(if index > removed { index - 1 } else { index })
                }
            };
            self.split = adjust(first).zip(adjust(second));
        }
        self.active = self.active.min(self.sessions.len().saturating_sub(1));
        if self.split.is_some_and(|(first, second)| first == second) {
            self.split = None;
        }
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
        if self
            .split
            .is_some_and(|(first, second)| index != first && index != second)
        {
            self.split = None;
        }
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

    pub(super) fn toggle_split(&mut self, directory: &std::path::Path) {
        if self.split.is_some() {
            self.split = None;
            return;
        }
        if self.sessions.is_empty() {
            self.new_session(directory);
        }
        if self.sessions.len() == 1 {
            self.new_session(directory);
        }
        if self.sessions.len() >= 2 {
            let first = self.active.min(self.sessions.len() - 1);
            let second = (first + 1) % self.sessions.len();
            self.split = Some((first, second));
            self.active = second;
            self.open = true;
        }
    }

    pub(super) fn state(&self) -> serde_json::Value {
        let snapshot = self
            .sessions
            .get(self.active)
            .map(|tab| tab.session.snapshot());
        serde_json::json!({
            "open": self.open,
            "split": self.split.is_some(),
            "running": !self.sessions.is_empty(),
            "active": self.active,
            "sessions": self.sessions.iter().enumerate().map(|(index, tab)| serde_json::json!({"id":tab.id,"label":tab.label,"launch_directory":tab.launch_directory,"pid":tab.session.process_id(),"active":index==self.active})).collect::<Vec<_>>(),
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

    pub(super) fn open_external(&mut self, directory: &std::path::Path) {
        match launch_external_terminal(directory) {
            Ok(name) => {
                log::info!("Opened terminal integration {name}");
                self.error = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    pub(super) fn render(
        &mut self,
        ctx: &egui::Context,
        current_path: &std::path::Path,
        appearance: &fyle::appearance::Appearance,
    ) {
        if !self.open {
            return;
        }
        let mut open = self.open;
        egui::Window::new("Terminal")
            .id(egui::Id::new("fyle_embedded_terminal"))
            .default_size(egui::vec2(1080.0, 680.0))
            .min_size(egui::vec2(760.0, 460.0))
            .resizable(true)
            .open(&mut open)
            .show(ctx, |ui| {
                self.render_contents(ui, ctx, current_path, appearance)
            });
        self.open = open;
        if !self.sessions.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
    }

    fn render_contents(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        current_path: &std::path::Path,
        appearance: &fyle::appearance::Appearance,
    ) {
        let mut selected = None;
        let mut split_clicked = false;
        let mut close_all_clicked = false;
        let mut close_active_clicked = false;
        let mut external_clicked = false;
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("TERMINAL")
                    .monospace()
                    .strong()
                    .color(crate::theme::colors::CYBER_GREEN),
            );
            if let Some(tab) = self.sessions.get(self.active) {
                ui.label(
                    egui::RichText::new(tab.launch_directory.display().to_string())
                        .small()
                        .monospace()
                        .color(crate::theme::colors::MUTED),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let close = ui.small_button("×").on_hover_text("Close all sessions");
                crate::ui::expose_control(ui, "terminal_close", "Close terminal", &close);
                close_all_clicked = close.clicked();
                let split = ui.button(if self.split.is_some() {
                    "Unsplit"
                } else {
                    "Split right"
                });
                crate::ui::expose_control(ui, "terminal_split", "Toggle split pane", &split);
                split_clicked = split.clicked();
                let new = ui.button("+ Shell");
                crate::ui::expose_control(ui, "terminal_new", "New shell", &new);
                if new.clicked() {
                    self.new_session(current_path);
                }
                let end = ui.small_button("End session");
                crate::ui::expose_control(ui, "terminal_close_active", "End active session", &end);
                if end.clicked() {
                    close_active_clicked = true;
                }
                if appearance.allow_external_terminals && external_terminal_name().is_some() {
                    let external = ui.button("Open in external");
                    crate::ui::expose_control(
                        ui,
                        "terminal_external",
                        "Open in Foot or cmux",
                        &external,
                    );
                    external_clicked = external.clicked();
                }
            });
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(156.0, ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.label(
                        egui::RichText::new("SESSIONS")
                            .monospace()
                            .size(10.0)
                            .color(crate::theme::colors::ELECTRIC_BLUE),
                    );
                    for (index, tab) in self.sessions.iter().enumerate() {
                        let response = ui
                            .selectable_label(
                                index == self.active,
                                format!("{}  ·  {}", tab.label, tab.id),
                            )
                            .on_hover_text(tab.launch_directory.display().to_string());
                        crate::ui::expose_control(
                            ui,
                            &format!("terminal_tab:{index}"),
                            &tab.label,
                            &response,
                        );
                        if response.clicked() {
                            selected = Some(index);
                        }
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(tab.launch_directory.display().to_string())
                                    .small()
                                    .color(crate::theme::colors::MUTED),
                            )
                            .truncate(true),
                        );
                        ui.add_space(4.0);
                    }
                    if self.sessions.is_empty() {
                        ui.small("No running sessions");
                    }
                },
            );
            ui.separator();
            let pane_height = ui.available_height().max(80.0);
            let pane_indices = self.split.unwrap_or((self.active, usize::MAX));
            if pane_indices.1 != usize::MAX {
                ui.columns(2, |columns| {
                    for (slot, (column, index)) in columns
                        .iter_mut()
                        .zip([pane_indices.0, pane_indices.1])
                        .enumerate()
                    {
                        if let Some(tab) = self.sessions.get_mut(index) {
                            let clicked = render_terminal_surface(
                                column,
                                ctx,
                                tab,
                                index == self.active,
                                pane_height,
                                appearance,
                                if slot == 0 {
                                    "terminal_screen"
                                } else {
                                    "terminal_screen:1"
                                },
                            );
                            if clicked {
                                selected = Some(index);
                            }
                        }
                    }
                });
            } else if let Some(tab) = self.sessions.get_mut(self.active) {
                if render_terminal_surface(
                    ui,
                    ctx,
                    tab,
                    true,
                    pane_height,
                    appearance,
                    "terminal_screen",
                ) {
                    selected = Some(self.active);
                }
            }
        });
        if let Some(index) = selected {
            self.active = index;
            if self
                .split
                .is_some_and(|(first, second)| index != first && index != second)
            {
                self.split = None;
            }
        }
        if split_clicked {
            self.toggle_split(current_path);
        }
        if external_clicked {
            let directory = self
                .sessions
                .get(self.active)
                .map(|tab| tab.launch_directory.clone())
                .unwrap_or_else(|| current_path.to_path_buf());
            self.open_external(&directory);
        }
        if close_all_clicked {
            self.close();
        } else if close_active_clicked {
            self.close_active();
        }
        if self.sessions.is_empty() {
            self.open = false;
        }
        if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
    }
}

fn terminal_colors(theme: fyle::appearance::TerminalTheme) -> (egui::Color32, egui::Color32) {
    match theme {
        fyle::appearance::TerminalTheme::FyleMidnight => (
            egui::Color32::from_rgb(8, 16, 24),
            egui::Color32::from_rgb(0, 212, 255),
        ),
        fyle::appearance::TerminalTheme::Nord => (
            egui::Color32::from_rgb(46, 52, 64),
            egui::Color32::from_rgb(136, 192, 208),
        ),
        fyle::appearance::TerminalTheme::SolarizedDark => (
            egui::Color32::from_rgb(0, 43, 54),
            egui::Color32::from_rgb(42, 161, 152),
        ),
    }
}

fn render_terminal_surface(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    tab: &mut TerminalTab,
    active: bool,
    pane_height: f32,
    appearance: &fyle::appearance::Appearance,
    control_id: &str,
) -> bool {
    let (background, accent) = terminal_colors(appearance.terminal_theme);
    let border = if active {
        accent
    } else {
        egui::Color32::from_rgb(56, 76, 91)
    };
    let mut surface_clicked = false;
    egui::Frame::none()
        .fill(background)
        .stroke(egui::Stroke::new(
            if active { 1.4_f32 } else { 1.0_f32 },
            border,
        ))
        .rounding(egui::Rounding::same(6.0))
        .inner_margin(egui::Margin::same(8.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&tab.label)
                        .monospace()
                        .strong()
                        .color(accent),
                );
                ui.label(
                    egui::RichText::new(tab.launch_directory.display().to_string())
                        .small()
                        .monospace()
                        .color(crate::theme::colors::MUTED),
                );
                if let Some(pid) = tab.session.process_id() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(format!("PID {pid}"))
                                .small()
                                .monospace()
                                .color(crate::theme::colors::MUTED),
                        );
                    });
                }
            });
            ui.add_space(5.0);
            let font_family = match appearance.terminal_font {
                fyle::appearance::TerminalFont::JetBrainsMonoNerd => {
                    egui::FontFamily::Name(super::TERMINAL_FONT_FAMILY.into())
                }
                fyle::appearance::TerminalFont::SystemMonospace => {
                    egui::FontFamily::Name(super::SYSTEM_TERMINAL_FONT_FAMILY.into())
                }
            };
            let font_id = egui::FontId::new(appearance.terminal_font_size, font_family);
            let cell_width = ctx.fonts(|fonts| fonts.glyph_width(&font_id, 'M')).max(1.0);
            let cell_height = (ctx.fonts(|fonts| fonts.row_height(&font_id))
                * appearance.terminal_line_spacing)
                .max(1.0);
            let available_height = (ui.available_height().min(pane_height - 45.0)).max(60.0);
            let available_width = ui.available_width().max(cell_width * 20.0);
            let rows = (available_height / cell_height).floor().clamp(4.0, 200.0) as u16;
            let cols = (available_width / cell_width).floor().clamp(20.0, 400.0) as u16;
            if let Err(error) = tab.session.resize(rows, cols) {
                log::warn!("Could not resize terminal PTY: {error}");
            }
            let (screen_rect, response) = ui.allocate_exact_size(
                egui::vec2(available_width, available_height),
                egui::Sense::click_and_drag().union(egui::Sense::focusable_noninteractive()),
            );
            crate::ui::expose_control(ui, control_id, "Terminal screen", &response);
            if response.clicked() {
                response.request_focus();
            }
            if response.drag_started() || response.dragged() {
                if let Some(pointer) = ctx.pointer_interact_pos() {
                    let point =
                        terminal_point(pointer, screen_rect, cell_width, cell_height, rows, cols);
                    if response.drag_started() || tab.selection.is_none() {
                        tab.selection = Some(TerminalSelection {
                            anchor: point,
                            extent: point,
                        });
                    } else if let Some(selection) = &mut tab.selection {
                        selection.extent = point;
                    }
                }
            }
            surface_clicked = response.clicked();
            let focused = response.has_focus();
            let painter = ui.painter().with_clip_rect(screen_rect);
            painter.rect_filled(screen_rect, 3.0, background);
            let snapshot = tab.session.snapshot();
            if let Some(selection) = tab.selection {
                let (start, end) = ordered_selection(selection);
                for row in start.0..=end.0 {
                    let left = if row == start.0 { start.1 } else { 0 };
                    let right = if row == end.0 {
                        end.1.saturating_add(1)
                    } else {
                        snapshot.cols
                    };
                    painter.rect_filled(
                        egui::Rect::from_min_size(
                            screen_rect.min
                                + egui::vec2(left as f32 * cell_width, row as f32 * cell_height),
                            egui::vec2(right.saturating_sub(left) as f32 * cell_width, cell_height),
                        ),
                        0.0,
                        accent.linear_multiply(0.34),
                    );
                }
            }
            for run in &snapshot.runs {
                let position = screen_rect.min
                    + egui::vec2(run.col as f32 * cell_width, run.row as f32 * cell_height);
                let run_rect = egui::Rect::from_min_size(
                    position,
                    egui::vec2(run.width as f32 * cell_width, cell_height),
                );
                if let Some([r, g, b]) = run.background {
                    painter.rect_filled(run_rect, 0.0, egui::Color32::from_rgb(r, g, b));
                }
                let [r, g, b] = run.foreground;
                let mut job = egui::text::LayoutJob::default();
                let mut format =
                    egui::TextFormat::simple(font_id.clone(), egui::Color32::from_rgb(r, g, b));
                format.italics = run.italic;
                if run.bold {
                    format.font_id.size += 0.25;
                }
                job.append(&run.text, 0.0, format);
                let galley = painter.layout_job(job);
                painter.galley(position, galley, egui::Color32::from_rgb(r, g, b));
            }
            let (row, col) = snapshot.cursor;
            if row < snapshot.rows && col < snapshot.cols {
                let position =
                    screen_rect.min + egui::vec2(col as f32 * cell_width, row as f32 * cell_height);
                let cursor_rect =
                    egui::Rect::from_min_size(position, egui::vec2(cell_width, cell_height));
                match appearance.terminal_cursor {
                    fyle::appearance::TerminalCursor::Block => {
                        painter.rect_stroke(cursor_rect, 0.0, egui::Stroke::new(1.4_f32, accent));
                    }
                    fyle::appearance::TerminalCursor::Bar => {
                        painter.rect_filled(
                            egui::Rect::from_min_size(position, egui::vec2(2.0, cell_height)),
                            0.0,
                            accent,
                        );
                    }
                    fyle::appearance::TerminalCursor::Underline => {
                        painter.rect_filled(
                            egui::Rect::from_min_size(
                                egui::pos2(position.x, position.y + cell_height - 2.0),
                                egui::vec2(cell_width, 2.0),
                            ),
                            0.0,
                            accent,
                        );
                    }
                }
            }
            if focused {
                let events = ui.input(|input| input.events.clone());
                let mut bytes = Vec::new();
                let mut scroll_lines = 0;
                for event in events {
                    match event {
                        Event::Text(text) | Event::Paste(text) => {
                            bytes.extend_from_slice(text.as_bytes());
                        }
                        Event::Copy => {
                            if let Some(selection) = tab.selection {
                                ctx.output_mut(|output| {
                                    output.copied_text = selection_text(&snapshot, selection);
                                });
                            }
                        }
                        Event::Key {
                            key,
                            pressed: true,
                            modifiers,
                            ..
                        } => bytes.extend_from_slice(&encode_key(key, modifiers)),
                        Event::Scroll(delta) => {
                            scroll_lines -= (delta.y / cell_height).round() as i32;
                        }
                        _ => {}
                    }
                }
                if !bytes.is_empty() {
                    if let Err(error) = tab.session.send(&bytes) {
                        log::warn!("Could not send terminal input: {error}");
                    }
                }
                if scroll_lines != 0 {
                    tab.session.scroll(scroll_lines);
                }
            }
            ui.add_space(3.0);
            ui.label(
                egui::RichText::new(if focused {
                    "INPUT FOCUSED · Ctrl+C interrupts"
                } else {
                    "Click output to focus · paste supported"
                })
                .small()
                .color(crate::theme::colors::MUTED),
            );
        });
    surface_clicked
}

fn terminal_point(
    pointer: egui::Pos2,
    screen: egui::Rect,
    cell_width: f32,
    cell_height: f32,
    rows: u16,
    cols: u16,
) -> (u16, u16) {
    let col = ((pointer.x - screen.min.x) / cell_width).floor().max(0.0) as u16;
    let row = ((pointer.y - screen.min.y) / cell_height).floor().max(0.0) as u16;
    (
        row.min(rows.saturating_sub(1)),
        col.min(cols.saturating_sub(1)),
    )
}

fn ordered_selection(selection: TerminalSelection) -> ((u16, u16), (u16, u16)) {
    if selection.anchor <= selection.extent {
        (selection.anchor, selection.extent)
    } else {
        (selection.extent, selection.anchor)
    }
}

fn selection_text(
    snapshot: &fyle::core::terminal::TerminalSnapshot,
    selection: TerminalSelection,
) -> String {
    let (start, end) = ordered_selection(selection);
    let lines: Vec<_> = snapshot.text.lines().collect();
    (start.0..=end.0)
        .map(|row| {
            let line = lines.get(row as usize).copied().unwrap_or_default();
            let left = (if row == start.0 { start.1 } else { 0 }) as usize;
            let right = (if row == end.0 {
                end.1.saturating_add(1)
            } else {
                snapshot.cols
            }) as usize;
            line.chars()
                .skip(left)
                .take(right.saturating_sub(left))
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Clone, Copy)]
enum ExternalTerminal {
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Foot,
    Cmux,
}

pub(super) fn external_terminal_name() -> Option<&'static str> {
    #[cfg(target_os = "macos")]
    if find_executable("cmux").is_some() {
        return Some("cmux");
    }
    #[cfg(target_os = "linux")]
    if find_executable("foot").is_some() {
        return Some("Foot");
    }
    None
}

fn find_executable(name: &str) -> Option<std::path::PathBuf> {
    let mut directories = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    #[cfg(target_os = "macos")]
    directories.extend([
        std::path::PathBuf::from("/opt/homebrew/bin"),
        std::path::PathBuf::from("/usr/local/bin"),
    ]);
    directories
        .into_iter()
        .map(|directory| directory.join(name))
        .find(|path| {
            std::fs::metadata(path)
                .map(|metadata| {
                    metadata.is_file() && {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            metadata.permissions().mode() & 0o111 != 0
                        }
                        #[cfg(not(unix))]
                        {
                            true
                        }
                    }
                })
                .unwrap_or(false)
        })
}

fn external_terminal_command(
    terminal: ExternalTerminal,
    executable: &std::path::Path,
    directory: &std::path::Path,
) -> Command {
    match terminal {
        ExternalTerminal::Foot => {
            let mut command = Command::new(executable);
            let mut argument = std::ffi::OsString::from("--working-directory=");
            argument.push(directory.as_os_str());
            command.arg(argument);
            command
        }
        ExternalTerminal::Cmux => {
            let mut command = Command::new(executable);
            command
                .arg("new-workspace")
                .arg("--name")
                .arg("Fyle")
                .arg("--cwd")
                .arg(directory);
            command
        }
    }
}

fn launch_external_terminal(directory: &std::path::Path) -> anyhow::Result<&'static str> {
    anyhow::ensure!(directory.is_dir(), "Terminal location is not a directory");
    #[cfg(target_os = "macos")]
    let (terminal, name) = (ExternalTerminal::Cmux, "cmux");
    #[cfg(target_os = "linux")]
    let (terminal, name) = (ExternalTerminal::Foot, "Foot");
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    anyhow::bail!("Foot/cmux integration is not supported on this platform");
    let executable = find_executable(name);
    anyhow::ensure!(
        executable.is_some(),
        "{} CLI is unavailable; continue with Fyle’s built-in terminal",
        name
    );
    external_terminal_command(terminal, executable.as_deref().unwrap(), directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(name)
}

fn encode_key(key: egui::Key, modifiers: egui::Modifiers) -> Vec<u8> {
    if modifiers.command && !modifiers.ctrl {
        return Vec::new();
    }
    if modifiers.ctrl {
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
    fn split_view_creates_two_sessions_and_closing_a_pane_repairs_the_layout() {
        let dir = tempfile::tempdir().unwrap();
        let mut panel = TerminalPanel::default();
        panel.toggle_split(dir.path());
        assert_eq!(panel.sessions.len(), 2);
        assert!(panel.state()["split"].as_bool().unwrap());
        panel.close_active();
        assert_eq!(panel.sessions.len(), 1);
        assert!(!panel.state()["split"].as_bool().unwrap());
        panel.toggle_split(dir.path());
        assert!(panel.state()["split"].as_bool().unwrap());
        panel.toggle_split(dir.path());
        assert!(!panel.state()["split"].as_bool().unwrap());
    }

    #[test]
    fn terminal_selection_copies_only_the_selected_text_range() {
        let snapshot = fyle::core::terminal::TerminalSnapshot {
            text: "hello world\nsecond line".into(),
            rows: 2,
            cols: 20,
            runs: Vec::new(),
            cursor: (0, 0),
        };
        let selection = TerminalSelection {
            anchor: (1, 2),
            extent: (0, 1),
        };
        assert_eq!(selection_text(&snapshot, selection), "ello world\nsec");
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
        assert!(encode_key(egui::Key::C, egui::Modifiers::COMMAND).is_empty());
        assert_eq!(encode_key(egui::Key::C, egui::Modifiers::CTRL), [3]);
    }

    #[test]
    fn external_terminal_arguments_keep_directory_as_a_single_safe_argument() {
        let directory = std::path::Path::new("/tmp/Folder with spaces");
        let foot = external_terminal_command(
            ExternalTerminal::Foot,
            std::path::Path::new("foot"),
            directory,
        );
        assert_eq!(
            foot.get_args()
                .map(|arg| arg.to_string_lossy())
                .collect::<Vec<_>>(),
            ["--working-directory=/tmp/Folder with spaces"]
        );
        let cmux = external_terminal_command(
            ExternalTerminal::Cmux,
            std::path::Path::new("cmux"),
            directory,
        );
        let args = cmux
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            args,
            [
                "new-workspace",
                "--name",
                "Fyle",
                "--cwd",
                "/tmp/Folder with spaces"
            ]
        );
    }
}
