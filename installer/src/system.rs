//! What the installer knows about the machine it runs on.

use std::{fs, path::Path};

use anyhow::Result;
use serde::Deserialize;
use shell_iface::{logger::Logger, Shell};

use crate::utils::get_processor_make;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GpuVendor {
    Intel,
    Amd,
    Nvidia,
    /// VMs, server BMCs, ... Mesa covers these.
    Other,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Gpu {
    pub vendor: GpuVendor,
    /// PCI device ID; tells NVIDIA generations apart.
    pub device: u16,
}

/// A row of `lsblk --list --bytes`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct BlockDevice {
    pub path: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub size: Option<u64>,
    pub fstype: Option<String>,
    pub parttypename: Option<String>,
    pub label: Option<String>,
    pub partlabel: Option<String>,
    pub partuuid: Option<String>,
    pub uuid: Option<String>,
    pub model: Option<String>,
    /// Parent disk's kernel name, e.g. "nvme0n1"
    pub pkname: Option<String>,
    pub tran: Option<String>,
    #[serde(default)]
    pub rm: bool,
    /// Partition table: "gpt", "dos" or none
    pub pttype: Option<String>,
}

impl BlockDevice {
    /// One line for pickers, e.g. "/dev/sda1  512M  vfat  EFI System"
    pub fn describe(&self) -> String {
        let size = self.size.map(human_size);
        [&size, &self.fstype, &self.parttypename, &self.label, &self.model]
            .into_iter()
            .flatten()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .fold(self.path.clone(), |acc, s| format!("{acc}  {s}"))
    }

    /// Kernel name, "nvme0n1p2" for /dev/nvme0n1p2.
    pub fn name(&self) -> &str {
        self.path.trim_start_matches("/dev/")
    }
}

/// 1073741824 -> "1G", 1610612736 -> "1.5G"
pub fn human_size(bytes: u64) -> String {
    let mut size = bytes as f64;
    for unit in ["B", "K", "M", "G", "T"] {
        if size < 1024.0 || unit == "T" {
            return if size < 10.0 && unit != "B" && size.fract() > 0.05 {
                format!("{size:.1}{unit}")
            } else {
                format!("{size:.0}{unit}")
            };
        }
        size /= 1024.0;
    }
    unreachable!()
}

#[derive(Deserialize)]
struct Lsblk {
    blockdevices: Vec<BlockDevice>,
}

/// Block devices without RAM disks (1) and optical drives (11). Loop devices (the
/// ISO's squashfs) are type "loop", so they're never offered as disks.
pub fn lsblk(shell: &mut Shell) -> Result<Vec<BlockDevice>> {
    let output = shell.run_with_args(
        "lsblk",
        "--json --list --bytes -e 1,11 -o PATH,TYPE,SIZE,FSTYPE,PARTTYPENAME,LABEL,PARTLABEL,PARTUUID,UUID,MODEL,PKNAME,TRAN,RM,PTTYPE",
    )?;
    Ok(serde_json::from_slice::<Lsblk>(&output.stdout)?.blockdevices)
}

#[derive(Debug, Clone, Default)]
pub struct System {
    pub devices: Vec<BlockDevice>,
    pub gpus: Vec<Gpu>,
    pub cpu: Option<String>,
    pub uefi: bool,
    pub timezones: Vec<String>,
    /// Lines of /etc/locale.gen, e.g. "en_US.UTF-8 UTF-8"
    pub locales: Vec<String>,
    pub interfaces: Vec<String>,
    pub wireless: Vec<String>,
    pub kernel: String,
}

impl System {
    pub fn probe(logger: &Logger) -> System {
        let mut shell = Shell::new("System", logger);
        let (interfaces, wireless) = network_interfaces();
        System {
            devices: lsblk(&mut shell).unwrap_or_default(),
            gpus: detect_gpus(),
            cpu: get_processor_make(),
            uefi: Path::new("/sys/firmware/efi").exists(),
            timezones: parse_tzdata(&fs::read_to_string("/usr/share/zoneinfo/tzdata.zi").unwrap_or_default()),
            locales: parse_locale_gen(&fs::read_to_string("/etc/locale.gen").unwrap_or_default()),
            interfaces,
            wireless,
            kernel: fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default().trim().to_string(),
        }
    }

    pub fn disks(&self) -> Vec<&BlockDevice> {
        self.devices.iter().filter(|d| d.kind == "disk").collect()
    }

    pub fn partitions(&self) -> Vec<&BlockDevice> {
        self.devices.iter().filter(|d| d.kind == "part").collect()
    }

    /// Disks that could be the install target: not removable, not USB (the ISO stick), not zram.
    pub fn internal_disks(&self) -> Vec<&BlockDevice> {
        self.disks()
            .into_iter()
            .filter(|d| !d.rm && d.tran.as_deref() != Some("usb") && !d.name().starts_with("zram"))
            .filter(|d| d.size.unwrap_or(0) > 0)
            .collect()
    }

    pub fn has_gpu(&self, vendor: GpuVendor) -> bool {
        self.gpus.iter().any(|g| g.vendor == vendor)
    }
}

/// GPUs on the PCI bus.
pub fn detect_gpus() -> Vec<Gpu> {
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
        let device = u16::from_str_radix(read("device").trim().trim_start_matches("0x"), 16).unwrap_or(0);
        gpus.push(Gpu { vendor, device });
    }
    gpus
}

/// (all interfaces but lo, wireless ones)
fn network_interfaces() -> (Vec<String>, Vec<String>) {
    let mut all = vec![];
    let mut wireless = vec![];
    for entry in fs::read_dir("/sys/class/net").into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "lo" {
            continue;
        }
        if entry.path().join("wireless").exists() {
            wireless.push(name.clone());
        }
        all.push(name);
    }
    all.sort();
    wireless.sort();
    (all, wireless)
}

/// Zone names from tzdata.zi ("Z name ..." and "L target name"), what timedatectl lists.
pub fn parse_tzdata(s: &str) -> Vec<String> {
    let mut zones: Vec<String> = s
        .lines()
        .filter_map(|line| {
            let mut words = line.split(' ');
            match words.next()? {
                "Z" => words.next(),
                "L" => words.nth(1),
                _ => None,
            }
        })
        .map(String::from)
        .collect();
    zones.sort();
    zones.dedup();
    zones
}

/// "#en_US.UTF-8 UTF-8  " -> "en_US.UTF-8 UTF-8". Skips the comment header.
pub fn parse_locale_gen(s: &str) -> Vec<String> {
    s.lines()
        .filter_map(|line| {
            let line = line.strip_prefix('#').unwrap_or(line);
            let words: Vec<&str> = line.split_whitespace().collect();
            (!line.starts_with(char::is_whitespace) && words.len() == 2).then(|| words.join(" "))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsers() {
        assert_eq!(
            parse_tzdata("# version\nZ Asia/Kolkata 5:53:28 - LMT 1854\nL Asia/Kolkata Asia/Calcutta\nR x 1 2"),
            ["Asia/Calcutta", "Asia/Kolkata"]
        );
        assert_eq!(
            parse_locale_gen("# Configuration file\n#\n#  en_US ISO-8859-1\n#     <locale> <charset>\n#aa_DJ.UTF-8 UTF-8  \nen_US.UTF-8 UTF-8"),
            ["aa_DJ.UTF-8 UTF-8", "en_US.UTF-8 UTF-8"]
        );
        assert_eq!(human_size(512 * 1024 * 1024), "512M");
        assert_eq!(human_size(1536 * 1024 * 1024), "1.5G");
        assert_eq!(human_size(500 * 1024 * 1024 * 1024), "500G");
    }
}
