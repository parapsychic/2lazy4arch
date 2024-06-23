use std::{
    fs::OpenOptions,
    io::Write,
};

#[derive(Default, Debug)]
pub struct Logger {
    is_debug: bool,
}

impl Logger {
    pub fn new(is_debug: bool) -> Logger{
        Logger { is_debug}
    }

    pub fn debug(&self, origin: &str, msg: &str) {
        if self.is_debug {
            eprintln!("{}: {}", origin.to_uppercase(), msg);
        }

        let content = format!("{}: {}", origin.to_uppercase(), msg);
        println!("{}", content);
        let _ = append_to_file("shell_log.txt", &content);
    }
}


/// Opens a file, appends the content.
/// Creates the file if the file does not exist.
/// Adds a newline before appending just to be sure.
pub fn append_to_file(path: &str, content: &str) -> Result<(), String> {
    let mut file = match OpenOptions::new().append(true).create(true).open(path) {
        Ok(x) => x,
        Err(e) => {
            return Err(e.to_string());
        }
    };

    if let Err(e) = file.write(format!("\n{}", content).as_bytes()) {
        return Err(e.to_string());
    }

    Ok(())
}
///
/// Opens a file, writes the content.
/// Creates the file if the file does not exist.
pub fn write_to_file(path: &str, content: &str) -> Result<(), String> {
    let mut file = match OpenOptions::new().write(true).create(true).open(path) {
        Ok(x) => x,
        Err(e) => {
            return Err(e.to_string());
        }
    };

    if let Err(e) = file.write(content.as_bytes()) {
        return Err(e.to_string());
    }

    Ok(())
}
