use std::{collections::BTreeSet, fs};

use anyhow::{anyhow, bail, Result};
use config::{strip_secrets, Config, Desktop};
use essentials::Essentials;
use pacman::Pacman;
use serde_json::Value;
use shell_iface::logger::Logger;
use source::Source;
use system::System;
use target::Target;

pub mod base_installer;
pub mod config;
pub mod docker;
pub mod essentials;
pub mod filesystem_tasks;
pub mod hooks;
pub mod network;
pub mod pacman;
pub mod post_install;
pub mod remote;
pub mod session;
pub mod source;
pub mod storage;
pub mod system;
pub mod target;
pub mod utils;
pub mod validate;

/// Where the install leaves a copy of this binary; user steps run it inside the new system.
pub const INSTALLED_BINARY: &str = "/usr/local/bin/2lazy4arch";
/// Passwordless sudo for user steps, removed afterwards even if one fails. sudo uses
/// the last matching rule, so this has to sort after 10-wheel.
const SETUP_SUDOERS: &str = "/etc/sudoers.d/99-2lazy4arch-setup";
/// User steps inside the new system add the packages they skipped here.
pub const SKIPPED_FILE: &str = "/var/tmp/2lazy4arch-skipped";

pub struct Install<'a> {
    pub cfg: &'a Config,
    pub sys: &'a System,
    pub source: &'a Source,
    pub logger: &'a Logger,
    /// Copied to /etc/2lazy4arch/config.yaml with secrets stripped
    pub config_value: Value,
}

pub struct Summary {
    /// Packages that weren't found and were skipped
    pub skipped: Vec<String>,
}

pub fn step(msg: &str) {
    println!("\n\x1b[1;36m==>\x1b[0m \x1b[1m{msg}\x1b[0m");
}

/// Everything from getting online to a finished system. `confirm_missing` decides
/// whether to go on when packages aren't in the repos; it's asked before any disk
/// is touched.
pub fn install(job: &Install, confirm_missing: &dyn Fn(&[String]) -> bool) -> Result<Summary> {
    let Install { cfg, sys, source, logger, .. } = *job;

    step("Getting online");
    network::apply_proxy(cfg);
    network::bring_up_iso(cfg, source, logger)?;
    hooks::set_env(cfg, source);

    if !cfg.hooks.pre_install.is_empty() {
        step("pre_install hooks");
        hooks::run(cfg, hooks::Phase::PreInstall, source, logger)?;
    }

    step("Mirrors");
    let mut pacman = Pacman::new(logger);
    if cfg.mirrors != "auto" {
        if let Err(e) = pacman.run_reflector(&cfg.mirrors) {
            println!("reflector failed ({e}), keeping the current mirrorlist.");
        }
    }
    pacman.update_mirrors()?;

    step("Checking packages");
    let packages = cfg.packages(&sys.gpus);
    let mut essentials = Essentials::new(logger, cfg.bootloader, cfg.admin_tool);
    let base = base_installer::base_packages(cfg.microcode(sys.cpu.as_deref()));
    let wanted: Vec<&str> = base.iter().copied().chain(essentials.packages()).chain(packages.repo.iter().map(String::as_str)).collect();
    let missing = pacman.missing_from_repos(&wanted)?;
    if !missing.is_empty() && !confirm_missing(&missing) {
        bail!("Stopped before touching the disks: some packages aren't in the repos.");
    }

    step("Partitions");
    let plan = storage::plan(&cfg.storage, sys).map_err(|e| anyhow!(e.join("; ")))?;
    let mut filesystem = storage::apply(&plan, logger)?;
    let result = (|| -> Result<()> {
        filesystem.format_partitions()?;
        filesystem.mount_partitions()?;
        step("Installing the base system");
        let mut base_installer = base_installer::BaseInstaller::new(logger);
        base_installer.base_packages_install(&base)?;
        base_installer.genfstab()?;
        fs::copy(std::env::current_exe()?, target::path(INSTALLED_BINARY))?;
        Ok(())
    })();
    if result.is_err() {
        filesystem.try_unmount();
    }
    result?;

    let mut target = Target::new(logger);

    step("Swap, timezone, locale, hostname");
    if cfg.packages.multilib || !cfg.rice_users().is_empty() {
        essentials.enable_multilib()?;
    }
    essentials.initialize_swap(cfg.storage.swap.0)?;
    essentials.set_timezones(&cfg.timezone)?;
    essentials.gen_locale(&cfg.locale)?;
    essentials.set_hostname(&cfg.hostname, &cfg.network.hosts)?;

    step("Installing drivers, desktop and apps");
    essentials.install_essentials(&packages.repo, &packages.services)?;
    if cfg.desktop == Desktop::Dwm {
        essentials.install_dwm()?;
    }
    if cfg.nvidia_proprietary(&sys.gpus) {
        essentials.remove_kms_hook()?;
    }

    step("Bootloader");
    essentials.mkinitcpio()?;
    essentials.install_bootloader()?;

    step("Users");
    essentials.create_users(cfg)?;

    step("Network, remote access, Docker, login");
    let mut units = network::configure_target(cfg, source, &mut target)?;
    units.extend(remote::configure(cfg, &mut target)?);
    units.extend(docker::configure(cfg, source)?);
    for unit in units {
        target.enable(&unit)?;
    }
    session::autologin(cfg, &mut target)?;

    if !cfg.hooks.post_install.is_empty() {
        step("post_install hooks");
        hooks::run(cfg, hooks::Phase::PostInstall, source, logger)?;
    }

    // Steps 7-10: as users inside the new system, with temporary passwordless sudo
    let sudo_users: BTreeSet<String> = cfg
        .main_user()
        .map(|u| u.name.clone())
        .into_iter()
        .chain(cfg.rice_users())
        .chain(cfg.hooks.post_setup.iter().filter_map(|s| s.run_as.clone()).filter(|u| u != "root"))
        .collect();
    let rule: String = sudo_users.iter().map(|u| format!("{u} ALL=(ALL:ALL) NOPASSWD: ALL\n")).collect();
    target::write_mode(SETUP_SUDOERS, &rule, 0o440)?;
    let result = user_steps(job, &packages, &mut target);
    let removed = fs::remove_file(target::path(SETUP_SUDOERS));
    result?;
    removed?;

    let mut stripped = job.config_value.clone();
    strip_secrets(&mut stripped);
    target::write_mode("/etc/2lazy4arch/config.yaml", &serde_norway::to_string(&stripped)?, 0o600)?;

    let mut skipped: Vec<String> = pacman.skipped.clone();
    skipped.extend(essentials.skipped().iter().cloned());
    skipped.extend(fs::read_to_string(target::path(SKIPPED_FILE)).unwrap_or_default().split_whitespace().map(String::from));
    let _ = fs::remove_file(target::path(SKIPPED_FILE));
    skipped.sort();
    skipped.dedup();
    Ok(Summary { skipped })
}

