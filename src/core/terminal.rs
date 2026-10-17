//! An app-owned PTY session and bounded VT screen model.
//!
//! The child receives its working directory through the PTY command builder;
//! Fyle's own cwd and environment are never changed. Dropping a session kills
//! its shell process and releases its PTY handles.

use anyhow::{Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::{
    io::{Read, Write},
    path::Path,
    sync::{Arc, Mutex},
    thread,
};

const ROWS: u16 = 32;
const COLS: u16 = 100;
const SCROLLBACK: usize = 2_000;
const READ_CHUNK: usize = 8192;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalSnapshot {
    pub text: String,
    pub rows: u16,
    pub cols: u16,
    pub runs: Vec<TerminalRun>,
    pub cursor: (u16, u16),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalRun {
    pub row: u16,
    pub col: u16,
    pub width: u16,
    pub text: String,
    pub foreground: [u8; 3],
    pub background: Option<[u8; 3]>,
    pub bold: bool,
}

pub struct TerminalSession {
    screen: Arc<Mutex<vt100::Parser>>,
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    rows: u16,
    cols: u16,
}

impl TerminalSession {
    pub fn spawn(directory: &Path) -> Result<Self> {
        anyhow::ensure!(directory.is_dir(), "Terminal location is not a directory");
        let shell = std::env::var_os("SHELL")
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "/bin/sh".into());
        Self::spawn_shell(directory, Path::new(&shell))
    }

    fn spawn_shell(directory: &Path, shell: &Path) -> Result<Self> {
        let system = native_pty_system();
        let pair = system.openpty(PtySize {
            rows: ROWS,
            cols: COLS,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        let mut command = CommandBuilder::new(shell);
        command.cwd(directory);
        command.env("TERM", "xterm-256color");
        let child = pair
            .slave
            .spawn_command(command)
            .context("Could not start shell")?;
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        let master = pair.master;
        let screen = Arc::new(Mutex::new(vt100::Parser::new(ROWS, COLS, SCROLLBACK)));
        let reader_screen = Arc::clone(&screen);
        thread::Builder::new()
            .name("fyle-terminal-reader".into())
            .spawn(move || {
                let mut buffer = [0; READ_CHUNK];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(length) => reader_screen
                            .lock()
                            .unwrap_or_else(|error| error.into_inner())
                            .process(&buffer[..length]),
                    }
                }
            })?;
        Ok(Self {
            screen,
            writer,
            master,
            child,
            rows: ROWS,
            cols: COLS,
        })
    }

    pub fn send(&mut self, bytes: &[u8]) -> Result<()> {
        self.writer.write_all(bytes)?;
        self.writer.flush()?;
        Ok(())
    }

    pub fn snapshot(&self) -> TerminalSnapshot {
        let screen = self
            .screen
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let active = screen.screen();
        let mut runs: Vec<TerminalRun> = Vec::new();
        for row in 0..self.rows {
            for col in 0..self.cols {
                let Some(cell) = active.cell(row, col) else {
                    continue;
                };
                if cell.is_wide_continuation() {
                    continue;
                }
                let foreground = terminal_color(cell.fgcolor()).unwrap_or([194, 224, 224]);
                let background = terminal_color(cell.bgcolor());
                let text = if cell.has_contents() {
                    cell.contents()
                } else {
                    " "
                };
                if let Some(last) = runs.last_mut().filter(|last| {
                    last.row == row
                        && last.foreground == foreground
                        && last.background == background
                        && last.bold == cell.bold()
                        && last.col + last.width == col
                }) {
                    last.text.push_str(text);
                    last.width += if cell.is_wide() { 2 } else { 1 };
                } else {
                    runs.push(TerminalRun {
                        row,
                        col,
                        width: if cell.is_wide() { 2 } else { 1 },
                        text: text.into(),
                        foreground,
                        background,
                        bold: cell.bold(),
                    });
                }
            }
        }
        TerminalSnapshot {
            text: active.contents(),
            rows: self.rows,
            cols: self.cols,
            runs,
            cursor: active.cursor_position(),
        }
    }

    pub fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
        let rows = rows.clamp(4, 200);
        let cols = cols.clamp(20, 400);
        if self.rows == rows && self.cols == cols {
            return Ok(());
        }
        self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        self.screen
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .screen_mut()
            .set_size(rows, cols);
        self.rows = rows;
        self.cols = cols;
        Ok(())
    }

    pub fn scroll(&self, lines: i32) {
        let mut parser = self
            .screen
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let screen = parser.screen_mut();
        let current = screen.scrollback() as i32;
        screen.set_scrollback((current + lines).max(0) as usize);
    }

    pub fn process_id(&self) -> Option<u32> {
        self.child.process_id()
    }
}

