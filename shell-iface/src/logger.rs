use std::{fs::OpenOptions, io::Write};

/// The install log. Commands' output reaches it through a tee (installer::utils::tee_output).
pub const LOG_FILE: &str = "/var/log/2lazy4arch.log";

#[derive(Default, Debug)]
pub struct Logger {
    is_debug: bool,
}

impl Logger {
    pub fn new(is_debug: bool) -> Logger {
        Logger { is_debug }
    }

    /// Always goes to LOG_FILE (when writable); printed only when is_debug
    /// (stray prints would garble the TUI).
    pub fn debug(&self, origin: &str, msg: &str) {
        let content = format!("{}: {}", origin.to_uppercase(), crate::redact(msg));
        if self.is_debug {
            println!("{}", content);
        }
        let _ = append_to_file(LOG_FILE, &content);
    }
}

/// Opens a file, appends the content.
/// Creates the file if the file does not exist.
/// Adds a newline before appending just to be sure.
pub fn append_to_file(path: &str, content: &str) -> Result<(), String> {
    let mut file = OpenOptions::new().append(true).create(true).open(path).map_err(|e| e.to_string())?;
    file.write_all(format!("\n{}", content).as_bytes()).map_err(|e| e.to_string())
}
