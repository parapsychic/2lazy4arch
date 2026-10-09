//! `docker:` Images can't be pulled during the install, so a one-time service logs
//! in, pulls and starts everything on first boot, then deletes itself (and the
//! registry passwords with it).

use std::{path::Path, process::Command};

use anyhow::{bail, Result};
use serde_json::{Map, Value};
use shell_words::quote;

use crate::{config::Config, source::Source, target};

const DIR: &str = "/etc/2lazy4arch/docker";

/// compose.yaml from `containers`, with restart: unless-stopped where it's left out.
pub fn compose_file(docker: &crate::config::Docker) -> Value {
    let services: Map<String, Value> = docker
        .containers
        .iter()
        .map(|(name, service)| {
            let mut service = service.clone();
            if let Some(map) = service.as_object_mut() {
                map.entry("restart").or_insert_with(|| "unless-stopped".into());
            }
            (name.clone(), service)
        })
        .collect();
    let mut compose = Map::new();
    compose.insert("services".into(), Value::Object(services));
    for (key, section) in [("volumes", &docker.volumes), ("networks", &docker.networks)] {
        if !section.is_empty() {
            compose.insert(key.into(), serde_json::to_value(section).unwrap_or_default());
        }
    }
    Value::Object(compose)
}

/// Returns the unit to enable, if Docker is configured.
pub fn configure(cfg: &Config, source: &Source) -> Result<Vec<String>> {
    let Some(docker) = &cfg.docker else { return Ok(vec![]) };
    let mut compose_paths = vec![];

    if !docker.containers.is_empty() {
        let path = format!("{DIR}/compose.yaml");
        target::write_mode(&path, &serde_norway::to_string(&compose_file(docker))?, 0o600)?;
        compose_paths.push(path);
    }

    for (i, file) in docker.compose_files.iter().enumerate() {
        let local = source.fetch(file)?;
        let name = local.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or("compose.yaml".into());
        let stack = format!("{DIR}/stacks/{i}");
        if source.is_remote(file) {
            target::copy(&local, &format!("{stack}/{name}"), 0o600)?;
        } else {
            // the whole folder: build contexts and bind mounts are relative to it
            let folder = local.parent().unwrap_or(Path::new("."));
            std::fs::create_dir_all(target::path(&stack))?;
            let status = Command::new("cp").arg("-a").arg(folder.join(".")).arg(target::path(&stack)).status()?;
            if !status.success() {
                bail!("could not copy {}", folder.display());
            }
        }
        compose_paths.push(format!("{stack}/{name}"));
    }

    let mut script = String::from("#!/bin/bash\n# 2lazy4arch: first boot only, removes itself\nset -e\n");
    for login in &docker.login {
        script += &format!(
            "printf '%s' {} | docker login {} -u {} --password-stdin\n",
            quote(&login.password),
            quote(&login.registry),
            quote(&login.username)
        );
    }
    for path in &compose_paths {
        script += &format!("docker compose -f {path} up -d\n");
    }
    script += "systemctl disable 2lazy4arch-docker.service\nrm -f \"$0\"\n";
    target::write_mode(&format!("{DIR}/first-boot.sh"), &script, 0o700)?;
    target::write(
        "/etc/systemd/system/2lazy4arch-docker.service",
        &format!(
            "[Unit]\nDescription=Pull and start containers (2lazy4arch, first boot)\nRequires=docker.service\nAfter=docker.service network-online.target\nWants=network-online.target\nConditionPathExists={DIR}/first-boot.sh\n\n[Service]\nType=oneshot\nExecStart={DIR}/first-boot.sh\nTimeoutStartSec=0\n\n[Install]\nWantedBy=multi-user.target\n"
        ),
    )?;
    Ok(vec!["2lazy4arch-docker".into()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restart_is_added_when_left_out() {
        let docker: crate::config::Docker = serde_norway::from_str(
            "containers: { kuma: { image: louislam/uptime-kuma:1 }, db: { image: postgres:17, restart: always } }\nvolumes: { pgdata: {} }",
        )
        .unwrap();
        let compose = compose_file(&docker);
        assert_eq!(compose["services"]["kuma"]["restart"], "unless-stopped");
        assert_eq!(compose["services"]["db"]["restart"], "always");
        assert!(compose["volumes"]["pgdata"].is_object());
        assert!(compose.get("networks").is_none());
    }
}
