use std::{
    fs::{self, OpenOptions},
    io::Write,
};

use anyhow::{anyhow, Result};

pub const INSTALL_SUCCESS_FLAG: &str = "/var/tmp/2lazy4archinstallationflag";
/// AUR packages picked in part 1, installed by part 2 once yay exists.
pub const AUR_QUEUE: &str = "/var/tmp/2lazy4arch-aur-queue";

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

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GpuVendor {
    Intel,
    Amd,
    Nvidia,
    /// VMs, server BMCs, ... Mesa covers these.
    Other,
}

/// GPUs on the PCI bus, deduplicated by vendor.
pub fn detect_gpus() -> Vec<GpuVendor> {
    let mut gpus = vec![];
    for dev in fs::read_dir("/sys/bus/pci/devices").into_iter().flatten().flatten() {
        let read = |f: &str| fs::read_to_string(dev.path().join(f)).unwrap_or_default();
        // PCI class 0x03xxxx = display controller
        if !read("class").starts_with("0x03") {
            continue;
        }
        let vendor = match read("vendor").trim() {
            "0x8086" => GpuVendor::Intel,
            "0x1002" => GpuVendor::Amd,
            "0x10de" => GpuVendor::Nvidia,
            _ => GpuVendor::Other,
        };
        if !gpus.contains(&vendor) {
            gpus.push(vendor);
        }
    }
    gpus
}

/// Get UUID of root
/// This might fail if:
/// - the fstab is not generated
/// - the fstab is not generated with UUIDs using genfstab
/// - the root's UUID is not in fstab
pub fn get_uuid_root() -> Result<String> {
    let fstab = fs::read_to_string("/etc/fstab").map_err(|_| anyhow!("Could not open fstab"))?;
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
