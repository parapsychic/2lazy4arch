use std::fs;

use anyhow::Result;
use base_installer::BaseInstaller;
use config::{Config, Desktop};
use essentials::Essentials;
use filesystem_tasks::Filesystem;
use pacman::Pacman;
use shell_iface::logger::Logger;
use utils::{write_to_file, AUR_QUEUE, INSTALL_SUCCESS_FLAG};

pub mod base_installer;
pub mod config;
pub mod essentials;
pub mod filesystem_tasks;
pub mod pacman;
pub mod post_install;
pub mod utils;

/// Where part 1 leaves a copy of this binary for part 2.
pub const INSTALLED_BINARY: &str = "/usr/local/bin/2lazy4arch";

/// Ranks mirrors, syncs, and returns the packages part 1 would install that aren't
/// in the repos. Touches nothing on disk, so call it before install().
pub fn check_packages(logger: &Logger, cfg: &Config) -> Result<Vec<String>> {
    let mut pacman = Pacman::new(logger);
    step("Ranking mirrors");
    if let Err(e) = pacman.run_reflector(&cfg.mirror_country) {
        println!("reflector failed ({e}), keeping the current mirrorlist.");
    }
    pacman.update_mirrors()?;

    step("Checking packages");
    let mut packages = base_installer::base_packages();
    packages.extend(Essentials::new(logger, cfg.bootloader, cfg.super_user_utility).packages());
    packages.extend(cfg.packages().repo);
    pacman.missing_from_repos(&packages)
}

/// Part 1: from partitions to a bootable system with drivers, desktop and browser.
/// Packages that don't exist are skipped. Leaves the process chrooted in /mnt on success.
pub fn install(filesystem: &mut Filesystem, logger: &Logger, cfg: &Config) -> Result<()> {
    // Until we chroot, a failure can still unmount everything.
    let result = install_base(filesystem, logger);
    if result.is_err() {
        filesystem.try_unmount();
    }
    result?;

    install_system(logger, cfg)
}

fn install_base(filesystem: &mut Filesystem, logger: &Logger) -> Result<()> {
    step("Formatting and mounting partitions");
    filesystem.format_partitions()?;
    filesystem.mount_partitions()?;

    step("Installing the base system");
    let mut base_installer = BaseInstaller::new(logger);
    base_installer.base_packages_install()?;
    base_installer.genfstab()?;

    fs::copy(std::env::current_exe()?, format!("/mnt{INSTALLED_BINARY}"))?;
    Ok(())
}

fn install_system(logger: &Logger, cfg: &Config) -> Result<()> {
    let mut essentials = Essentials::new(logger, cfg.bootloader, cfg.super_user_utility);
    let packages = cfg.packages();

    step("Entering the new system");
    essentials.chroot()?;

    step("Swap, timezone, locale, hostname");
    essentials.initialize_swap(cfg.swap_gb)?;
    essentials.set_timezones(&cfg.timezone)?;
    essentials.gen_locale(&cfg.locale)?;
    essentials.set_hostname(&cfg.hostname)?;
    essentials.set_password("root", &cfg.root_password)?;

    step("Installing drivers, desktop and apps");
    essentials.install_essentials(&packages.repo, &packages.services)?;
    if cfg.desktop == Desktop::Dwm {
        essentials.install_dwm()?;
    }
    if cfg.nvidia_proprietary() {
        essentials.remove_kms_hook()?;
    }

    step("Installing the bootloader");
    essentials.mkinitcpio()?;
    essentials.install_bootloader()?;

    step("Creating your user");
    essentials.user_management(&cfg.username, &cfg.password)?;

    if !packages.aur.is_empty() {
        write_to_file(AUR_QUEUE, &packages.aur.join("\n"))?;
    }
    write_to_file(INSTALL_SUCCESS_FLAG, "true")
}

fn step(msg: &str) {
    println!("\n\x1b[1;36m==>\x1b[0m \x1b[1m{msg}\x1b[0m");
}
