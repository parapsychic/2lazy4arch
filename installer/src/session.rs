//! What happens at login: autologin per login manager, and apps (plus VNC servers
//! that live in the session) started for each user, in each desktop's own way.

use std::{fs, os::unix, path::Path};

use anyhow::Result;
use shell_words::quote;

use crate::{
    config::{Config, Desktop},
    target::{self, Target},
};

/// Logs `autologin` straight in at boot.
pub fn autologin(cfg: &Config, target: &mut Target) -> Result<()> {
    let Some(user) = &cfg.autologin else { return Ok(()) };
    match cfg.desktop {
        Desktop::Gnome => target::write("/etc/gdm/custom.conf", &format!("[daemon]\nAutomaticLoginEnable=True\nAutomaticLogin={user}\n")),
        Desktop::Hyprland => target::write("/etc/sddm.conf.d/autologin.conf", &format!("[Autologin]\nUser={user}\nSession=hyprland.desktop\n")),
        Desktop::Kde => target::write("/etc/plasmalogin.conf.d/autologin.conf", &format!("[Autologin]\nUser={user}\nSession=plasma.desktop\n")),
        Desktop::Dwm | Desktop::Xfce => {
            // lightdm only autologins members of the autologin group
            target.run("groupadd", "-rf autologin")?;
            target.run("gpasswd", &format!("-a {} autologin", quote(user)))?;
            let session = if cfg.desktop == Desktop::Dwm { "dwm" } else { "xfce" };
            target::write("/etc/lightdm/lightdm.conf.d/50-autologin.conf", &format!("[Seat:*]\nautologin-user={user}\nautologin-session={session}\n"))
        }
        Desktop::Lxde => {
            let conf = fs::read_to_string(target::path("/etc/lxdm/lxdm.conf")).unwrap_or_else(|_| "[base]\n".into());
            let line = format!("autologin={user}");
            let updated = match conf.lines().find(|l| l.trim_start_matches(['#', ' ']).starts_with("autologin=")) {
                Some(old) => conf.replacen(old, &line, 1),
                None => conf.replacen("[base]", &format!("[base]\n{line}"), 1),
            };
            target::write("/etc/lxdm/lxdm.conf", &updated)
        }
        Desktop::None => target::write(
            "/etc/systemd/system/getty@tty1.service.d/autologin.conf",
            &format!("[Service]\nExecStart=\nExecStart=-/sbin/agetty -o '-p -f -- \\\\u' --noclear --autologin {user} %I $TERM\n"),
        ),
    }
}

/// ~/.config/autostart/<name>.desktop (KDE, GNOME, Xfce, LXDE)
pub fn xdg_autostart(home: &str, name: &str, exec: &str) -> Result<()> {
    target::write(
        &format!("{home}/.config/autostart/{name}.desktop"),
        &format!("[Desktop Entry]\nType=Application\nName={name}\nExec={}\nX-GNOME-Autostart-enabled=true\n", exec.replace('%', "%%")),
    )
}

/// Commands Hyprland runs at start, in ~/.config/hypr/2lazy4arch.lua, required
/// from the user's hyprland.lua (a copy of the default one if they have none).
pub fn hyprland_autostart(home: &str, commands: &[String]) -> Result<()> {
    let config = format!("{home}/.config/hypr/hyprland.lua");
    if !Path::new(&target::path(&config)).exists() {
        let default = fs::read_to_string(target::path("/usr/share/hypr/hyprland.lua")).unwrap_or_default();
        target::write(&config, &default)?;
    }
    let current = fs::read_to_string(target::path(&config))?;
    if !current.contains("require(\"2lazy4arch\")") {
        target::append(&config, "\nrequire(\"2lazy4arch\")")?;
    }
    let calls: String = commands.iter().map(|c| format!("  hl.exec_cmd([==[{c}]==])\n")).collect();
    target::write(&format!("{home}/.config/hypr/2lazy4arch.lua"), &format!("-- written by 2lazy4arch\nhl.on(\"hyprland.start\", function ()\n{calls}end)\n"))
}

/// A systemd user service started at login (desktop: none).
fn user_service(home: &str, name: &str, command: &str) -> Result<()> {
    let escaped = command.replace('\\', "\\\\").replace('"', "\\\"").replace('%', "%%");
    let dir = format!("{home}/.config/systemd/user");
    target::write(
        &format!("{dir}/{name}.service"),
        &format!("[Unit]\nDescription={name} (2lazy4arch autostart)\n\n[Service]\nExecStart=/bin/sh -c \"{escaped}\"\n\n[Install]\nWantedBy=default.target\n"),
    )?;
    let link = target::path(&format!("{dir}/default.target.wants/{name}.service"));
    fs::create_dir_all(Path::new(&link).parent().unwrap())?;
    let _ = fs::remove_file(&link);
    unix::fs::symlink(format!("../{name}.service"), link)?;
    Ok(())
}

/// autostart.apps for every user they apply to, plus `extra` session commands per
/// user (wayvnc). Home folders get chowned afterwards.
pub fn autostart_apps(cfg: &Config, extra: &dyn Fn(&str) -> Vec<String>) -> Result<()> {
    for user in &cfg.users {
        let home = user.home();
        let apps: Vec<(String, String)> = cfg
            .autostart
            .apps
            .iter()
            .filter(|a| a.users.as_ref().is_none_or(|users| users.contains(&user.name)))
            .map(|a| (a.name(), a.command.clone()))
            .collect();
        let extra = extra(&user.name);
        match cfg.desktop {
            Desktop::Hyprland => {
                let commands: Vec<String> = extra.into_iter().chain(apps.into_iter().map(|(_, c)| c)).collect();
                if !commands.is_empty() {
                    hyprland_autostart(&home, &commands)?;
                }
            }
            Desktop::Dwm => {
                if !apps.is_empty() {
                    // lightdm runs ~/.xprofile before dwm
                    let lines: Vec<String> = apps.iter().map(|(_, c)| format!("{c} &")).collect();
                    target::append(&format!("{home}/.xprofile"), &lines.join("\n"))?;
                }
            }
            Desktop::None => {
                for (name, command) in &apps {
                    user_service(&home, name, command)?;
                }
            }
            _ => {
                for (name, command) in &apps {
                    xdg_autostart(&home, name, command)?;
                }
            }
        }
    }
    Ok(())
}
