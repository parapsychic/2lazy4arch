use std::process::Command;

use anyhow::{bail, Result};
use nix::unistd::Uid;
use shell_iface::{logger::Logger, Shell};

enum PackageManager {
    Pacman,
    Yay,
}

pub struct Pacman<'a> {
    shell: Shell<'a>,
    /// specifies whether this is running on the live/installer environment
    /// or the acutal installed machine
    is_non_root: bool,
    program: PackageManager,
    /// Packages install() skipped because they don't exist.
    pub skipped: Vec<String>,
}

impl<'a> Pacman<'a> {
    pub fn new<'b>(logger: &'b Logger) -> Pacman<'b> {
        let is_non_root = !Uid::effective().is_root();

        let shell = Shell::new("Pacman", logger);
        Pacman {
            shell,
            is_non_root,
            program: PackageManager::Pacman,
            skipped: vec![],
        }
    }

    pub fn yay(&mut self) -> &mut Self {
        self.program = PackageManager::Yay;
        self
    }

    pub fn pacman(&mut self) -> &mut Self {
        self.program = PackageManager::Pacman;
        self
    }

    /// pacman runs through sudo when we're not root; yay must not be root and calls sudo itself.
    fn run(&mut self, args: &str) -> Result<()> {
        match (&self.program, self.is_non_root) {
            (PackageManager::Pacman, false) => self.shell.run_and_wait_with_args("pacman", args)?,
            (PackageManager::Pacman, true) => {
                self.shell.run_and_wait_with_args("sudo", &format!("pacman {args}"))?
            }
            (PackageManager::Yay, true) => self.shell.run_and_wait_with_args("yay", args)?,
            (PackageManager::Yay, false) => {
                self.shell.log("ERROR: Called YAY as root.");
                bail!("yay can't run as root");
            }
        };
        Ok(())
    }

    pub fn update_mirrors(&mut self) -> Result<()> {
        self.run("-Syy --noconfirm")
    }

    /// Installs what exists and skips (and reports) what doesn't, instead of
    /// failing the whole transaction on one bad name.
    pub fn install(&mut self, packages: &[&str]) -> Result<()> {
        if packages.is_empty() {
            return Ok(());
        }
        // fresh databases so the check is accurate; the -Su below completes the -Syu
        self.run("-Sy --noconfirm")?;
        let packages = self.keep_available(packages)?;
        if packages.is_empty() {
            return Ok(());
        }
        let packages = packages.join(" ");
        self.shell.log(&format!("Installing {packages}."));
        self.run(&format!("-Su --needed --noconfirm {packages}"))
    }

    /// Drops the packages that don't exist: not in the repos, or for yay, not in the AUR either.
    /// Prints them and remembers them in `skipped`. Sync the databases first.
    pub fn keep_available<'p>(&mut self, packages: &[&'p str]) -> Result<Vec<&'p str>> {
        let mut missing = self.missing_from_repos(packages)?;
        let place = match self.program {
            PackageManager::Pacman => "in the repos",
            PackageManager::Yay => {
                missing = self.missing_from_aur(missing);
                "in the repos or the AUR"
            }
        };
        if !missing.is_empty() {
            println!("\x1b[1;33mNot found {place}, skipping:\x1b[0m {}", missing.join(" "));
            self.skipped.extend(missing.iter().cloned());
        }
        Ok(packages.iter().copied().filter(|p| !missing.iter().any(|m| m == p)).collect())
    }

    /// Names pacman can't resolve from the synced databases. Groups and provides
    /// (e.g. libva-mesa-driver -> mesa) count as found, same as for `pacman -S`.
    pub fn missing_from_repos(&mut self, packages: &[&str]) -> Result<Vec<String>> {
        if packages.is_empty() {
            return Ok(vec![]);
        }
        // Not through Shell: we need stderr from a failing run. One call reports every missing target.
        let output = Command::new("pacman")
            .env("LC_ALL", "C")
            .args(["-Sp", "--print-format", "%n", "--"])
            .args(packages)
            .output()?;
        Ok(String::from_utf8_lossy(&output.stderr)
            .lines()
            .filter_map(|l| l.strip_prefix("error: target not found: "))
            .map(String::from)
            .collect())
    }

    /// Of `packages`, the ones the AUR doesn't have either. If the AUR can't be
    /// reached, nothing is skipped and yay gets to try.
    fn missing_from_aur(&mut self, packages: Vec<String>) -> Vec<String> {
        if packages.is_empty() {
            return packages;
        }
        let query = packages
            .iter()
            .map(|p| format!("arg[]={}", p.replace('+', "%2B")))
            .collect::<Vec<_>>()
            .join("&");
        let response = self
            .shell
            .run_with_args("curl", &format!("-gfsS --max-time 30 https://aur.archlinux.org/rpc/v5/info?{query}"))
            .ok()
            .and_then(|out| serde_json::from_slice::<serde_json::Value>(&out.stdout).ok());
        let Some(response) = response else {
            println!("Couldn't reach the AUR to check {}, letting yay try.", packages.join(" "));
            return vec![];
        };
        let found: Vec<&str> = response["results"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|r| r["Name"].as_str())
            .collect();
        packages.into_iter().filter(|p| !found.contains(&p.as_str())).collect()
    }

    /// Ranks the country's mirrors. Live environment only (it ships reflector, we're root).
    pub fn run_reflector(&mut self, country: &str) -> Result<()> {
        // --latest keeps `--sort rate` from benchmarking every mirror in big countries
        self.shell.run_and_wait_with_args(
            "reflector",
            &format!("-c '{country}' --protocol https --latest 20 --sort rate --save /etc/pacman.d/mirrorlist"),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs Arch with synced databases and network: cargo test -- --ignored
    #[test]
    #[ignore]
    fn reports_only_missing_packages() {
        let logger = Logger::new(false);
        let mut pacman = Pacman::new(&logger);
        // a package, a group, a provide, an AUR package, a typo
        let wanted = ["firefox", "gnome", "libva-mesa-driver", "brave-bin", "not-a-package-xyz"];
        assert_eq!(pacman.missing_from_repos(&wanted).unwrap(), ["brave-bin", "not-a-package-xyz"]);
        assert_eq!(pacman.yay().keep_available(&wanted).unwrap(), ["firefox", "gnome", "libva-mesa-driver", "brave-bin"]);
        assert_eq!(pacman.skipped, ["not-a-package-xyz"]);
    }
}
