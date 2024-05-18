use anyhow::{anyhow, Result};
use shell_iface::{logger::Logger, Shell};
use std::{fs, os::unix};

use crate::{
    pacman::Pacman,
    utils::{append_to_file, get_processor_make, get_uuid_root, write_to_file},
};

pub enum Bootloader {
    Grub,
    SystemDBoot,
}

pub enum SuperUserUtility {
    Sudo,
    Doas,
}

/// Essentials basically installs arch to be a bootable/usable state.
/// This is same as the install.sh
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
        let shell = Shell::new("Base Installer", logger);
        let pacman = Pacman::new(logger);

        Essentials {
            is_chroot: false,
            shell,
            pacman,
            bootloader,
            super_user_utility,
        }
    }

    /// chroot into the system
    /// It is imperative that this should be called first before executing any other fns.
    /// Instead of calling arch-chroot, chroot is being called directly.
    /// Followed instructions from [here](https://wiki.archlinux.org/title/Chroot#Using_chroot)
    pub fn chroot(&mut self) -> Result<()> {
        self.shell.log("Entering chroot.");
        self.shell
            .run_with_args("mount", "-t proc /proc /mnt/proc/")?;
        self.shell
            .run_with_args("mount", "-t sysfs /sys /mnt/sys/")?;
        self.shell
            .run_with_args("mount", "-o bind /dev /mnt/dev/")?;
        self.shell
            .run_with_args("mount", "-o bind /run /mnt/run/")?;
        self.shell.run_with_args(
            "mount",
            "-o bind /sys/firmware/efi/efivars /mnt/sys/firmware/efi/efivars/",
        )?;
        fs::copy("/etc/resolv.conf", "/mnt/etc/resolv.conf")?;
        std::os::unix::fs::chroot("/sandbox")?;
        std::env::set_current_dir("/")?;

        self.shell.log("Entered chroot.");

        self.shell.log("Sourcing bash profiles from chroot.");
        self.shell.run_with_args("source", "/etc/profile")?;
        self.shell.run_with_args("source", "~/.bashrc")?;
        self.is_chroot = true;

        self.shell.log("Completed entering chroot.");
        Ok(())
    }

    /// Sets the swap size.
    /// Size is the number of 1M blocks on disk.
    /// eg. 1GB swap = 1024, 8GB = 8096...
    /// Should be run in a multithreaded manner. There is no point in waiting for this to complete.
    /// But, must panic if the operation fails as that would affect the whole system.
    pub fn initialize_swap(&mut self, size: usize) -> Result<()> {
        self.shell.log("Initializing Swap.");
        if !self.is_chroot {
            self.shell.log("Cannot initialize swap, not in chroot.");
            return Err(anyhow!("Cannot initialize swap, not in chroot."));
        }

        self.shell.log(&format!("Size: {} MB", size));
        self.shell.log("Creating Swap Partition");
        let status = self.shell.run_and_wait_with_args(
            "dd",
            &format!(
                "if=/dev/zero of=/swapfile bs=1M count={} status=progress",
                size
            ),
        )?;

        if !status.success() {
            self.shell.log("dd failed. Exited with non-zero status.");
            return Err(anyhow!("Could not create swap file."));
        }

        self.shell
            .run_and_wait_with_args("chmod", "600 /swapfile")?;
        self.shell.run_and_wait_with_args("mkswap", "/swapfile")?;
        self.shell.run_and_wait_with_args("swapon", "/swapfile")?;

        self.shell.log("Appending swap to fstab.");
        append_to_file("/etc/fstab", "/swapfile none  swap defaults 0 0")
    }

    /// Sets the timezone.
    /// Expects a valid Timezone from zoneinfo
    /// /usr/share/zoneinfo/Asia/Kolkata
    pub fn set_timezones(&mut self, timezone: &str) -> Result<()> {
        self.shell.log("Setting timezones.");
        if !self.is_chroot {
            self.shell.log("Setting timezones failed. Not in chroot.");
            return Err(anyhow!("Setting timezones failed. Not in chroot."));
        }

        self.shell.log("Synchronizing Timezones");
        unix::fs::symlink(
            format!("/usr/share/zoneinfo/{}", timezone),
            "/etc/localtime",
        )?;
        self.shell.run_and_wait_with_args("hwclock", "--systohc")?;
        Ok(())
    }

    /// Generates locale.
    /// Expects a valid locale. Does not check.
    pub fn gen_locale(&mut self, locale: &str, encoding: &str) -> Result<()> {
        self.shell.log("Generating Locale");

        if !self.is_chroot {
            self.shell.log("Setting locale failed. Not in chroot.");
            return Err(anyhow!("Setting locale failed. Not in chroot."));
        }

        self.shell.log("Appending locale to fstab.");

        append_to_file("/etc/locale.gen", &format!("{} {}", locale, encoding))?;
        self.shell.run_and_wait("locale-gen")?;
        append_to_file("/etc/locale.conf", &format!("LANG={}", locale))
    }

    /// Sets the hostname and the hosts configuration
    pub fn set_hostname(&mut self, hostname: &str) -> Result<()> {
        self.shell.log("Setting hostname");

        if !self.is_chroot {
            self.shell.log("Setting hostname failed. Not in chroot.");
            return Err(anyhow!("Setting hostname failed. Not in chroot."));
        }

        write_to_file("/etc/hostname", hostname)?;
        self.shell.log("Setting hosts");
        append_to_file(
            "/etc/hosts",
            &format!(
                "127.0.0.1\tlocalhost\n::1\tlocalhost\n127.0.1.1\t{}.localdomain\t{}",
                hostname, hostname
            ),
        )?;

        Ok(())
    }

    /// Runs mkinicpio
    pub fn mkinitcpio(&mut self) -> Result<()> {
        self.shell.log("Running mkinitcpio");

        if !self.is_chroot {
            self.shell.log("Cannot run mkinitcpio. Not in chroot.");
            return Err(anyhow!("Cannot run mkinitcpio. Not in chroot."));
        }

        self.shell.run_and_wait_with_args("mkinitcpio", "-P")?;
        self.shell.log("Completed mkinitcpio");

        Ok(())
    }

    /// set up password
    pub fn set_password(&mut self, user: &str, password: &str) -> Result<()> {
        self.shell.log(&format!("Setting password for {}", user));

        if !self.is_chroot {
            self.shell.log("Cannot set password. Not in chroot.");
            return Err(anyhow!("Cannot set password. Not in chroot."));
        }

        self.shell
            .spawn_with_piped_input(&"chpasswd", &format!("{}:{}", user, password))?;
        self.shell.log("Password set successfully.");

        Ok(())
    }

    /// installs the required programs
    pub fn install_essentials(&mut self, extra_programs: Option<Vec<&str>>) -> Result<()> {
        self.shell.log("Starting essentials package install");

        if !self.is_chroot {
            self.shell
                .log("Cannot install essential packages. Not in chroot.");
            return Err(anyhow!("Cannot install essential packages. Not in chroot."));
        }

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
            "pulseaudio-bluetooth",
            "alsa-utils",
            "cups",
            "neovim",
        ];

        if let Some(extras) = extra_programs {
            essential_packages.extend(extras)
        }

        self.pacman.install(essential_packages)?;
        self.shell.log("Completed essentials package install");

        self.shell.log("Enabling Services");
        self.shell
            .run_and_wait_with_args("systemctl", "enable NetworkManager")?;
        self.shell
            .run_and_wait_with_args("systemctl", "enable bluetooth")?;

        self.shell.log("Completed enabling Services");

        Ok(())
    }

    pub fn install_bootloader(&mut self) -> Result<()> {
        match self.bootloader {
            Bootloader::Grub => self.install_grub(),
            Bootloader::SystemDBoot => self.install_systemdboot(),
        }
    }

    /// Installs and configures grub
    /// Shouldn't be called from outside
    /// Only one bootloader can be installed
    fn install_grub(&mut self) -> Result<()> {
        self.shell.log("Installing Grub as the Bootloader");

        if !self.is_chroot {
            self.shell.log("Cannot install grub. Not in chroot.");
            return Err(anyhow!("Cannot install grub. Not in chroot."));
        }

        self.shell
            .log("os-prober is disabled, windows won't be recognized");
        self.shell
            .log("run grub-mkconfig again with edited grub file");
        self.shell.run_and_wait_with_args(
            "grub-install",
            "--target=x86_64-efi --efi-directory=/boot --bootloader-id=GRUB",
        )?;
        self.shell
            .run_and_wait_with_args("grub-mkconfig", "-o /boot/grub/grub.cfg")?;
        Ok(())
    }

    /// Installs and configures systemd-boot
    /// Shouldn't be called from outside
    /// Only one bootloader can be installed
    /// Does not support  Secure boot. TODO
    fn install_systemdboot(&mut self) -> Result<()> {
        self.shell.log("Installing SystemD Boot as the Bootloader");
        // TODO: Show this to user instead of logging.
        self.shell.log("This mode does not support secure boot. If you have secure boot installed, you might want to set up [signing the bootloader](https://wiki.archlinux.org/title/Systemd-boot#Signing_for_Secure_Boot).");

        if !self.is_chroot {
            self.shell
                .log("Cannot install systemd-boot. Not in chroot.");
            return Err(anyhow!("Cannot install systemd-boot. Not in chroot."));
        }

        self.shell.run_and_wait_with_args("bootctl", "install")?;

        self.shell
            .run_and_wait_with_args("systemctl", "enable systemd-boot-update.service")?;
        write_to_file(
            "/boot/loader/loader.conf",
            "default  arch.conf
timeout  4
console-mode max
editor   no",
        )?;

        let uuid = get_uuid_root()?;
        let default_conf;
        let fallback_conf;

        // bad code, idc.
        // basically decides whether to load a ucode or not.
        if let Some(processor) = get_processor_make() {
            default_conf = format!(
                "title   Arch Linux 
linux   /vmlinuz-linux
initrd  /{}-ucode.img
initrd  /initramfs-linux.img
options root=UUID={} rw",
                processor, uuid
            );
            fallback_conf = format!(
                "title   Arch Linux (fallback initramfs)
linux   /vmlinuz-linux
initrd  /{}-ucode.img
initrd  /initramfs-linux-fallback.img
options root=UUID={} rw",
                processor, uuid
            );
        } else {
            default_conf = format!(
                "title   Arch Linux 
linux   /vmlinuz-linux
initrd  /initramfs-linux.img
options root=UUID={} rw",
                uuid
            );
            fallback_conf = format!(
                "title   Arch Linux (fallback initramfs)
linux   /vmlinuz-linux
initrd  /initramfs-linux-fallback.img
options root=UUID={} rw",
                uuid
            );
        }

        // write default entry
        write_to_file("/loader/entries/arch.conf", &default_conf)?;

        // write default entry
        write_to_file("/loader/entries/arch-fallback.conf", &fallback_conf)?;
        Ok(())
    }

    /// Adds a new user, sets permissions, installs and sets up the super user utility.
    pub fn user_management(&mut self, user: &str, password: &str) -> Result<()> {
        self.shell.log("Setting up User Management");

        if !self.is_chroot {
            self.shell
                .log("Cannot install essential packages. Not in chroot.");
            return Err(anyhow!("Cannot install essential packages. Not in chroot."));
        }

        self.shell
            .run_and_wait_with_args("useradd", &format!("-mG wheel {}", user))?;
        self.set_password(user, password)?;
        self.shell.log("Password set successfully.");

        self.shell.log("Adding wheel to sudoers");

        match self.super_user_utility {
            // I think sudo is already installed during the base build
            // if not it will be installed during some package install as a dependency
            SuperUserUtility::Sudo => {
                self.shell.run_and_wait_with_args(
                    "sed",
                    "-i \"82 i %wheel ALL=(ALL) ALL\" /etc/sudoers",
                )?;
            }
            SuperUserUtility::Doas => {
                self.pacman.install(vec!["opendoas"])?;
                write_to_file(
                    "/etc/doas.conf",
                    "permit setenv { XAUTHORITY LANG LC_ALL } persist :wheel as root",
                )?;
                unix::fs::symlink("/usr/bin/doas", "/usr/bin/sudo")?;
            }
        }

        Ok(())
    }

    /// Exits chroot
    /// Also unmounts all the partitions that were mounted during the chroot setup.
    /// Follows the guide [here](https://wiki.archlinux.org/title/Chroot#Using_chroot)
    pub fn exit_chroot(&mut self) -> Result<()> {
        self.shell.log("Started exit chroot.");

        if !self.is_chroot {
            self.shell.log("Cannot exit chroot: Not in chroot.");
            return Err(anyhow!("Cannot exit chroot: Not in chroot."));
        }

        self.shell.run_and_wait("exit")?;
        std::env::set_current_dir("/")?;
        self.shell
            .run_and_wait_with_args("umount", "--recursive /path/to/new/root")?;

        self.is_chroot = false;
        self.shell.log("Exit chroot successful.");
        Ok(())
    }
}
