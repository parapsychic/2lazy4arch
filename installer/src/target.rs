//! The system being installed, mounted at /mnt. Files are written from the ISO
//! through ROOT; commands run inside it with arch-chroot.

use std::{fs, os::unix::fs::PermissionsExt, path::Path};

use anyhow::Result;
use shell_iface::{logger::Logger, Shell};
use shell_words::quote;

pub const ROOT: &str = "/mnt";

/// "/etc/hostname" -> "/mnt/etc/hostname"
pub fn path(p: &str) -> String {
    format!("{ROOT}{p}")
}

/// Writes a file in the target, creating its folders.
pub fn write(p: &str, content: &str) -> Result<()> {
    let full = path(p);
    if let Some(parent) = Path::new(&full).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&full, content)?;
    Ok(())
}

/// write(), then chmod.
pub fn write_mode(p: &str, content: &str, mode: u32) -> Result<()> {
    write(p, content)?;
    fs::set_permissions(path(p), fs::Permissions::from_mode(mode))?;
    Ok(())
}

/// Copies a local file into the target with `mode`.
pub fn copy(local: &Path, p: &str, mode: u32) -> Result<()> {
    let full = path(p);
    if let Some(parent) = Path::new(&full).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(local, &full)?;
    fs::set_permissions(&full, fs::Permissions::from_mode(mode))?;
    Ok(())
}

/// Appends a line block, starting on a new line.
pub fn append(p: &str, content: &str) -> Result<()> {
    let existing = fs::read_to_string(path(p)).unwrap_or_default();
    let sep = if existing.is_empty() || existing.ends_with('\n') { "" } else { "\n" };
    write(p, &format!("{existing}{sep}{}\n", content.trim_end_matches('\n')))
}

pub struct Target<'a> {
    shell: Shell<'a>,
}

impl<'a> Target<'a> {
    pub fn new<'b>(logger: &'b Logger) -> Target<'b> {
        Target { shell: Shell::new("Target", logger) }
    }

    pub fn log(&self, msg: &str) {
        self.shell.log(msg);
    }

    /// `cmd args` inside the target, as root.
    pub fn run(&mut self, cmd: &str, args: &str) -> Result<()> {
        self.shell.run_and_wait_with_args("arch-chroot", &format!("{ROOT} {cmd} {args}"))?;
        Ok(())
    }

    /// `cmd args` inside the target as `user`, with their HOME.
    pub fn run_as(&mut self, user: &str, cmd: &str, args: &str) -> Result<()> {
        if user == "root" {
            return self.run(cmd, args);
        }
        let user = quote(user);
        self.shell.run_and_wait_with_args(
            "arch-chroot",
            &format!("-u {user} {ROOT} /usr/bin/env HOME=/home/{user} USER={user} LOGNAME={user} {cmd} {args}"),
        )?;
        Ok(())
    }

    /// A bash -e snippet inside the target as `user`.
    pub fn bash(&mut self, user: &str, script: &str) -> Result<()> {
        self.run_as(user, "bash", &format!("-ec {}", quote(script)))
    }

    /// `cmd args` inside the target with `input` on stdin.
    pub fn run_with_input(&mut self, cmd: &str, args: &str, input: &str) -> Result<()> {
        self.shell.run_with_input("arch-chroot", &format!("{ROOT} {cmd} {args}"), input)
    }

    pub fn enable(&mut self, unit: &str) -> Result<()> {
        self.run("systemctl", &format!("enable {}", quote(unit)))
    }

    /// Hands a home folder (or a path in it) to its user.
    pub fn chown(&mut self, user: &str, p: &str) -> Result<()> {
        self.run("chown", &format!("-R {}: {}", quote(user), quote(p)))
    }
}
