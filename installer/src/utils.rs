use std::{fs::OpenOptions, io::Write};

use anyhow::{anyhow, Result};

/// Opens a file, writes the content.
/// Creates the file if the file does not exist.
pub fn write_to_file(path: &str, content: &str) -> Result<()> {
    let mut fstab = match OpenOptions::new().write(true).create(true).open(path) {
        Ok(x) => x,
        Err(e) => {
            return Err(anyhow!(e));
        }
    };

    if let Err(e) = fstab.write(content.as_bytes()) {
        return Err(anyhow!(e));
    }

    Ok(())
}


/// Opens a file, appends the content.
/// Creates the file if the file does not exist.
/// Adds a newline before appending just to be sure.
pub fn append_to_file(path: &str, content: &str) -> Result<()> {
    let mut fstab = match OpenOptions::new().append(true).create(true).open(path) {
        Ok(x) => x,
        Err(e) => {
            return Err(anyhow!(e));
        }
    };

    if let Err(e) = fstab.write(format!("\n{}",content).as_bytes()) {
        return Err(anyhow!(e));
    }

    Ok(())
}
