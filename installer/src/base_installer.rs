use std::{fs::OpenOptions, io::Write};

use anyhow::{anyhow, Result};
use shell_iface::{logger::Logger, Shell};

use crate::utils::get_processor_make;

/* This module contains all the utility fns for smaller base installation. */
pub struct BaseInstaller<'a> {
    shell: Shell<'a>,
}

impl<'a> BaseInstaller<'a> {
    pub fn new<'b>(logger: &'b Logger) -> BaseInstaller<'b> {
        let shell = Shell::new("Base Installer", logger);
        BaseInstaller { shell }
    }

    /// Installs the base packages
    pub fn base_packages_install(&mut self) -> Result<()> {
        self.shell.log(&"Installing base packages.");

        let package_cmd = if let Some(p) = get_processor_make() {
            format!("-K /mnt base linux linux-firmware {}-ucode neovim reflector", p)
        } else {
            String::from("-K /mnt base linux linux-firmware neovim reflector")
        };

        match self.shell.run_and_wait_with_args("pacstrap", &package_cmd) {
            Ok(_) => Ok(()),
            Err(e) => {
                self.shell.log(&format!(
                    "Failed to install base packages: ORIGINAL ERROR: {}",
                    e
                ));
                Err(anyhow!("Could not install base packages."))
            }
        }
    }

    /// Generates and Writes fstab configuration.
    pub fn genfstab(&mut self) -> Result<()> {
        self.shell.log(&"Generating fstab.");
        let output = self.shell.run_with_args("genfstab", "-U /mnt")?;

        let mut fstab = match OpenOptions::new()
            .append(true)
            .create(true)
            .open("/mnt/etc/fstab")
        {
            Ok(x) => x,
            Err(e) => {
                self.shell
                    .log(&format!("Could not open /mnt/etc/fstab. {}", e));
                return Err(anyhow!("Could not open fstab"));
            }
        };

        if let Err(e) = fstab.write(&output.stdout) {
            self.shell
                .log(&format!("Could not write to /mnt/etc/fstab. {}", e));
            return Err(anyhow!("Could not write to fstab"));
        }

        Ok(())
    }
}
