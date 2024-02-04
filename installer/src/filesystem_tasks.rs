use std::{fs, os::unix::fs::FileTypeExt, path::Path};

use anyhow::{anyhow, Result};
use shell_iface::{logger::Logger, Shell};

use crate::partition_table::PartitionTable;

pub struct Filesystem<'a> {
    shell: Shell<'a>,
    partitions: PartitionTable,
}

impl<'a> Filesystem<'a> {
    pub fn new<'b>(logger: &'b Logger) -> Filesystem<'b> {
        let shell = Shell::new("FILESYSTEM", logger);
        Filesystem {
            shell,
            partitions: PartitionTable::new(),
        }
    }

    pub fn get_disks(&mut self) -> Result<String> {
        match self.shell.run_with_args("lsblk", "-o NAME -d -n") {
            Ok(x) => Ok(String::from_utf8(x.stdout)?),
            Err(e) => Err(e),
        }
    }

    pub fn lsblk(&mut self) -> Result<String> {
        match self.shell.run("lsblk") {
            Ok(x) => Ok(String::from_utf8(x.stdout)?),
            Err(e) => Err(e),
        }
    }

    pub fn format_partitions(&mut self, format_boot: bool) -> Result<()> {
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
            let _ = self.shell.run_and_wait_with_args(
                "mkfs.ext4",
                &format!("-F {}", self.partitions.get_value("home").clone().unwrap())
            )?;
        }

        if format_boot {
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

    pub fn mount_partitions(&mut self) -> Result<()> {
        if self.partitions.get_value("boot").is_none()
            || self.partitions.get_value("root").is_none()
        {
            self.shell.log("Boot or root is not set");
            return Err(anyhow!("Boot or root is not set"));
        }

        self.shell
            .run_and_wait_with_args("mount", &format!("{} /mnt", self.get_root().unwrap()))?;

        match Path::new("/mnt/boot").try_exists() {
            Ok(exists) => {
                if exists {
                    self.shell.log(
                        "/mnt/boot exists. This was not supposed to happen. Trying to continue.",
                    );
                } else {
                    fs::create_dir("/mnt/boot")?;
                }
                self.shell.run_and_wait_with_args(
                    "mount",
                    &format!("{} /mnt/boot", self.get_boot().unwrap()),
                )?;
            }
            Err(e) => {
                self.shell.log(&format!("Existence of /mnt/boot cannot be confirmed. This is usually a permission error. Original Error: {:#?}", e));
                return Err(anyhow!("Could not confirm the existence of /mnt/boot. This could be a permission issue on /mnt "));
            }
        };

        if let Some(home_path) = self.get_home() {
            match Path::new("/mnt/home").try_exists() {
                Ok(exists) => {
                    if exists {
                        self.shell.log("/mnt/home exists. This was not supposed to happen. Trying to continue.");
                    } else {
                        fs::create_dir("/mnt/home")?;
                    }
                    self.shell
                        .run_and_wait_with_args("mount", &format!("{} /mnt/home", home_path))?;
                }
                Err(e) => {
                    self.shell.log(&format!("Existence of /mnt/home cannot be confirmed. This is usually a permission error. Original Error: {:#?}", e));
                    return Err(anyhow!("Could not confirm the existence of /mnt/home. This could be a permission issue on /mnt "));
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
            if let Some(_) = self.get_home() { // try to delete only if there is some value
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

    pub fn try_unmount(&mut self){
        for (_, v) in self.partitions.iter() {
           let _ = self.shell.run_and_wait_with_args("umount", v);
        }
    }
}

fn ends_with_number(s: &str) -> bool {
    s.chars().rev().take_while(|&c| c.is_ascii_digit()).count() > 0
}