fn user_steps(job: &Install, packages: &config::PackageSet, target: &mut Target) -> Result<()> {
    let Install { cfg, source, logger, .. } = *job;
    let homes = |target: &mut Target| -> Result<()> {
        for user in &cfg.users {
            target.chown(&user.name, &user.home())?;
        }
        Ok(())
    };
    homes(target)?;
    let main = cfg.main_user().map(|u| u.name.clone()).unwrap_or("root".into());

    if !packages.aur.is_empty() {
        step("AUR packages");
        let args: Vec<String> = packages.aur.iter().map(|p| shell_words::quote(p).to_string()).collect();
        target.run_as(&main, INSTALLED_BINARY, &format!("--user-setup -- {}", args.join(" ")))?;
    }
    if !packages.after_aur.is_empty() {
        let after: Vec<&str> = packages.after_aur.iter().map(String::as_str).collect();
        Pacman::in_target(logger).install(&after)?;
    }
    for user in cfg.rice_users() {
        step(&format!("ParaPsychic rice for {user}"));
        target.run_as(&user, INSTALLED_BINARY, "--user-setup --rice")?;
    }

    step("Autostart");
    // after the AUR, so units from AUR packages exist
    for service in &cfg.autostart.services {
        target.enable(service)?;
    }
    session::autostart_apps(cfg, &|_| remote::wayvnc_command(cfg).into_iter().collect())?;
    homes(target)?;

    if !cfg.hooks.post_setup.is_empty() {
        step("post_setup hooks");
        hooks::run(cfg, hooks::Phase::PostSetup, source, logger)?;
    }
    Ok(())
}

/// Copies the install log into the new system.
pub fn copy_log() {
    let _ = fs::copy(utils::LOG_FILE, target::path(utils::LOG_FILE));
}
