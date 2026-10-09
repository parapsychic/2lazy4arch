//! `remote:` SSH, VNC (the server depends on the desktop), Tailscale, and the
//! firewall rule behind `listen: tailscale`.

use anyhow::Result;
use shell_words::quote;

use crate::{
    config::{Config, Desktop},
    session::xdg_autostart,
    target::{self, Target},
};

/// Sets everything up; returns units to enable.
pub fn configure(cfg: &Config, target: &mut Target) -> Result<Vec<String>> {
    let mut units = vec![];
    ssh(cfg)?;
    units.extend(vnc(cfg, target)?);
    units.extend(tailscale(cfg)?);
    firewall(cfg)?;
    Ok(units)
}

fn ssh(cfg: &Config) -> Result<()> {
    let Some(ssh) = cfg.ssh() else { return Ok(()) };
    let mut conf = format!("Port {}\nPermitRootLogin {}\n", ssh.port, if ssh.root_login { "yes" } else { "no" });
    let with_keys: Vec<&str> = cfg.users.iter().filter(|u| u.has_keys()).map(|u| u.name.as_str()).collect();
    let password_login = ssh.password_login.unwrap_or(with_keys.len() < cfg.users.len());
    conf += &format!("PasswordAuthentication {}\n", if password_login { "yes" } else { "no" });
    target::write("/etc/ssh/sshd_config.d/10-2lazy4arch.conf", &conf)?;
    // Default: users with keys need them. A Match block has to be the last thing
    // sshd reads, so it goes at the end of sshd_config, not in the drop-in.
    if ssh.password_login.is_none() && password_login && !with_keys.is_empty() {
        target::append("/etc/ssh/sshd_config", &format!("\n# 2lazy4arch: these users log in with keys only\nMatch User {}\n    PasswordAuthentication no", with_keys.join(",")))?;
    }
    Ok(())
}

/// KDE's KStringHandler::obscure, how krfb stores passwords in its config.
fn kde_obscure(s: &str) -> String {
    s.chars().map(|c| if (c as u32) <= 0x21 { c } else { char::from_u32(0x1001F - c as u32).unwrap_or(c) }).collect()
}

/// Session commands a user's Hyprland runs for VNC.
pub fn wayvnc_command(cfg: &Config) -> Option<String> {
    let vnc = cfg.vnc().filter(|_| cfg.desktop == Desktop::Hyprland)?;
    Some(if vnc.view_only { "wayvnc -d".into() } else { "wayvnc".into() })
}

fn vnc(cfg: &Config, target: &mut Target) -> Result<Vec<String>> {
    let Some(vnc) = cfg.vnc() else { return Ok(vec![]) };
    let password = vnc.password.clone().unwrap_or_default();
    let port = vnc.port;
    match cfg.desktop {
        // one server for the display, login screen included
        Desktop::Dwm | Desktop::Xfce | Desktop::Lxde => {
            // the password goes in on stdin, so it never shows up in the install's output
            target.run_with_input("sh", "-c 'read -r pw && x11vnc -storepasswd \"$pw\" /etc/x11vnc.pass'", &format!("{password}\n"))?;
            let view_only = if vnc.view_only { " -viewonly" } else { "" };
            target::write(
                "/etc/systemd/system/2lazy4arch-x11vnc.service",
                &format!(
                    "[Unit]\nDescription=VNC server for the X display (2lazy4arch)\nAfter=display-manager.service\n\n[Service]\nExecStart=/usr/bin/x11vnc -display :0 -auth guess -forever -loop -noxdamage -repeat -shared -rfbauth /etc/x11vnc.pass -rfbport {port}{view_only}\nRestart=on-failure\nRestartSec=3\n\n[Install]\nWantedBy=graphical.target\n"
                ),
            )?;
            return Ok(vec!["2lazy4arch-x11vnc".into()]);
        }
        // per session: wayvnc with a username/password over RSA-AES; started by session::autostart_apps
        Desktop::Hyprland => {
            for user in &cfg.users {
                let dir = format!("{}/.config/wayvnc", user.home());
                target::write_mode(
                    &format!("{dir}/config"),
                    &format!("address=0.0.0.0\nport={port}\nenable_auth=true\nusername={}\npassword={password}\nrsa_private_key_file={dir}/rsa_key.pem\n", user.name),
                    0o600,
                )?;
                target.run("openssl", &format!("genrsa -traditional -out {dir}/rsa_key.pem 2048"))?;
            }
        }
        // per session: grdctl needs the user's session (and keyring), so a script
        // does it at first login and removes itself
        Desktop::Gnome => {
            for user in &cfg.users {
                let home = user.home();
                let script = format!("{home}/.local/share/2lazy4arch/vnc-setup.sh");
                let view_only = if vnc.view_only { "enable-view-only" } else { "disable-view-only" };
                let set_port = if port == 5900 { String::new() } else { format!("grdctl vnc set-port {port} || true\n") };
                target::write_mode(
                    &script,
                    &format!(
                        "#!/bin/sh\nset -e\ngrdctl vnc set-auth-method password\ngrdctl vnc set-password {}\ngrdctl vnc {view_only}\n{set_port}grdctl vnc enable\nsystemctl --user enable --now gnome-remote-desktop.service\nrm -f \"$HOME/.config/autostart/2lazy4arch-vnc.desktop\" \"$0\"\n",
                        quote(&password)
                    ),
                    0o700,
                )?;
                xdg_autostart(&home, "2lazy4arch-vnc", &script)?;
            }
        }
        // per session: krfb with unattended access, password in its config instead of KWallet
        Desktop::Kde => {
            for user in &cfg.users {
                let home = user.home();
                target::write_mode(
                    &format!("{home}/.config/krfbrc"),
                    &format!(
                        "[MainWindow]\nstartMinimized=true\n\n[Security]\nallowDesktopControl={}\nallowUnattendedAccess=true\nnoWallet=true\nunattendedPassword={}\n\n[TCP]\nport={port}\nuseDefaultPort={}\n",
                        !vnc.view_only,
                        kde_obscure(&password),
                        port == 5900
                    ),
                    0o600,
                )?;
                xdg_autostart(&home, "krfb", "krfb --nodialog")?;
            }
        }
        Desktop::None => {}
    }
    Ok(vec![])
}

