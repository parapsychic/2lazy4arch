use std::{collections::BTreeMap, fs, os::unix, process::Command};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use shell_iface::logger::Logger;
use shell_words::quote;

use crate::{
    config::{Config, User},
    pacman::Pacman,
    system::parse_locale_gen,
    target::{self, Target},
    utils::get_uuid_root,
};

#[derive(Clone, Copy, PartialEq, Debug, Default, Deserialize, Serialize)]
pub enum Bootloader {
    #[default]
    #[serde(rename = "grub")]
    Grub,
    #[serde(rename = "systemd-boot")]
    SystemDBoot,
}

#[derive(Clone, Copy, PartialEq, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SuperUserUtility {
    #[default]
    Sudo,
    Doas,
}

/// Makes the system at /mnt bootable and usable: everything from swap to users.
pub struct Essentials<'a> {
    target: Target<'a>,
    pacman: Pacman<'a>,
    pub bootloader: Bootloader,
    pub super_user_utility: SuperUserUtility,
}

impl<'a> Essentials<'a> {
    pub fn new<'b>(
        logger: &'b Logger,
        bootloader: Bootloader,
        super_user_utility: SuperUserUtility,
    ) -> Essentials<'b> {
        Essentials {
            target: Target::new(logger),
            pacman: Pacman::in_target(logger),
            bootloader,
            super_user_utility,
        }
    }

    /// Packages install() skipped because they don't exist.
    pub fn skipped(&self) -> &[String] {
        &self.pacman.skipped
    }

    /// Uncomments [multilib] in the target's pacman.conf.
    pub fn enable_multilib(&mut self) -> Result<()> {
        let conf = fs::read_to_string(target::path("/etc/pacman.conf"))?;
        let enabled = conf.replace("#[multilib]\n#Include = /etc/pacman.d/mirrorlist", "[multilib]\nInclude = /etc/pacman.d/mirrorlist");
        target::write("/etc/pacman.conf", &enabled)
    }

    /// Creates a swap file of `size` GB. None skips it.
    pub fn initialize_swap(&mut self, size: Option<u32>) -> Result<()> {
        let Some(size) = size else {
            self.target.log("Swap disabled.");
            return Ok(());
        };
        // allocates instead of writing zeroes like dd, so it's instant
        self.target.run("mkswap", &format!("-U clear --size {size}G --file /swapfile"))?;
        target::append("/etc/fstab", "/swapfile none swap defaults 0 0")
    }

    /// Expects a zone from zoneinfo, eg: Asia/Kolkata
    pub fn set_timezones(&mut self, timezone: &str) -> Result<()> {
        let localtime = target::path("/etc/localtime");
        let _ = fs::remove_file(&localtime);
        unix::fs::symlink(format!("/usr/share/zoneinfo/{timezone}"), localtime)?;
        self.target.run("hwclock", "--systohc")
    }

    /// Expects a locale like en_US.UTF-8; its charset comes from locale.gen.
    pub fn gen_locale(&mut self, locale: &str) -> Result<()> {
        let known = parse_locale_gen(&fs::read_to_string(target::path("/etc/locale.gen")).unwrap_or_default());
        let line = known
            .into_iter()
            .find(|l| l.split(' ').next() == Some(locale))
            .unwrap_or_else(|| format!("{locale} UTF-8"));
        target::append("/etc/locale.gen", &line)?;
        self.target.run("locale-gen", "")?;
        target::write("/etc/locale.conf", &format!("LANG={locale}\n"))
    }

    /// Sets the hostname, /etc/hosts and extra hosts lines (address -> names).
    pub fn set_hostname(&mut self, hostname: &str, hosts: &BTreeMap<String, Vec<String>>) -> Result<()> {
        target::write("/etc/hostname", &format!("{hostname}\n"))?;
        let mut lines = format!("127.0.0.1\tlocalhost\n::1\tlocalhost\n127.0.1.1\t{hostname}.localdomain\t{hostname}");
        for (address, names) in hosts {
            lines += &format!("\n{address}\t{}", names.join(" "));
        }
        target::append("/etc/hosts", &lines)
    }

    pub fn mkinitcpio(&mut self) -> Result<()> {
        self.target.run("mkinitcpio", "-P")
    }

    /// Keeps nouveau out of the initramfs so the proprietary NVIDIA driver can load.
    /// Run mkinitcpio afterwards.
    pub fn remove_kms_hook(&mut self) -> Result<()> {
        let conf = fs::read_to_string(target::path("/etc/mkinitcpio.conf"))?;
        target::write("/etc/mkinitcpio.conf", &conf.replace(" kms ", " "))
    }

    /// Installs packages() plus `extra_programs`, and enables `services`.
    pub fn install_essentials(&mut self, extra_programs: &[String], services: &[String]) -> Result<()> {
        let mut packages = self.packages();
        packages.extend(extra_programs.iter().map(String::as_str));
        // pacstrap already copied the ranked mirrorlist
        self.pacman.install(&packages)?;

        for service in ["NetworkManager", "bluetooth"].into_iter().chain(services.iter().map(String::as_str)) {
            self.target.enable(service)?;
        }
        Ok(())
    }

    /// The programs every install gets.
    pub fn packages(&self) -> Vec<&'static str> {
        let mut essential_packages = vec![
            "efibootmgr",
            "os-prober",
            "ntfs-3g",
            "networkmanager",
            "network-manager-applet",
            "wireless_tools",
            "wpa_supplicant",
            "dialog",
            "mtools",
            "dosfstools",
            "base-devel",
            "linux-headers",
            "bluez",
            "bluez-utils",
            "pipewire",
            "pipewire-pulse",
            "pipewire-jack",
            "pipewire-alsa",
            "wireplumber",
            "alsa-utils",
            "git",
            "cups",
        ];
        if let SuperUserUtility::Doas = self.super_user_utility {
            // sudo is always there, base-devel depends on it
            essential_packages.push("opendoas");
        }
        if let Bootloader::Grub = self.bootloader {
            essential_packages.push("grub");
        }
        essential_packages
    }

    /// Builds dwm, dmenu and st from suckless git into /usr/local, sources stay in
    /// /usr/local/src for editing config.h. Login goes through lightdm.
    pub fn install_dwm(&mut self) -> Result<()> {
        for tool in ["dwm", "dmenu", "st"] {
            let dir = format!("/usr/local/src/{tool}");
            let _ = fs::remove_dir_all(target::path(&dir));
            self.target.run("git", &format!("clone --depth 1 https://git.suckless.org/{tool} {dir}"))?;
            self.target.run("make", &format!("-C {dir} clean install"))?;
        }
        target::write(
            "/usr/share/xsessions/dwm.desktop",
            "[Desktop Entry]\nEncoding=UTF-8\nName=Dwm\nComment=the dynamic window manager\nExec=/usr/local/bin/dwm\nIcon=dwm\nType=XSession\n",
        )
    }

    pub fn install_bootloader(&mut self) -> Result<()> {
        match self.bootloader {
            Bootloader::Grub => {
                // os-prober puts other installed OSes in the menu
                target::append("/etc/default/grub", "GRUB_DISABLE_OS_PROBER=false")?;
                self.target.run("grub-install", "--target=x86_64-efi --efi-directory=/boot --bootloader-id=GRUB")?;
                self.target.run("grub-mkconfig", "-o /boot/grub/grub.cfg")
            }
            Bootloader::SystemDBoot => {
                // Secure Boot needs signing: https://wiki.archlinux.org/title/Systemd-boot#Signing_for_Secure_Boot
                self.target.run("bootctl", "install")?;
                self.target.enable("systemd-boot-update.service")?;
                target::write("/boot/loader/loader.conf", "default  arch.conf\ntimeout  4\nconsole-mode max\neditor   no\n")?;
                // No ucode initrd: mkinitcpio's microcode hook embeds it in the initramfs.
                // No fallback entry: Arch's preset doesn't build the fallback image anymore.
                let uuid = get_uuid_root(&target::path("/etc/fstab"))?;
                target::write(
                    "/boot/loader/entries/arch.conf",
                    &format!("title   Arch Linux\nlinux   /vmlinuz-linux\ninitrd  /initramfs-linux.img\noptions root=UUID={uuid} rw\n"),
                )
            }
        }
    }

    /// Plain password, crypt hash, or neither to lock the account.
    fn set_password(&mut self, user: &str, password: Option<&str>, hash: Option<&str>) -> Result<()> {
        match (password, hash) {
            (Some(password), _) => self.target.run_with_input("chpasswd", "", &format!("{user}:{password}\n")),
            (None, Some(hash)) => self.target.run_with_input("chpasswd", "-e", &format!("{user}:{hash}\n")),
            (None, None) => self.target.run("passwd", &format!("-l {}", quote(user))),
        }
    }

    /// Users with their groups, shells, passwords and SSH keys; root's password (or a
    /// locked root); wheel for sudo / doas.
    pub fn create_users(&mut self, cfg: &Config) -> Result<()> {
        for (i, user) in cfg.users.iter().enumerate() {
            let mut groups = user.groups.clone();
            if cfg.is_admin(i) {
                groups.push("wheel".into());
                if cfg.docker.is_some() {
                    groups.push("docker".into());
                }
            }
            for group in &groups {
                self.target.run("groupadd", &format!("-f {}", quote(group)))?;
            }
            let mut args = format!("-m -s {}", user.shell.path());
            if !groups.is_empty() {
                args += &format!(" -G {}", quote(&groups.join(",")));
            }
            if let Some(full_name) = &user.full_name {
                args += &format!(" -c {}", quote(full_name));
            }
            self.target.run("useradd", &format!("{args} {}", quote(&user.name)))?;
            self.set_password(&user.name, user.password.as_deref(), user.password_hash.as_deref())?;
            self.authorized_keys(user)?;
        }
        self.set_password("root", cfg.root_password.as_deref(), cfg.root_password_hash.as_deref())?;

        // sudo is always installed (base-devel), so wheel always gets it.
        target::write_mode("/etc/sudoers.d/10-wheel", "%wheel ALL=(ALL:ALL) ALL\n", 0o440)?;
        if let SuperUserUtility::Doas = self.super_user_utility {
            // doas rejects a config without a trailing newline
            target::write_mode("/etc/doas.conf", "permit setenv { XAUTHORITY LANG LC_ALL } persist :wheel as root\n", 0o400)?;
        }
        Ok(())
    }

    /// ~/.ssh/authorized_keys from ssh_keys and github.com/<user>.keys
    fn authorized_keys(&mut self, user: &User) -> Result<()> {
        let mut keys = user.ssh_keys.clone();
        if let Some(github) = &user.github {
            let output = Command::new("curl").args(["-fsSL", &format!("https://github.com/{github}.keys")]).output()?;
            if !output.status.success() {
                bail!("could not fetch the SSH keys of github.com/{github}");
            }
            keys.extend(String::from_utf8_lossy(&output.stdout).lines().filter(|l| !l.trim().is_empty()).map(String::from));
        }
        if keys.is_empty() {
            return Ok(());
        }
        let ssh = format!("{}/.ssh", user.home());
        target::write_mode(&format!("{ssh}/authorized_keys"), &(keys.join("\n") + "\n"), 0o600)?;
        fs::set_permissions(target::path(&ssh), unix::fs::PermissionsExt::from_mode(0o700))?;
        self.target.chown(&user.name, &ssh)
    }
}
