//! Checks that run before anything touches the disks.

use serde_json::Value;

use crate::{
    config::{Config, ConnectionType, Desktop, MIRROR_COUNTRIES},
    source::Source,
    storage,
    system::System,
};

pub const SCHEMA: &str = include_str!("../../unattended-config-schema.json");

/// Where the config breaks the schema, one line each.
pub fn schema_errors(config: &Value) -> Vec<String> {
    let schema: Value = serde_json::from_str(SCHEMA).expect("the bundled schema is valid JSON");
    let validator = jsonschema::validator_for(&schema).expect("the bundled schema compiles");
    validator
        .iter_errors(config)
        .map(|e| {
            let path = e.instance_path().to_string();
            let path = path.trim_start_matches('/');
            format!("{}: {e}", if path.is_empty() { "config" } else { path })
        })
        .collect()
}

/// Files the config names, relative to it.
pub fn files_named(cfg: &Config) -> Vec<&str> {
    let hooks = [&cfg.hooks.pre_install, &cfg.hooks.post_install, &cfg.hooks.post_setup];
    let mut files: Vec<&str> = hooks.into_iter().flatten().filter_map(|s| s.script.as_deref()).collect();
    files.extend(cfg.packages.pacman_list.as_deref());
    files.extend(cfg.packages.aur_list.as_deref());
    for eap in cfg.network.connections.iter().filter_map(|c| c.eap.as_ref()) {
        files.extend([&eap.ca_cert, &eap.client_cert, &eap.private_key].into_iter().filter_map(|f| f.as_deref()));
    }
    if let Some(docker) = &cfg.docker {
        files.extend(docker.compose_files.iter().map(String::as_str));
    }
    files
}

/// Why this config wouldn't work on this machine, one reason each.
pub fn system_errors(cfg: &Config, sys: &System, source: &Source) -> Vec<String> {
    let mut errors = vec![];
    if !sys.uefi {
        errors.push("the machine booted in BIOS mode, and 2lazy4arch only installs on UEFI".into());
    }
    if let Err(storage_errors) = storage::plan(&cfg.storage, sys) {
        errors.extend(storage_errors);
    }

    // Users
    let names: Vec<&str> = cfg.users.iter().map(|u| u.name.as_str()).collect();
    if names.is_empty() {
        errors.push("users: at least one user is needed".into());
    }
    for (i, name) in names.iter().enumerate() {
        if *name == "root" {
            errors.push("users: root can't be listed, use root_password".into());
        } else if names[..i].contains(name) {
            errors.push(format!("users: {name} is listed twice"));
        }
    }
    if !names.is_empty() && cfg.admins().is_empty() && cfg.root_password.is_none() && cfg.root_password_hash.is_none() {
        errors.push("nobody could run this machine: make a user an admin, or set a root password".into());
    }
    let mut must_exist = |name: &str, what: &str, root_ok: bool| {
        if !names.contains(&name) && !(root_ok && name == "root") {
            errors.push(format!("{what} names {name}, who isn't in users"));
        }
    };
    if let Some(user) = &cfg.autologin {
        must_exist(user, "autologin", false);
    }
    for user in cfg.autostart.apps.iter().filter_map(|a| a.users.as_ref()).flatten() {
        must_exist(user, "autostart.apps", false);
    }
    for (phase, steps) in [("hooks.post_install", &cfg.hooks.post_install), ("hooks.post_setup", &cfg.hooks.post_setup)] {
        for user in steps.iter().filter_map(|s| s.run_as.as_deref()) {
            must_exist(user, phase, true);
        }
    }
    for user in cfg.rice_users() {
        must_exist(&user, "parapsychic_mode", false);
    }

    // Remote access
    if !cfg.tailscale_only_ports().is_empty() && cfg.tailscale().is_none() {
        errors.push("listen: tailscale needs remote.tailscale turned on".into());
    }
    if cfg.vnc().is_some() && cfg.desktop == Desktop::None {
        errors.push("remote.vnc needs a desktop to show".into());
    }
    if cfg.vnc().is_some_and(|v| v.password.is_none()) {
        errors.push("remote.vnc needs a password".into());
    }

    // Values that have to exist on this ISO
    if cfg.mirrors != "auto" && !MIRROR_COUNTRIES.contains(&cfg.mirrors.as_str()) {
        errors.push(format!("mirrors: {} isn't a country reflector knows", cfg.mirrors));
    }
    if !sys.timezones.is_empty() && !sys.timezones.contains(&cfg.timezone) {
        errors.push(format!("timezone: there's no {}", cfg.timezone));
    }
    if !sys.locales.is_empty() && !sys.locales.iter().any(|l| l.split(' ').next() == Some(&cfg.locale)) {
        errors.push(format!("locale: there's no {}", cfg.locale));
    }
    for c in &cfg.network.connections {
        let wifi = c.kind == ConnectionType::Wifi;
        let candidates = if wifi { &sys.wireless } else { &sys.interfaces };
        if wifi && sys.wireless.is_empty() {
            errors.push(format!("network: {} is wifi, but this machine has no wifi", c.name));
        } else if let Some(interface) = c.interface.as_ref().filter(|i| !candidates.contains(i)) {
            errors.push(format!("network: {} uses {interface}, which this machine doesn't have", c.name));
        }
    }

    for file in files_named(cfg) {
        if let Err(e) = source.fetch(file) {
            errors.push(format!("{file}: {e}"));
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::BlockDevice;

    #[test]
    fn example_config_follows_the_schema() {
        let value: Value = serde_norway::from_str(include_str!("../../unattended-config.yaml")).unwrap();
        assert_eq!(schema_errors(&value), Vec::<String>::new());

        let mut broken = value;
        broken["desktop"] = "plan9".into();
        broken["users"][0]["name"] = "Root!".into();
        let errors = schema_errors(&broken);
        assert!(errors.iter().any(|e| e.starts_with("desktop:")), "{errors:?}");
        assert!(errors.iter().any(|e| e.starts_with("users/0/name:")), "{errors:?}");
    }

    #[test]
    fn system_checks() {
        let yaml = r#"
version: 1
storage: { partitioning: [ { disk: auto, wipe: true } ] }
users: [ { name: arch, password: x, admin: false } ]
autologin: ghost
desktop: none
mirrors: Atlantis
remote: { vnc: { enable: true, password: x, listen: tailscale } }
network: { connections: [ { name: home, type: wifi, ssid: Home } ] }
hooks: { pre_install: [ { script: ./missing.sh } ] }
"#;
        let value: Value = serde_norway::from_str(yaml).unwrap();
        assert_eq!(schema_errors(&value), Vec::<String>::new());
        let cfg: Config = serde_json::from_value(value).unwrap();
        let sys = System {
            uefi: true,
            devices: vec![BlockDevice { path: "/dev/vda".into(), kind: "disk".into(), size: Some(64 << 30), ..Default::default() }],
            timezones: vec!["UTC".into()],
            locales: vec!["en_US.UTF-8 UTF-8".into()],
            ..Default::default()
        };
        let errors = system_errors(&cfg, &sys, &Source::Dir(std::env::temp_dir()));
        for expected in [
            "nobody could run this machine",
            "autologin names ghost",
            "listen: tailscale needs",
            "remote.vnc needs a desktop",
            "mirrors: Atlantis",
            "this machine has no wifi",
            "./missing.sh",
        ] {
            assert!(errors.iter().any(|e| e.contains(expected)), "no {expected:?} in {errors:#?}");
        }
        assert_eq!(errors.len(), 7, "{errors:#?}");
    }
}
