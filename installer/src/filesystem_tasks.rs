use std::{fs, os::unix::fs::FileTypeExt};

use anyhow::{anyhow, Result};
use shell_iface::{Shell, logger::Logger};

use crate::enums::StorageSize;

pub struct Filesystem<'a> {
    shell: Shell<'a>,
    boot: Option<String>,
    root: Option<String>,
    home: Option<String>,
    logger: &'a Logger,
}

impl<'a> Filesystem<'a> {
    pub fn new<'b>(logger: &'b Logger) -> Filesystem<'b> {
        let shell = Shell::new("FILESYSTEM", logger);
        Filesystem {
            shell,
            boot: None,
            root: None,
            home: None,
            logger,
        }
    }

    pub fn get_disks(&mut self) -> Result<String> {
        match self.shell.run_with_args("lsblk", "-o NAME -d -n") {
            Ok(x) => Ok(String::from_utf8(x.stdout)?),
            Err(e) => Err(e),
        }
    }

    pub fn initialize_swap(&mut self, size: usize, unit: Option<StorageSize>) -> Result<()> {
        let unit_char = match unit {
            Some(StorageSize::Megabyte) => 'M',
            Some(StorageSize::Gigabyte) => 'G',
            Some(StorageSize::Kilobyte) => 'K',
            Some(StorageSize::Terabyte) => 'T',
            Some(StorageSize::Byte) => ' ',
            None => {
                println!("No unit set, defaulting to Megabytes. \n [This shouldn't have happenend. The UI has sent some wrong info");
                'M'
            }
        };

        println!("Size: {}", size);
        println!("Creating Swap Partition");
        let status = self.shell.run_and_wait_with_args(
            "dd",
            "if=/dev/zero of=/swapfile bs=1M count=8192 status=progress",
        )?;

        if !status.success(){
            self.logger.debug("FILESYSTEM: dd failed. Exited with non-zero status.");
            return Err(anyhow!("Could not create swap file."));
        }

        Ok(())
    }

    pub fn lsblk(&mut self) -> Result<String> {
        match self.shell.run("lsblk") {
            Ok(x) => Ok(String::from_utf8(x.stdout)?),
            Err(e) => Err(e),
        }
    }

    pub fn format_partitions(&mut self, format_boot: bool) -> Result<()> {
        if self.boot.is_none() || self.root.is_none() {
            self.logger.debug("FILESYSTEM: Boot or root is not set");
            return Err(anyhow!("Boot or root is not set"));
        }

        let _ = self
            .shell
            .run_and_wait_with_args("mkfs.ext4", &self.root.clone().unwrap())?;

        if self.home.is_none() {
            self.logger
                .debug("Home is not set. No separate partition will be created");
        } else {
            let _ = self
                .shell
                .run_and_wait_with_args("mkfs.ext4", &self.home.clone().unwrap())?;
        }

        if format_boot {
            let _ = self.shell.run_and_wait_with_args("mkfs.fat", &format!("-F 32 {}", self.boot.clone().unwrap()));
        }

        return Ok(());
    }

    pub fn partition_disks(&mut self, disk: &str) -> Result<()> {
        let mut handle = self.shell.spawn_with_args("cfdisk", disk)?;
        let status = handle.wait()?;
        if status.success() {
            return Ok(());
        }
        self.logger
            .debug("cfdisk failed. Is the script not running as root?");
        Err(anyhow!("cfdisk failed. Partitioning failure."))
    }


    pub fn set_boot(&mut self, partition: &str) -> Result<()> {
        let partition = partition.trim();
        let metadata = fs::metadata(&partition)?;
        if !metadata.file_type().is_block_device() || !ends_with_number(partition){
            self.logger.debug(&format!("FILESYSTEM: {}: NOT A BLOCK DEVICE or DOES NOT END WITH A NUMBER. Cannot mount to boot", partition));
            return Err(anyhow!("{} does not look like a partition.", partition))
        }
        if let Some(x) = &self.root {
            if x == partition {
            self.logger.debug(&format!("FILESYSTEM: {}: / and /boot have same partition. Cannot mount to boot", partition));
            return Err(anyhow!("{} is already mounted to /.", partition))
            }
        }

        if let Some(x) = &self.home{
            if x == partition {
            self.logger.debug(&format!("FILESYSTEM: {}: /home and /boot have same partition. Cannot mount to boot", partition));
            return Err(anyhow!("{} is already mounted to /home.", partition))
            }
        }

        self.boot = Some(partition.to_string());
        Ok(())
    }

    pub fn set_root(&mut self, partition: &str) -> Result<()> {
        let partition = partition.trim();
        let metadata = fs::metadata(&partition)?;
        if !metadata.file_type().is_block_device() || !ends_with_number(partition){
            self.logger.debug(&format!("FILESYSTEM: {}: NOT A BLOCK DEVICE or DOES NOT END WITH A NUMBER. Cannot mount to root", partition));
            return Err(anyhow!("{} does not look like a partition.", partition))
        }
        if let Some(x) = &self.boot {
            if x == partition {
            self.logger.debug(&format!("FILESYSTEM: {}: / and /boot have same partition. Cannot mount to root", partition));
            return Err(anyhow!("{} is already mounted to /boot.", partition))
            }
        }

        if let Some(x) = &self.home{
            if x == partition {
            self.logger.debug(&format!("FILESYSTEM: {}: Cannot mount to root. To set /home and / to same partition, uncheck separate /home", partition));
            return Err(anyhow!("{} is already mounted to /home. To have them share the same partition, uncheck separate /home.", partition))
            }
        }

        self.root = Some(partition.to_string());
        Ok(())
    }
}

fn ends_with_number(s: &str) -> bool {
    s.chars().rev().take_while(|&c| c.is_ascii_digit()).count() > 0
}

