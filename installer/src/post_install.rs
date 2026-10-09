use std::{fs, path::Path};

use crate::pacman::Pacman;
use anyhow::{anyhow, Result};
use shell_iface::{logger::Logger, Shell};

// ParaPsychic mode. Fork? Point these at your own dotfiles.
const DOTFILES_REPO: &str = "https://github.com/parapsychic/dot-files.git";
const GIT_NAME: &str = "parapsychic";
const GIT_EMAIL: &str = "febinkdominic@outlook.com";

/// PostInstall installs optional stuff after the first boot.
/// Runs as the normal user; privileged steps go through sudo.
pub struct PostInstall<'a> {
    shell: Shell<'a>,
    pacman: Pacman<'a>,
}

impl<'a> PostInstall<'a> {
    pub fn new<'b>(logger: &'b Logger) -> PostInstall<'b> {
        PostInstall {
            shell: Shell::new("PostInstall", logger),
            pacman: Pacman::new(logger),
        }
    }

    /// Packages skipped so far because they don't exist.
    pub fn skipped(&self) -> &[String] {
        &self.pacman.skipped
    }

    pub fn install_packages(&mut self, packages: &[&str]) -> Result<()> {
        self.pacman.pacman().install(packages)
    }

    /// Installs AUR (or repo) packages with yay, bootstrapping yay first if needed.
    pub fn install_aur(&mut self, packages: &[&str]) -> Result<()> {
        if packages.is_empty() {
            return Ok(());
        }
        if !Path::new("/usr/bin/yay").exists() {
            self.setup_yay()?;
        }
        self.pacman.yay().install(packages)
    }

    fn setup_yay(&mut self) -> Result<()> {
        self.shell.log("Installing yay");
        self.pacman.pacman().install(&["git", "base-devel", "go"])?;
        let dir = "/tmp/2lazy4arch-yay";
        let _ = fs::remove_dir_all(dir);
        self.shell.run_and_wait_with_args(
            "git",
            &format!("clone --depth 1 https://aur.archlinux.org/yay.git {dir}"),
        )?;
        self.shell
            .run_in_directory_and_wait_with_args(dir, "makepkg", "-si --noconfirm")?;
        Ok(())
    }

    /// ParaPsychic's rice: dotfiles, dwm/dmenu from them, X touchpad config. Was the `rice` script.
    pub fn misc_options(&mut self) -> Result<()> {
        self.shell.log("Running ParaPsychic specific settings...");
        let home = std::env::var("HOME")?;
        let dots = format!("{home}/dot-files");

        self.shell.log("Setting up pacman in style, enabling multilib");
        self.shell.run_and_wait_with_args(
            "sudo",
            r"sed -i -e 's/^#Color$/Color\nILoveCandy/' -e '/^#\[multilib\]$/,/^#Include/ s/^#//' /etc/pacman.conf",
        )?;

        self.install_packages(&[
            "xorg-server", "xorg-xinit", "libx11", "libxft", "libxinerama", // X + dwm build deps
            "mpv", "htop", "fastfetch", "fzf", "lolcat", "ueberzug", "ttf-hack", "noto-fonts-emoji",
            "brightnessctl", "lf", "ytfzf",
        ])?;
        self.install_aur(&["yt-dlp-drop-in", "tabbed", "otf-manjari"])?;

        self.shell.log("Cloning dot-files");
        let _ = fs::remove_dir_all(&dots);
        self.shell
            .run_and_wait_with_args("git", &format!("clone {DOTFILES_REPO} {dots}"))?;

        self.shell.log("Building dwm and dmenu");
        for (src, dest) in [("dwm", ".dwm"), ("dmenu", ".dmenu")] {
            let dest = format!("{home}/{dest}");
            self.copy(&format!("{dots}/{src}"), &dest)?;
            self.shell.run_in_directory_and_wait_with_args(&dest, "make", "")?;
            self.shell
                .run_in_directory_and_wait_with_args(&dest, "sudo", "make install")?;
        }
        self.sudo_write(
            "/usr/share/xsessions/dwm.desktop",
            "[Desktop Entry]\nEncoding=UTF-8\nName=Dwm\nComment=the dynamic window manager\nExec=/usr/local/bin/dwm\nIcon=dwm\nType=XSession\n",
        )?;

        self.shell.log("Copying dotfiles");
        fs::create_dir_all(format!("{home}/.config"))?;
        for (src, dest) in [
            (".xinitrc", ".xinitrc"),
            (".bashrc", ".bashrc"),
            ("autostart.sh", "autostart.sh"),
            (".bin", ".bin"),
            ("nvim", ".config/nvim"),
            ("dunst", ".config/dunst"),
            ("conky", ".config/conky"),
            ("alacritty", ".config/alacritty"),
            ("lf", ".config/lf"),
        ] {
            self.copy(&format!("{dots}/{src}"), &format!("{home}/{dest}"))?;
        }

        self.shell.log("Setting up Git (not authenticated with GitHub)");
        self.shell
            .run_and_wait_with_args("git", &format!("config --global user.email {GIT_EMAIL}"))?;
        self.shell
            .run_and_wait_with_args("git", &format!("config --global user.name {GIT_NAME}"))?;

        self.shell.log("Setting up touchpad");
        self.sudo_write(
            "/etc/X11/xorg.conf.d/90-touchpad.conf",
            r#"Section "InputClass"
    Identifier "touchpad"
    MatchIsTouchpad "on"
    Driver "libinput"
    Option "Tapping" "on"
    Option "NaturalScrolling" "on"
    Option "ScrollMethod" "twofinger"
    Option "TappingDrag" "on"
    Option "DisableWhileTyping" "on"
EndSection
"#,
        )?;

        self.shell.log("Ricing complete");
        Ok(())
    }

    /// `cp -rT`: copies a file or a directory's contents onto `dest`, without nesting
    /// `src` inside `dest` when it already exists.
    fn copy(&mut self, src: &str, dest: &str) -> Result<()> {
        self.shell.run_and_wait_with_args("cp", &format!("-rT {src} {dest}"))?;
        Ok(())
    }

    /// Writes a root-owned file, creating parent directories.
    fn sudo_write(&mut self, path: &str, content: &str) -> Result<()> {
        let tmp = std::env::temp_dir().join("2lazy4arch-file");
        fs::write(&tmp, content)?;
        let tmp = tmp.to_str().ok_or_else(|| anyhow!("non UTF-8 temp dir"))?;
        self.shell
            .run_and_wait_with_args("sudo", &format!("install -Dm644 {tmp} {path}"))?;
        Ok(())
    }
}