fn tailscale(cfg: &Config) -> Result<Vec<String>> {
    let Some(ts) = cfg.tailscale() else { return Ok(vec![]) };
    if ts.advertise_exit_node || !ts.advertise_routes.is_empty() {
        target::write("/etc/sysctl.d/99-2lazy4arch-tailscale.conf", "net.ipv4.ip_forward = 1\nnet.ipv6.conf.all.forwarding = 1\n")?;
    }
    let Some(key) = &ts.auth_key else { return Ok(vec![]) };
    let mut args = format!("--hostname={}", ts.hostname.as_ref().unwrap_or(&cfg.hostname));
    for (on, flag) in [(ts.ssh, " --ssh"), (ts.accept_routes, " --accept-routes"), (ts.advertise_exit_node, " --advertise-exit-node")] {
        if on {
            args += flag;
        }
    }
    if !ts.advertise_routes.is_empty() {
        args += &format!(" --advertise-routes={}", ts.advertise_routes.join(","));
    }
    let key_file = "/etc/2lazy4arch/tailscale-authkey";
    target::write_mode(key_file, key, 0o600)?;
    target::write(
        "/etc/systemd/system/2lazy4arch-tailscale-up.service",
        &format!(
            "[Unit]\nDescription=Join the tailnet (2lazy4arch, first boot)\nRequires=tailscaled.service\nAfter=tailscaled.service network-online.target\nWants=network-online.target\nConditionPathExists={key_file}\n\n[Service]\nType=oneshot\nExecStart=/bin/sh -c 'tailscale up --auth-key=\"$(cat {key_file})\" {args} && rm -f {key_file}'\n\n[Install]\nWantedBy=multi-user.target\n"
        ),
    )?;
    Ok(vec!["2lazy4arch-tailscale-up".into()])
}

/// `listen: tailscale`: those ports only answer on the Tailscale interface.
fn firewall(cfg: &Config) -> Result<()> {
    let ports = cfg.tailscale_only_ports();
    if ports.is_empty() {
        return Ok(());
    }
    let ports: Vec<String> = ports.iter().map(u16::to_string).collect();
    target::write(
        "/etc/nftables.conf",
        &format!(
            "#!/usr/bin/nft -f\n# 2lazy4arch: these ports only answer on Tailscale\n\ndestroy table inet lazy_listen\ntable inet lazy_listen {{\n  chain input {{\n    type filter hook input priority filter - 10; policy accept;\n    iifname {{ \"lo\", \"tailscale0\" }} accept\n    tcp dport {{ {} }} drop\n  }}\n}}\n",
            ports.join(", ")
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obscure_round_trips() {
        let hidden = kde_obscure("hunter2!");
        assert_ne!(hidden, "hunter2!");
        assert_eq!(kde_obscure(&hidden), "hunter2!");
    }
}
