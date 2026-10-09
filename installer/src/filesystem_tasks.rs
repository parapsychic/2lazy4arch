use std::collections::BTreeMap;

use anyhow::{anyhow, bail, Result};
use shell_iface::{logger::Logger, Shell};

pub struct Filesystem<'a> {
    shell: Shell<'a>,
    /// Mount point relative to /mnt -> partition. "" is the root, so the
    /// BTreeMap order mounts / first and parents before children (home < home/media).
    pub partitions: BTreeMap<String, String>,
    pub format_boot: bool,
    pub format_home: bool,
}

impl<'a> Filesystem<'a> {
    pub fn new<'b>(logger: &'b Logger) -> Filesystem<'b> {
        let shell = Shell::new("FILESYSTEM", logger);
        Filesystem {
            shell,
            partitions: BTreeMap::new(),
            format_boot: false,
            format_home: false,
        }
    }

    /// Partition mounted at `mount_point` ("/", "boot", "/home", ...).
    pub fn get(&self, mount_point: &str) -> Option<&String> {
        self.partitions.get(mount_point.trim_matches('/'))
    }

    /// Mounts `partition` at `mount_point`, or clears the mount point when None.
    pub fn set(&mut self, mount_point: &str, partition: Option<&str>) -> Result<()> {
        let key = mount_point.trim_matches('/').to_string();
        self.partitions.remove(&key);
        let Some(partition) = partition else {
            return Ok(());
        };
        if let Some((used, _)) = self.partitions.iter().find(|(_, p)| *p == partition) {
            bail!("{partition} is already used for /{used}");
        }
        self.partitions.insert(key, partition.to_string());
        Ok(())
    }

    pub fn format_partitions(&mut self) -> Result<()> {
        let root = self.get("/").cloned().ok_or_else(|| anyhow!("Root partition is not set"))?;
        let boot = self.get("boot").cloned().ok_or_else(|| anyhow!("Boot partition is not set"))?;

        self.shell.run_and_wait_with_args("mkfs.ext4", &format!("-F {root}"))?;
        if self.format_boot {
            self.shell.run_and_wait_with_args("mkfs.fat", &format!("-F 32 {boot}"))?;
        }
        match self.get("home").cloned() {
            Some(home) if self.format_home => {
                self.shell.run_and_wait_with_args("mkfs.ext4", &format!("-F {home}"))?;
            }
            _ => self.shell.log("Not formatting home."),
        }
        Ok(())
    }

    /// Mounts everything under /mnt.
    pub fn mount_partitions(&mut self) -> Result<()> {
        for (mount_point, partition) in self.partitions.clone() {
            self.shell
                .run_and_wait_with_args("mount", &format!("--mkdir {partition} /mnt/{mount_point}"))?;
        }
        Ok(())
    }

    pub fn partition_disks(&mut self, disk: &str) -> Result<()> {
        let status = self.shell.spawn_with_args("cfdisk", disk)?.wait()?;
        if !status.success() {
            self.shell.log("cfdisk failed. Is the script not running as root?");
            bail!("cfdisk failed. Partitioning failure.");
        }
        Ok(())
    }

    pub fn try_unmount(&mut self) {
        let _ = self.shell.run_and_wait_with_args("umount", "-R /mnt");
    }
}
