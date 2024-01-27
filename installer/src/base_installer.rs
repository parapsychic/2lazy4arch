use anyhow::{anyhow, Result};
use shell_iface::{
    logger::{self, Logger},
    Shell,
};

/* This module contains all the utility fns for smaller base installation. */
pub struct BaseInstaller<'a> {
    shell: Shell<'a>,
    logger: &'a Logger,
}

impl<'a> BaseInstaller<'a> {
    pub fn new<'b>(logger: &'b Logger) -> BaseInstaller<'b> {
        let shell = Shell::new("PACMAN", logger);
        BaseInstaller { shell, logger }
    }

    pub fn base_packages_install(&mut self) -> Result<()> {
        match self
            .shell
            .run_and_wait_with_args("pacstrap", "-K /mnt base linux linux-firmware neovim")
        {
            Ok(_) => Ok(()),
            Err(e) => {
                self.logger.debug(&format!(
                    "Failed to install base packages: ORIGINAL ERROR: {}",
                    e
                ));
                Err(anyhow!("Could not install base packages."))
            }
        }
    }
}
