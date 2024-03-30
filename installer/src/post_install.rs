use std::fs;

use crate::{pacman::Pacman, utils::sed};
use anyhow::Result;
use shell_iface::{logger::Logger, Shell};

/// PostInstall installs optional stuff.
/// This is same as the postinstall.sh
/// Calling PostInstall functions without the process running as superuser will fail.
pub struct PostInstall<'a> {
    shell: Shell<'a>,
    pacman: Pacman<'a>,
    is_yay_installed: bool,
}

impl<'a> PostInstall<'a> {
    /// Installs additional optional software
    pub fn new<'b>(logger: &'b Logger) -> PostInstall<'b> {
        let shell = Shell::new("PostInstall", logger);
        let pacman = Pacman::new(logger);

        PostInstall { shell, pacman, is_yay_installed: false }
    }

    /// reads from a file and installs all the packages.
    /// expects valid files without errors or invalid packages
    /// packages file uses pacman to install.
    /// aur packages file uses yay to install
    /// A valid file contains valid package names separated by a newline only
    pub fn install_additionals(&mut self, packages_file: &str, aur_packages_file: &str) -> Result<()> {
        let parsed_file = fs::read_to_string(packages_file)?;
        let packages = parsed_file.split("\n").collect::<Vec<&str>>();
        self.shell.log(&format!("Installing packages with pacman: {}", parsed_file));
        self.pacman.pacman().install(packages)?;

        if self.is_yay_installed{
            self.shell.log("Installing yay");
            self.setup_yay()?;
        }

        self.shell.log("Installing yay");
        let parsed_file = fs::read_to_string(aur_packages_file)?;
        let aur_packages = parsed_file.split("\n").collect::<Vec<&str>>();
        self.shell.log(&format!("Installing packages with aur: {}", parsed_file));
        self.pacman.pacman().install(aur_packages)?;
        Ok(())
    }

    pub fn setup_yay(&mut self) -> Result<()> {
        match self.shell.run_and_wait_with_args("rm", "-fr yay") {
            Ok(_) => {
                self.shell.log("Removed existing yay repo");
            }
            Err(_) => {
                self.shell.log("Yay repo not found. Cloning...");
            }
        }

        // yay requires go to install
        self.pacman.install(vec!["go"])?;

        self.shell
            .run_and_wait_with_args("git", "clone https://aur.archlinux.org/yay.git")?;

        self.shell
            .run_in_directory_and_wait_with_args("yay", "makepkg", "-si PKGBUILD")?;
        
        self.is_yay_installed = true;
        Ok(())
    }

    /// good to haves.
    /// includes specific stuff for me
    pub fn misc_options(&mut self) -> Result<()> {
        self.shell.log("Setting up pacman in style");
        sed("/etc/pacman.conf", 33, "ILoveCandy")?;
        sed("/etc/pacman.conf", 34, "Color")?;

        Ok(())
    }
}
