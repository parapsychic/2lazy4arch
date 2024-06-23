use std::{fs, os::unix::fs::FileTypeExt, path::Path};

use anyhow::{anyhow, Result};
use serde::Deserialize;
use shell_iface::{logger::Logger, Shell};

use crate::partition_table::PartitionTable;

#[derive(Debug, Deserialize)]
pub struct BlockDevicePartition {
    pub name: String,
    pub mountpoints: Option<Vec<Option<String>>>,
    pub size: Option<String>,
    pub children: Option<Vec<BlockDevicePartition>>,
}

#[derive(Debug, Deserialize)]
struct BlockDevices {
    blockdevices: Vec<BlockDevicePartition>,
}

pub struct Filesystem<'a> {
    shell: Shell<'a>,
    pub partitions: PartitionTable,
    pub format_boot: bool,
    pub format_home: bool,
}

impl<'a> Filesystem<'a> {
    pub fn new<'b>(logger: &'b Logger) -> Filesystem<'b> {
        let shell = Shell::new("FILESYSTEM", logger);
        Filesystem {
            shell,
            partitions: PartitionTable::new(),
            format_boot: false,
            format_home: false,
        }
    }

    pub fn get_disks(&mut self) -> Result<String> {
        match self.shell.run_with_args("lsblk", "-o NAME -d -n") {
            Ok(x) => Ok(String::from_utf8(x.stdout)?),
            Err(e) => Err(e),
        }
    }

    pub fn lsblk(&mut self) -> Result<Vec<BlockDevicePartition>> {
        let data = match self
            .shell
            .run_with_args("lsblk", "--json --output name,size,mountpoints")
        {
            Ok(x) => String::from_utf8(x.stdout)?,
            Err(e) => return Err(e),
        };

        let blockdevices: BlockDevices = serde_json::from_str(&data)?;
        Ok(blockdevices.blockdevices)
    }

    pub fn format_partitions(&mut self) -> Result<()> {
        if self.partitions.get_value("boot").is_none()
            || self.partitions.get_value("root").is_none()
        {
            self.shell.log("Boot or root is not set");
            return Err(anyhow!("Boot or root is not set"));
        }

        let _ = self.shell.run_and_wait_with_args(
            "mkfs.ext4",
            &format!("-F {}", self.partitions.get_value("root").clone().unwrap()),
        )?;

        if self.partitions.get_value("home").is_none() {
            self.shell
                .log("Home is not set. No separate partition will be created");
        } else {
            if self.format_home {
                let _ = self.shell.run_and_wait_with_args(
                    "mkfs.ext4",
                    &format!("-F {}", self.partitions.get_value("home").clone().unwrap()),
                )?;
            } else {
                self.shell.log("Format home is false, skipping...");
            }
        }

        if self.format_boot {
            let _ = self.shell.run_and_wait_with_args(
                "mkfs.fat",
                &format!(
                    "-F 32 {}",
                    self.partitions.get_value("boot").clone().unwrap()
                ),
            );
        }

        return Ok(());
    }

    /// Remove a mount point 
    pub fn remove_mount_point(&mut self, mount_point: &str) -> Result<String> {
        self.partitions.remove_key(mount_point)
    }

    /// Set other partitions
    pub fn set_mount_points(&mut self, partition: &str, mount_point: &str) -> Result<()> {
        if let Some(_) = self.partitions.get_key(mount_point) {
            // try to delete only if there is some value
            match self.partitions.remove(Some(&mount_point), Some(&partition)) {
                Ok(_) => {}
                Err(x) => {
                    self.shell.log(&x.to_string());
                    return Err(x);
                }
            }
        }

        let partition = partition.trim();
        let metadata = fs::metadata(&partition)?;
        if !metadata.file_type().is_block_device() || !ends_with_number(partition) {
            self.shell.log(&format!(
                "{}: NOT A BLOCK DEVICE or DOES NOT END WITH A NUMBER. Cannot mount to boot",
                partition
            ));
            return Err(anyhow!("{} does not look like a partition.", partition));
        }

        match self
            .partitions
            .insert(String::from(mount_point), String::from(partition))
        {
            Ok(_) => {}
            Err(x) => {
                self.shell.log(&x.to_string());
                return Err(x);
            }
        }
        Ok(())
    }

    /// Mounts all partitions
    pub fn mount_partitions(&mut self) -> Result<()> {
        // check if essential partitions are set.
        if self.partitions.get_value("boot").is_none()
            || self.partitions.get_value("root").is_none()
        {
            self.shell.log("Boot or root is not set");
            return Err(anyhow!("Boot or root is not set"));
        }

        // mount root
        self.shell
            .run_and_wait_with_args("mount", &format!("{} /mnt", self.get_root().unwrap()))?;

        // mount other devices in any order.
        for (k, v) in self.partitions.iter() {
            let mount_path = format!("/mnt/{}", &k);
            match Path::new(&mount_path).try_exists() {
                Ok(exists) => {
                    // check if a dir exists to mount to.
                    // Ideally, it shouldn't and we should be making it.
                    if exists {
                        self.shell.log(&format!(
                            "{} exists. This was not supposed to happen. Trying to continue.",
                            &mount_path
                        ));
                    } else {
                        self.shell.run_and_wait_with_args(
                            "mount",
                            &format!("--mkdir {} {}", v, mount_path),
                        )?;
                    }
                }
                Err(e) => {
                    self.shell.log(&format!("Existence of {} cannot be confirmed. This is usually a permission error. Original Error: {:#?}", &mount_path, e));
                    return Err(anyhow!("Could not confirm the existence of {}. This could be a permission issue on /mnt ", mount_path));
                }
            };
        }

        Ok(())
    }

    pub fn partition_disks(&mut self, disk: &str) -> Result<()> {
        let mut handle = self.shell.spawn_with_args("cfdisk", disk)?;
        let status = handle.wait()?;
        if status.success() {
            return Ok(());
        }
        self.shell
            .log("cfdisk failed. Is the script not running as root?");
        Err(anyhow!("cfdisk failed. Partitioning failure."))
    }

    /* GETTERS */
    pub fn get_boot(&self) -> Option<String> {
        match self.partitions.get_value("boot") {
            Some(x) => Some(x.to_string()),
            None => None,
        }
    }

    pub fn get_home(&self) -> Option<String> {
        match self.partitions.get_value("home") {
            Some(x) => Some(x.to_string()),
            None => None,
        }
    }

    pub fn get_root(&self) -> Option<String> {
        match self.partitions.get_value("root") {
            Some(x) => Some(x.to_string()),
            None => None,
        }
    }

    /* SETTERS */
    pub fn set_boot(&mut self, partition: &str) -> Result<()> {
        if let Some(_) = self.get_boot() {
            // try to delete only if there is some value
            match self.partitions.remove_key("boot") {
                Ok(_) => {}
                Err(x) => {
                    self.shell.log(&x.to_string());
                    return Err(x);
                }
            }
        }

        let partition = partition.trim();
        let metadata = fs::metadata(&partition)?;
        if !metadata.file_type().is_block_device() || !ends_with_number(partition) {
            self.shell.log(&format!(
                "{}: NOT A BLOCK DEVICE or DOES NOT END WITH A NUMBER. Cannot mount to boot",
                partition
            ));
            return Err(anyhow!("{} does not look like a partition.", partition));
        }

        match self
            .partitions
            .insert(String::from("boot"), partition.to_string())
        {
            Ok(_) => {}
            Err(x) => {
                self.shell.log(&x.to_string());
                return Err(x);
            }
        }
        Ok(())
    }

    pub fn set_home(&mut self, partition: Option<&str>) -> Result<()> {
        if partition.is_none() {
            if let Some(_) = self.get_home() {
                // try to delete only if there is some value
                match self.partitions.remove_key("home") {
                    Ok(_) => {}
                    Err(x) => {
                        self.shell.log(&x.to_string());
                        return Err(x);
                    }
                }
            }
            return Ok(());
        }

        let partition = partition.unwrap().trim();
        let metadata = fs::metadata(&partition)?;
        if !metadata.file_type().is_block_device() || !ends_with_number(partition) {
            self.shell.log(&format!(
                "{}: NOT A BLOCK DEVICE or DOES NOT END WITH A NUMBER. Cannot mount to boot",
                partition
            ));
            return Err(anyhow!("{} does not look like a partition.", partition));
        }

        match self
            .partitions
            .insert(String::from("home"), partition.to_string())
        {
            Ok(_) => {}
            Err(x) => {
                self.shell.log(&x.to_string());
                return Err(x);
            }
        }
        Ok(())
    }

    pub fn set_root(&mut self, partition: &str) -> Result<()> {
        if let Some(_) = self.get_root() {
            // try to delete only if there is some value
            match self.partitions.remove_key("root") {
                Ok(_) => {}
                Err(x) => {
                    self.shell.log(&x.to_string());
                    return Err(x);
                }
            }
        }
        let partition = partition.trim();
        let metadata = fs::metadata(&partition)?;
        if !metadata.file_type().is_block_device() || !ends_with_number(partition) {
            self.shell.log(&format!(
                "{}: NOT A BLOCK DEVICE or DOES NOT END WITH A NUMBER. Cannot mount to root",
                partition
            ));
            return Err(anyhow!("{} does not look like a partition.", partition));
        }

        match self
            .partitions
            .insert(String::from("root"), partition.to_string())
        {
            Ok(_) => {}
            Err(x) => {
                self.shell.log(&x.to_string());
                return Err(x);
            }
        }
        Ok(())
    }

    /* CLEAN UP FUNCTIONS */
    pub fn clear_mounts(&mut self) {
        self.partitions.clear();
    }

    pub fn try_unmount(&mut self) {
        for (_, v) in self.partitions.iter() {
            let _ = self.shell.run_and_wait_with_args("umount", v);
        }
    }
}

fn ends_with_number(s: &str) -> bool {
    s.chars().rev().take_while(|&c| c.is_ascii_digit()).count() > 0
}
