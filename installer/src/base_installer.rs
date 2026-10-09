use std::{fs::OpenOptions, io::Write};

use anyhow::{anyhow, Result};
use shell_iface::{logger::Logger, Shell};

use crate::pacman::Pacman;

/// What pacstrap installs, plus the microcode package if any.
pub fn base_packages(microcode: Option<&'static str>) -> Vec<&'static str> {
    let mut packages = vec!["base", "linux", "linux-firmware", "neovim", "reflector"];
    packages.extend(microcode);
    packages
}

/* This module contains all the utility fns for smaller base installation. */
pub struct BaseInstaller<'a> {
    shell: Shell<'a>,
    pacman: Pacman<'a>,
}

impl<'a> BaseInstaller<'a> {
    pub fn new<'b>(logger: &'b Logger) -> BaseInstaller<'b> {
        let shell = Shell::new("Base Installer", logger);
        BaseInstaller { shell, pacman: Pacman::new(logger) }
    }

    /// Installs the base packages
    pub fn base_packages_install(&mut self, packages: &[&str]) -> Result<()> {
        self.shell.log("Installing base packages.");
        let packages = self.pacman.keep_available(packages)?;

        match self
            .shell
            .run_and_wait_with_args("pacstrap", &format!("-K /mnt {}", packages.join(" ")))
        {
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
        self.shell.log("Generating fstab.");
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
