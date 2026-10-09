//! The install runs as a child process on a pseudo-terminal. Its output is kept
//! on a vt100 screen drawn inside the TUI, and its progress comes from the
//! terminal title it sets ("2lazy4arch <n>/<total> <task>", see installer::step).

use std::{
    io::{Read, Write},
    sync::mpsc::{self, Receiver},
    thread,
    time::Instant,
};

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
};

/// Keeps the last window title the child set.
#[derive(Default)]
pub struct Title(pub String);

impl vt100::Callbacks for Title {
    fn set_window_title(&mut self, _: &mut vt100::Screen, title: &[u8]) {
        self.0 = String::from_utf8_lossy(title).to_string();
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Status {
    Running,
    /// true when it exited successfully
    Finished(bool),
}

pub struct Install {
    pub parser: vt100::Parser<Title>,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    output: Receiver<Vec<u8>>,
    exit: Receiver<bool>,
    pub status: Status,
    pub started: Instant,
    pub finished: Option<Instant>,
}

/// Lines kept above the screen for pgup.
const SCROLLBACK: usize = 10_000;

impl Install {
    pub fn spawn(program: &str, args: &[&str], rows: u16, cols: u16) -> Result<Install> {
        let pty = native_pty_system().openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })?;
        let mut command = CommandBuilder::new(program);
        command.args(args);
        command.env("TERM", "xterm-256color");
        command.cwd(std::env::current_dir()?);
        let mut child = pty.slave.spawn_command(command)?;
        drop(pty.slave);

        let mut reader = pty.master.try_clone_reader()?;
        let (output_tx, output) = mpsc::channel();
        thread::spawn(move || {
            let mut buf = [0u8; 8192];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 || output_tx.send(buf[..n].to_vec()).is_err() {
                    break;
                }
            }
        });
        let (exit_tx, exit) = mpsc::channel();
        thread::spawn(move || {
            let ok = child.wait().map(|s| s.success()).unwrap_or(false);
            let _ = exit_tx.send(ok);
        });

        Ok(Install {
            parser: vt100::Parser::new_with_callbacks(rows, cols, SCROLLBACK, Title::default()),
            writer: pty.master.take_writer()?,
            master: pty.master,
            output,
            exit,
            status: Status::Running,
            started: Instant::now(),
            finished: None,
        })
    }

    /// Takes in whatever the child printed, and notices when it exits.
    pub fn pump(&mut self) {
        while let Ok(bytes) = self.output.try_recv() {
            self.parser.process(&bytes);
        }
        if let Ok(ok) = self.exit.try_recv() {
            self.status = Status::Finished(ok);
            self.finished = Some(Instant::now());
        }
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        if self.parser.screen().size() != (rows, cols) {
            self.parser.screen_mut().set_size(rows, cols);
            let _ = self.master.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 });
        }
    }

    /// Keys go to the installer (its y/N prompt, ctrl+c).
    pub fn send(&mut self, key: KeyEvent) {
        if let Some(bytes) = key_bytes(key) {
            let _ = self.writer.write_all(&bytes);
            let _ = self.writer.flush();
        }
    }

    /// Positive scrolls back into history.
    pub fn scroll(&mut self, rows: isize) {
        let screen = self.parser.screen_mut();
        let to = (screen.scrollback() as isize + rows).max(0) as usize;
        screen.set_scrollback(to);
    }

    /// (step, total, task) from the title the installer sets.
    pub fn progress(&self) -> (usize, usize, String) {
        parse_title(&self.parser.callbacks().0)
    }

    /// Seconds since the start, frozen when it finishes.
    pub fn elapsed(&self) -> u64 {
        self.finished.unwrap_or_else(Instant::now).duration_since(self.started).as_secs()
    }
}

/// "2lazy4arch 7/14 Installing the base system" -> (7, 14, "Installing the base system")
pub fn parse_title(title: &str) -> (usize, usize, String) {
    let parse = || -> Option<(usize, usize, String)> {
        let rest = title.strip_prefix("2lazy4arch ")?;
        let (count, task) = rest.split_once(' ').unwrap_or((rest, ""));
        let (step, total) = count.split_once('/')?;
        Some((step.parse().ok()?, total.parse().ok()?, task.to_string()))
    };
    parse().unwrap_or((0, 0, "Starting".into()))
}

/// What a key sends down a terminal.
fn key_bytes(key: KeyEvent) -> Option<Vec<u8>> {
    let bytes = match key.code {
        KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) && c.is_ascii_alphabetic() => {
            vec![(c.to_ascii_lowercase() as u8) - b'a' + 1]
        }
        KeyCode::Char(c) => c.to_string().into_bytes(),
        KeyCode::Enter => b"\r".to_vec(),
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Tab => b"\t".to_vec(),
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Left => b"\x1b[D".to_vec(),
        _ => return None,
    };
    Some(bytes)
}

fn color(color: vt100::Color) -> Color {
    match color {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(i) => Color::Indexed(i),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

/// Draws the vt100 screen into `area`.
pub fn draw_screen(screen: &vt100::Screen, area: Rect, buf: &mut Buffer) {
    for row in 0..area.height {
        for col in 0..area.width {
            let Some(cell) = screen.cell(row, col) else { continue };
            let target = buf.get_mut(area.x + col, area.y + row);
            let contents = cell.contents();
            target.set_symbol(if contents.is_empty() { " " } else { contents });
            let mut style = Style::new().fg(color(cell.fgcolor())).bg(color(cell.bgcolor()));
            for (on, modifier) in [(cell.bold(), Modifier::BOLD), (cell.italic(), Modifier::ITALIC), (cell.underline(), Modifier::UNDERLINED), (cell.inverse(), Modifier::REVERSED)] {
                if on {
                    style = style.add_modifier(modifier);
                }
            }
            target.set_style(style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn titles() {
        assert_eq!(parse_title("2lazy4arch 7/14 Installing the base system"), (7, 14, "Installing the base system".into()));
        assert_eq!(parse_title("bash"), (0, 0, "Starting".into()));
    }

    /// A child on a real pty: output on the screen, progress from its title, its exit status.
    #[test]
    fn runs_a_child() {
        let script = r#"printf '\033]0;2lazy4arch 2/5 Doing things\007'; printf '\033[1;32mhello\033[0m\n'; read answer; echo "got $answer"; exit 3"#;
        let mut install = Install::spawn("sh", &["-c", script], 10, 40).unwrap();
        thread::sleep(Duration::from_millis(300));
        install.send(KeyEvent::from(KeyCode::Char('y')));
        install.send(KeyEvent::from(KeyCode::Enter));
        for _ in 0..50 {
            install.pump();
            if install.status != Status::Running {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
        thread::sleep(Duration::from_millis(200));
        install.pump();
        assert_eq!(install.status, Status::Finished(false));
        assert_eq!(install.progress(), (2, 5, "Doing things".into()));
        let text = install.parser.screen().contents();
        assert!(text.contains("hello") && text.contains("got y"), "{text}");
        assert!(install.parser.screen().cell(0, 0).unwrap().bold());
    }
}
