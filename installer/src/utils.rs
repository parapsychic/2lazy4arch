use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::fd::AsRawFd,
    process::{Child, Command, Stdio},
};

use anyhow::{anyhow, Result};

/// Everything the install prints, on the ISO; copied into the new system at the end.
pub const LOG_FILE: &str = shell_iface::logger::LOG_FILE;

/// Writes the content, replacing the file if it exists.
pub fn write_to_file(path: &str, content: &str) -> Result<()> {
    Ok(fs::write(path, content)?)
}

/// Opens a file, appends the content.
/// Creates the file if the file does not exist.
/// Adds a newline before appending just to be sure.
pub fn append_to_file(path: &str, content: &str) -> Result<()> {
    let mut file = OpenOptions::new().append(true).create(true).open(path)?;
    file.write_all(format!("\n{}", content).as_bytes())?;
    Ok(())
}

/// Checks whether the processor is Intel or AMD.
/// Returns None if none of them.
pub fn get_processor_make() -> Option<String> {
    let processor_info = fs::read_to_string("/proc/cpuinfo").ok()?;
    if processor_info.contains("AuthenticAMD") {
        return Some(String::from("amd"));
    }
    if processor_info.contains("GenuineIntel") {
        return Some(String::from("intel"));
    }
    None
}

/// Root's UUID from an fstab written by genfstab -U.
pub fn get_uuid_root(fstab_path: &str) -> Result<String> {
    let fstab = fs::read_to_string(fstab_path).map_err(|_| anyhow!("Could not open {fstab_path}"))?;
    fstab
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .map(|l| l.split_whitespace().collect::<Vec<_>>())
        .find(|row| row.len() > 2 && row[1] == "/")
        .and_then(|row| row[0].strip_prefix("UUID=").map(String::from))
        .ok_or_else(|| anyhow!("Could not find the root UUID in fstab"))
}

/// Check if a string is a valid mount point, e.g. "data" or "/mnt/windows".
pub fn is_valid_mount_point(name: &str) -> bool {
    let name = name.trim_matches('/');
    !name.is_empty()
        && name.split('/').all(|part| {
            !part.is_empty()
                && !part.starts_with('-')
                && part != ".."
                && part.chars().all(|c| c.is_ascii_alphanumeric() || "_-.".contains(c))
        })
}

/// From here on, stdout and stderr (ours and every command's) also go to `path`.
/// Keep the returned tee alive until the end.
pub fn tee_output(path: &str) -> Result<Child> {
    let tee = Command::new("tee").args(["-a", path]).stdin(Stdio::piped()).spawn()?;
    let fd = tee.stdin.as_ref().ok_or_else(|| anyhow!("tee has no stdin"))?.as_raw_fd();
    nix::unistd::dup2(fd, 1)?;
    nix::unistd::dup2(fd, 2)?;
    Ok(tee)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mount_points() {
        assert!(is_valid_mount_point("/data"));
        assert!(is_valid_mount_point("mnt/windows"));
        assert!(!is_valid_mount_point("/"));
        assert!(!is_valid_mount_point("a//b"));
        assert!(!is_valid_mount_point("../etc"));
        assert!(!is_valid_mount_point("my disk"));
    }
}