fn terminal_color(color: vt100::Color) -> Option<[u8; 3]> {
    match color {
        vt100::Color::Default => None,
        vt100::Color::Rgb(r, g, b) => Some([r, g, b]),
        vt100::Color::Idx(index @ 0..=15) => Some([
            [
                0, 205, 0, 205, 0, 205, 0, 229, 127, 255, 127, 255, 127, 255, 127, 255,
            ][index as usize],
            [
                0, 0, 205, 0, 0, 0, 205, 229, 127, 127, 255, 127, 127, 255, 255, 255,
            ][index as usize],
            [
                0, 0, 0, 205, 205, 205, 205, 229, 127, 127, 127, 255, 255, 127, 255, 255,
            ][index as usize],
        ]),
        vt100::Color::Idx(index @ 16..=231) => {
            let value = index - 16;
            let component = |n: u8| if n == 0 { 0 } else { 55 + n * 40 };
            Some([
                component(value / 36),
                component((value / 6) % 6),
                component(value % 6),
            ])
        }
        vt100::Color::Idx(index) => {
            let gray = 8 + (index - 232) * 10;
            Some([gray, gray, gray])
        }
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(pid) = self.master.process_group_leader() {
            // portable-pty creates a fresh session for this shell; signal only
            // that session's process group so foreground jobs do not outlive it.
            unsafe {
                libc::kill(-pid, libc::SIGHUP);
            }
        }
        let _ = self.child.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn pty_runs_a_child_local_shell_and_captures_output() {
        let dir = tempfile::tempdir().unwrap();
        let mut session = TerminalSession::spawn_shell(dir.path(), Path::new("/bin/sh")).unwrap();
        assert!(session.process_id().is_some());
        session.send(b"printf 'FYLE_PTY_OK\\n'\n").unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if session.snapshot().text.contains("FYLE_PTY_OK") {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!(
            "PTY did not capture shell output: {}",
            session.snapshot().text
        );
    }

    #[test]
    fn resize_clamps_untrusted_dimensions_to_supported_terminal_bounds() {
        let dir = tempfile::tempdir().unwrap();
        let mut session = TerminalSession::spawn_shell(dir.path(), Path::new("/bin/sh")).unwrap();
        session.resize(0, u16::MAX).unwrap();
        assert_eq!(session.master.get_size().unwrap().rows, 4);
        assert_eq!(session.master.get_size().unwrap().cols, 400);
    }

    #[test]
    fn vt_screen_snapshot_preserves_ansi_rgb_runs_and_cursor_position() {
        let dir = tempfile::tempdir().unwrap();
        let mut session = TerminalSession::spawn_shell(dir.path(), Path::new("/bin/sh")).unwrap();
        session
            .send(b"printf '\\033[38;2;1;2;3mFYLE_COLOR_OK\\033[0m'\n")
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let snapshot = loop {
            let snapshot = session.snapshot();
            if snapshot
                .runs
                .iter()
                .any(|run| run.text.contains("FYLE_COLOR_OK") && run.foreground == [1, 2, 3])
            {
                break snapshot;
            }
            assert!(
                Instant::now() < deadline,
                "PTY did not capture ANSI output: {:?}; runs={:?}",
                snapshot.text,
                snapshot
                    .runs
                    .iter()
                    .filter(|run| run.text.contains("FYLE_COLOR_OK"))
                    .collect::<Vec<_>>()
            );
            thread::sleep(Duration::from_millis(10));
        };
        assert!(snapshot
            .runs
            .iter()
            .any(|run| run.text.contains("FYLE_COLOR_OK") && run.foreground == [1, 2, 3]));
        assert!(snapshot.cursor.1 > 0);
    }

    #[test]
    fn invalid_working_directory_is_rejected_before_spawning() {
        assert!(TerminalSession::spawn(Path::new("/path/that/does/not/exist")).is_err());
    }
}
