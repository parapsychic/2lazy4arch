use anyhow::{anyhow, bail, Result};
use shell_iface::{logger::Logger, Shell};
use std::{
    fs,
    os::unix::{self, fs::PermissionsExt},
};

use crate::{
    pacman::Pacman,
    utils::{append_to_file, get_uuid_root, write_to_file},
};

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Bootloader {
    #[default]
    Grub,
    SystemDBoot,
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum SuperUserUtility {
    #[default]
    Sudo,
    Doas,
}

/// Essentials basically installs arch to be a bootable/usable state.
/// This is same as the install.sh
/// Essentials must be the last to run before program exits.
/// Reason in chroot function
pub struct Essentials<'a> {
    is_chroot: bool,
    shell: Shell<'a>,
    pacman: Pacman<'a>,
    pub bootloader: Bootloader,
    pub super_user_utility: SuperUserUtility,
}

impl<'a> Essentials<'a> {
    /// Installs and sets up the system to be bootable and bare-minimum usable.
    /// Calling all functions on this struct is usually called the end of installation.
    /// Set the bootloader, Super user utility
    pub fn new<'b>(
        logger: &'b Logger,
        bootloader: Bootloader,
        super_user_utility: SuperUserUtility,
    ) -> Essentials<'b> {
        let shell = Shell::new("Essentials", logger);
        let pacman = Pacman::new(logger);

        Essentials {
            is_chroot: false,
            shell,
            pacman,
            bootloader,
            super_user_utility,
        }
    }

    fn ensure_chroot(&self, what: &str) -> Result<()> {
        if !self.is_chroot {
            self.shell.log(&format!("Cannot {what}. Not in chroot."));
            bail!("Cannot {what}. Not in chroot.");
        }
        Ok(())
    }

    /// chroot into the system
    /// It is imperative that this should be called first before executing any other fns.
    /// Instead of calling arch-chroot, chroot is being called directly.
    /// Followed instructions from [here](https://wiki.archlinux.org/title/Chroot#Using_chroot)
    /// Since I can't un-chroot once we're inside chroot,
    /// we need to make sure this struct's functions are run at the very end.
    /// This behavior is consistent with how chroot works in Unix-like systems:
    /// once a process is chrooted, it cannot simply "unchroot" itself.
    /// So, either refactor the whole code to include forking process,
    /// or just rely on this process to exit and
    /// thus send the user back to un-chrooted environment.
    pub fn chroot(&mut self) -> Result<()> {
        self.shell.log("Entering chroot.");
        self.shell.run_with_args("mount", "-t proc /proc /mnt/proc/")?;
        self.shell.run_with_args("mount", "-t sysfs /sys /mnt/sys/")?;
        self.shell.run_with_args("mount", "-o bind /dev /mnt/dev/")?;
        self.shell.run_with_args("mount", "-o bind /run /mnt/run/")?;
        self.shell.run_with_args(
            "mount",
            "-o bind /sys/firmware/efi/efivars /mnt/sys/firmware/efi/efivars/",
        )?;
        fs::copy("/etc/resolv.conf", "/mnt/etc/resolv.conf")?;
        unix::fs::chroot("/mnt")?;
        std::env::set_current_dir("/")?;
        self.is_chroot = true;

        self.shell.log("Entered chroot.");
        Ok(())
    }

    /// Creates a swap file of `size` GB. 0 skips it.
    pub fn initialize_swap(&mut self, size: usize) -> Result<()> {
        self.ensure_chroot("initialize swap")?;
        if size == 0 {
            self.shell.log("Swap disabled.");
            return Ok(());
        }

        self.shell.log(&format!("Creating a {size} GB swap file."));
        // allocates instead of writing zeroes like dd, so it's instant
        self.shell.run_and_wait_with_args(
            "mkswap",
            &format!("-U clear --size {size}G --file /swapfile"),
        )?;
        append_to_file("/etc/fstab", "/swapfile none swap defaults 0 0")
    }

    /// Sets the timezone.
    /// Expects a valid Timezone from zoneinfo, eg: Asia/Kolkata
    pub fn set_timezones(&mut self, timezone: &str) -> Result<()> {
        self.ensure_chroot("set timezone")?;
        self.shell.log("Synchronizing Timezones");
        let _ = fs::remove_file("/etc/localtime");
        unix::fs::symlink(format!("/usr/share/zoneinfo/{timezone}"), "/etc/localtime")?;
        self.shell.run_and_wait_with_args("hwclock", "--systohc")?;
        Ok(())
    }

    /// Generates locale.
    /// Expects a line from /etc/locale.gen, eg: "en_US.UTF-8 UTF-8"
    pub fn gen_locale(&mut self, locale: &str) -> Result<()> {
        self.ensure_chroot("set locale")?;
        self.shell.log("Generating Locale");

        let lang = locale.split_whitespace().next().ok_or_else(|| anyhow!("Empty locale"))?;
        append_to_file("/etc/locale.gen", locale)?;
        self.shell.run_and_wait("locale-gen")?;
        write_to_file("/etc/locale.conf", &format!("LANG={lang}\n"))
    }

    /// Sets the hostname and the hosts configuration
    pub fn set_hostname(&mut self, hostname: &str) -> Result<()> {
        self.ensure_chroot("set hostname")?;
        self.shell.log("Setting hostname");

        write_to_file("/etc/hostname", &format!("{hostname}\n"))?;
        append_to_file(
            "/etc/hosts",
            &format!("127.0.0.1\tlocalhost\n::1\tlocalhost\n127.0.1.1\t{hostname}.localdomain\t{hostname}"),
        )
    }

    /// Runs mkinicpio
    pub fn mkinitcpio(&mut self) -> Result<()> {
        self.ensure_chroot("run mkinitcpio")?;
        self.shell.run_and_wait_with_args("mkinitcpio", "-P")?;
        Ok(())
    }

    /// Keeps nouveau out of the initramfs so the proprietary NVIDIA driver can load.
    /// Run mkinitcpio afterwards.
    pub fn remove_kms_hook(&mut self) -> Result<()> {
        self.ensure_chroot("edit mkinitcpio.conf")?;
        let conf = fs::read_to_string("/etc/mkinitcpio.conf")?;
        write_to_file("/etc/mkinitcpio.conf", &conf.replace(" kms ", " "))
    }

    /// set up password
    pub fn set_password(&mut self, user: &str, password: &str) -> Result<()> {
        self.ensure_chroot("set password")?;
        self.shell.log(&format!("Setting password for {user}"));

        let status = self
            .shell
            .spawn_with_piped_input("chpasswd", &format!("{user}:{password}"))?
            .wait()?;
        if !status.success() {
            bail!("Could not set the password for {user}");
        }
        Ok(())
    }

    /// Installs packages() plus `extra_programs`, and enables `services`.
    pub fn install_essentials(&mut self, extra_programs: &[&str], services: &[&str]) -> Result<()> {
        self.ensure_chroot("install essential packages")?;
        self.shell.log("Starting essentials package install");

        let mut packages = self.packages();
        packages.extend(extra_programs);
        // pacstrap already copied the ranked mirrorlist
        self.pacman.install(&packages)?;

        self.shell.log("Enabling Services");
        for service in ["NetworkManager", "bluetooth"].iter().chain(services) {
            self.shell.run_and_wait_with_args("systemctl", &format!("enable {service}"))?;
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
        self.ensure_chroot("install dwm")?;
        for tool in ["dwm", "dmenu", "st"] {
            let dir = format!("/usr/local/src/{tool}");
            let _ = fs::remove_dir_all(&dir);
            self.shell.run_and_wait_with_args(
                "git",
                &format!("clone --depth 1 https://git.suckless.org/{tool} {dir}"),
            )?;
            self.shell.run_and_wait_with_args("make", &format!("-C {dir} clean install"))?;
        }
        fs::create_dir_all("/usr/share/xsessions")?;
        write_to_file(
            "/usr/share/xsessions/dwm.desktop",
            "[Desktop Entry]\nEncoding=UTF-8\nName=Dwm\nComment=the dynamic window manager\nExec=/usr/local/bin/dwm\nIcon=dwm\nType=XSession\n",
        )
    }

    pub fn install_bootloader(&mut self) -> Result<()> {
        match self.bootloader {
            Bootloader::Grub => self.install_grub(),
            Bootloader::SystemDBoot => self.install_systemdboot(),
        }
    }

    /// Installs and configures grub, with os-prober so other OSes show up.
    fn install_grub(&mut self) -> Result<()> {
        self.ensure_chroot("install grub")?;
        self.shell.log("Installing Grub as the Bootloader");

        append_to_file("/etc/default/grub", "GRUB_DISABLE_OS_PROBER=false\n")?;
        self.shell.run_and_wait_with_args(
            "grub-install",
            "--target=x86_64-efi --efi-directory=/boot --bootloader-id=GRUB",
        )?;
        self.shell.run_and_wait_with_args("grub-mkconfig", "-o /boot/grub/grub.cfg")?;
        Ok(())
    }

    /// Installs and configures systemd-boot
    /// Does not support Secure boot. TODO
    fn install_systemdboot(&mut self) -> Result<()> {
        self.ensure_chroot("install systemd-boot")?;
        self.shell.log("Installing SystemD Boot as the Bootloader");
        self.shell.log("This mode does not support secure boot. If you have secure boot installed, you might want to set up [signing the bootloader](https://wiki.archlinux.org/title/Systemd-boot#Signing_for_Secure_Boot).");

        self.shell.run_and_wait_with_args("bootctl", "install")?;
        self.shell
            .run_and_wait_with_args("systemctl", "enable systemd-boot-update.service")?;
        write_to_file(
            "/boot/loader/loader.conf",
            "default  arch.conf\ntimeout  4\nconsole-mode max\neditor   no\n",
        )?;

        // No ucode initrd: mkinitcpio's microcode hook embeds it in the initramfs.
        // No fallback entry: Arch's preset doesn't build the fallback image anymore.
        let uuid = get_uuid_root()?;
        write_to_file(
            "/boot/loader/entries/arch.conf",
            &format!("title   Arch Linux\nlinux   /vmlinuz-linux\ninitrd  /initramfs-linux.img\noptions root=UUID={uuid} rw\n"),
        )
    }

    /// Adds a new user, sets permissions and sets up the super user utility.
    pub fn user_management(&mut self, user: &str, password: &str) -> Result<()> {
        self.ensure_chroot("set up the user")?;
        self.shell.log("Setting up User Management");

        self.shell
            .run_and_wait_with_args("useradd", &format!("-mG wheel {user}"))?;
        self.set_password(user, password)?;

        // sudo is always installed (base-devel), so wheel always gets it.
        // makepkg and yay rely on it in part 2.
        write_to_file("/etc/sudoers.d/10-wheel", "%wheel ALL=(ALL:ALL) ALL\n")?;
        fs::set_permissions("/etc/sudoers.d/10-wheel", fs::Permissions::from_mode(0o440))?;

        if let SuperUserUtility::Doas = self.super_user_utility {
            // doas rejects a config without a trailing newline
            write_to_file(
                "/etc/doas.conf",
                "permit setenv { XAUTHORITY LANG LC_ALL } persist :wheel as root\n",
            )?;
            fs::set_permissions("/etc/doas.conf", fs::Permissions::from_mode(0o400))?;
        }
        Ok(())
    }
}
