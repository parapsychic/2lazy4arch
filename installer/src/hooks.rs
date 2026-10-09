//! `hooks:` your own steps. `run:` is bash -e, `script:` runs with its own shebang.

use std::{fs, os::unix::fs::PermissionsExt};

use anyhow::{Context, Result};
use shell_iface::{logger::Logger, Shell};
use shell_words::quote;

use crate::{
    config::{Config, HookStep, OnError},
    source::Source,
    target::{self, Target},
};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase {
    /// On the ISO before partitioning, as root
    PreInstall,
    /// After the system is configured, inside it unless chroot: false
    PostInstall,
    /// The very end, inside, as the first user by default
    PostSetup,
}

/// LAZY_* for every hook. LAZY_ROOT is /mnt here, / inside the new system.
pub fn set_env(cfg: &Config, source: &Source) {
    let main = cfg.main_user();
    let users: Vec<&str> = cfg.users.iter().map(|u| u.name.as_str()).collect();
    let desktop = serde_json::to_value(cfg.desktop).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
    for (key, value) in [
        ("LAZY_USER", main.map(|u| u.name.clone()).unwrap_or_default()),
        ("LAZY_HOME", main.map(|u| u.home()).unwrap_or_default()),
        ("LAZY_USERS", users.join(" ")),
        ("LAZY_ADMINS", cfg.admins().join(" ")),
        ("LAZY_DESKTOP", desktop),
        ("LAZY_DIR", source.dir().display().to_string()),
        ("LAZY_ROOT", target::ROOT.to_string()),
    ] {
        std::env::set_var(key, value);
    }
}

pub fn run(cfg: &Config, phase: Phase, source: &Source, logger: &Logger) -> Result<()> {
    let steps = match phase {
        Phase::PreInstall => &cfg.hooks.pre_install,
        Phase::PostInstall => &cfg.hooks.post_install,
        Phase::PostSetup => &cfg.hooks.post_setup,
    };
    for (i, step) in steps.iter().enumerate() {
        println!("\x1b[1m-> {}\x1b[0m", step.label());
        if let Err(e) = run_step(cfg, phase, i, step, source, logger) {
            match step.on_error {
                OnError::Stop => return Err(e.context(format!("hook \"{}\" failed", step.label()))),
                OnError::Continue => println!("\x1b[1;33mhook \"{}\" failed, continuing:\x1b[0m {e:#}", step.label()),
            }
        }
    }
    Ok(())
}

fn run_step(cfg: &Config, phase: Phase, i: usize, step: &HookStep, source: &Source, logger: &Logger) -> Result<()> {
    let inside = match phase {
        Phase::PreInstall => false,
        Phase::PostInstall => step.chroot,
        Phase::PostSetup => true,
    };
    let script = step.script.as_ref().map(|s| source.fetch(s)).transpose()?;

    if !inside {
        let mut shell = Shell::new("Hook", logger);
        match (&step.run, script) {
            (Some(run), _) => shell.run_and_wait_with_args("bash", &format!("-ec {}", quote(run)))?,
            (None, Some(local)) => {
                fs::set_permissions(&local, fs::Permissions::from_mode(0o755))?;
                shell.run_and_wait_with_args(&local.display().to_string(), "")?
            }
            (None, None) => return Ok(()),
        };
        return Ok(());
    }

    let default_user = match phase {
        Phase::PostSetup => cfg.main_user().map(|u| u.name.as_str()).unwrap_or("root"),
        _ => "root",
    };
    let user = step.run_as.as_deref().unwrap_or(default_user);
    let mut target = Target::new(logger);
    match (&step.run, script) {
        (Some(run), _) => target.run_as(user, "env", &format!("LAZY_ROOT=/ bash -ec {}", quote(run))),
        (None, Some(local)) => {
            // not /tmp: arch-chroot mounts a fresh tmpfs there
            let inside_path = format!("/var/tmp/2lazy4arch-hooks/{i}-{}", local.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default());
            target::copy(&local, &inside_path, 0o755).context("copying the script into the new system")?;
            target.run_as(user, "env", &format!("LAZY_ROOT=/ {inside_path}"))
        }
        (None, None) => Ok(()),
    }
}
