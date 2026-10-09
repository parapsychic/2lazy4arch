use crate::{
    essentials::{Bootloader, SuperUserUtility},
    utils::GpuVendor,
};

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum NvidiaDriver {
    /// nvidia-open: Turing (GTX 16xx / RTX 20xx) and newer
    #[default]
    Open,
    /// nvidia-580xx from the AUR: Maxwell / Pascal (GTX 9xx / 10xx)
    Legacy,
    Nouveau,
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum AmdDriver {
    #[default]
    Mesa,
    /// AMD's proprietary Vulkan driver + AMF encoder (AUR), on top of Mesa
    Pro,
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Desktop {
    /// Built from suckless git (see Essentials::install_dwm)
    #[default]
    Dwm,
    Hyprland,
    Kde,
    Gnome,
    Xfce,
    Lxde,
    None,
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Browser {
    #[default]
    Firefox,
    LibreWolf,
    Chromium,
    Vivaldi,
    Brave,
    Zen,
    Chrome,
    None,
}

/// Everything picked in the TUI.
#[derive(Default, Debug)]
pub struct Config {
    pub mirror_country: String,
    pub swap_gb: usize,
    pub timezone: String,
    /// A line from /etc/locale.gen, e.g. "en_US.UTF-8 UTF-8"
    pub locale: String,
    pub hostname: String,
    pub username: String,
    pub password: String,
    pub root_password: String,
    pub bootloader: Bootloader,
    pub super_user_utility: SuperUserUtility,
    pub gpus: Vec<GpuVendor>,
    pub nvidia: NvidiaDriver,
    pub amd: AmdDriver,
    pub desktop: Desktop,
    pub browser: Browser,
}

#[derive(Default, Debug, PartialEq)]
pub struct Packages {
    pub repo: Vec<&'static str>,
    /// yay only exists after part 2, so AUR packages are queued for it.
    pub aur: Vec<&'static str>,
    pub services: Vec<&'static str>,
}

impl Config {
    pub fn has_gpu(&self, vendor: GpuVendor) -> bool {
        self.gpus.contains(&vendor)
    }

    /// The proprietary driver needs nouveau kept out of the initramfs.
    pub fn nvidia_proprietary(&self) -> bool {
        self.has_gpu(GpuVendor::Nvidia) && self.nvidia != NvidiaDriver::Nouveau
    }

    /// Drivers, desktop and browser packages for the chosen config.
    pub fn packages(&self) -> Packages {
        let mut p = Packages::default();
        p.repo.push("mesa");

        if self.has_gpu(GpuVendor::Intel) {
            p.repo.extend(["vulkan-intel", "intel-media-driver"]);
        }
        if self.has_gpu(GpuVendor::Amd) {
            p.repo.push("vulkan-radeon");
            if self.amd == AmdDriver::Pro {
                p.aur.extend(["vulkan-amdgpu-pro", "amf-amdgpu-pro"]);
            }
        }
        if self.has_gpu(GpuVendor::Nvidia) {
            // laptops like the TUF 505DT: iGPU drives the screen, NVIDIA renders on demand (prime-run)
            let hybrid = self.has_gpu(GpuVendor::Intel) || self.has_gpu(GpuVendor::Amd);
            match self.nvidia {
                NvidiaDriver::Open => {
                    p.repo.extend(["nvidia-open", "nvidia-utils", "nvidia-settings"]);
                    if hybrid {
                        p.repo.push("nvidia-prime");
                    }
                }
                // ponytail: no nvidia-prime here, it depends on repo nvidia-utils which conflicts with 580xx
                NvidiaDriver::Legacy => p.aur.extend(["nvidia-580xx-dkms", "nvidia-580xx-utils"]),
                NvidiaDriver::Nouveau => p.repo.push("vulkan-nouveau"),
            }
        }

        let lightdm = ["lightdm", "lightdm-gtk-greeter"];
        match self.desktop {
            Desktop::Dwm => {
                // build deps for dwm/dmenu/st + X
                p.repo.extend(["xorg-server", "xorg-xinit", "libx11", "libxft", "libxinerama"]);
                p.repo.extend(lightdm);
                p.services.push("lightdm");
            }
            Desktop::Hyprland => {
                // apps referenced by Hyprland's default config
                p.repo.extend(["hyprland", "kitty", "dolphin", "hyprlauncher", "xdg-desktop-portal-hyprland", "sddm"]);
                p.services.push("sddm");
            }
            Desktop::Kde => {
                p.repo.extend(["plasma-meta", "konsole", "dolphin"]);
                p.services.push("plasmalogin");
            }
            Desktop::Gnome => {
                p.repo.push("gnome");
                p.services.push("gdm");
            }
            Desktop::Xfce => {
                p.repo.extend(["xorg-server", "xfce4", "xfce4-goodies"]);
                p.repo.extend(lightdm);
                p.services.push("lightdm");
            }
            Desktop::Lxde => {
                // the lxde group ships lxdm
                p.repo.extend(["xorg-server", "lxde"]);
                p.services.push("lxdm");
            }
            Desktop::None => {}
        }

        match self.browser {
            Browser::Firefox => p.repo.push("firefox"),
            Browser::LibreWolf => p.repo.push("librewolf"),
            Browser::Chromium => p.repo.push("chromium"),
            Browser::Vivaldi => p.repo.push("vivaldi"),
            Browser::Brave => p.aur.push("brave-bin"),
            Browser::Zen => p.aur.push("zen-browser-bin"),
            Browser::Chrome => p.aur.push("google-chrome"),
            Browser::None => {}
        }

        // otherwise pacman picks gnu-free-fonts for the ttf-font dependency
        if self.desktop != Desktop::None || self.browser != Browser::None {
            p.repo.push("noto-fonts");
        }
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use GpuVendor::*;

    #[test]
    fn hybrid_amd_nvidia_laptop() {
        let cfg = Config { gpus: vec![Amd, Nvidia], desktop: Desktop::Kde, browser: Browser::Brave, ..Default::default() };
        let p = cfg.packages();
        for pkg in ["vulkan-radeon", "nvidia-open", "nvidia-utils", "nvidia-prime", "plasma-meta"] {
            assert!(p.repo.contains(&pkg), "missing {pkg}");
        }
        assert_eq!(p.aur, ["brave-bin"]);
        assert_eq!(p.services, ["plasmalogin"]);
        assert!(cfg.nvidia_proprietary());
    }

    #[test]
    fn intel_only_ignores_nvidia_choice() {
        let cfg = Config { gpus: vec![Intel], desktop: Desktop::None, browser: Browser::None, ..Default::default() };
        let p = cfg.packages();
        assert_eq!(p.repo, ["mesa", "vulkan-intel", "intel-media-driver"]);
        assert!(p.aur.is_empty() && p.services.is_empty());
        assert!(!cfg.nvidia_proprietary());
    }

    #[test]
    fn aur_drivers_are_queued() {
        let cfg = Config {
            gpus: vec![Intel, Nvidia, Amd],
            nvidia: NvidiaDriver::Legacy,
            amd: AmdDriver::Pro,
            ..Default::default()
        };
        let p = cfg.packages();
        assert!(!p.repo.iter().any(|x| x.starts_with("nvidia")));
        assert_eq!(p.aur, ["vulkan-amdgpu-pro", "amf-amdgpu-pro", "nvidia-580xx-dkms", "nvidia-580xx-utils"]);
        assert!(cfg.nvidia_proprietary());
    }
}
