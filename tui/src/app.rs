use anyhow::{bail, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use installer::{
    config::{
        default_layout, nvidia_driver_for, Browser, Config, Desktop, DiskPlan, Docker, FormatChoice, GpuChoice, GpuDriver, Home, LoginShell, OtherMount, ParaMode, Root, Ssh, Swap, Tailscale, User, Vnc,
        MIRROR_COUNTRIES,
    },
    essentials::{Bootloader, SuperUserUtility},
    filesystem_tasks::Filesystem,
    system::{lsblk, BlockDevice, GpuVendor, System},
    utils::is_valid_mount_point,
    validate,
};
use ratatui::widgets::ListState;
use serde_json::json;
use shell_iface::{logger::Logger, Shell};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    Welcome,
    Partition,
    Boot,
    FormatBoot,
    Root,
    Home,
    FormatHome,
    ExtraMounts,
    Swap,
    Mirrors,
    Timezone,
    Locale,
    Accounts,
    Shell,
    MoreUsers,
    Bootloader,
    Privilege,
    Nvidia,
    Amd,
    Desktop,
    Autologin,
    Browser,
    Wifi,
    Extras,
    Packages,
    Finish,
    Review,
}

/// In stage order (see STAGES).
pub const STEPS: [Step; 27] = [
    Step::Welcome,
    Step::Wifi,
    Step::Mirrors,
    Step::Partition,
    Step::Boot,
    Step::FormatBoot,
    Step::Root,
    Step::Home,
    Step::FormatHome,
    Step::ExtraMounts,
    Step::Swap,
    Step::Bootloader,
    Step::Nvidia,
    Step::Amd,
    Step::Timezone,
    Step::Locale,
    Step::Accounts,
    Step::Shell,
    Step::MoreUsers,
    Step::Privilege,
    Step::Autologin,
    Step::Desktop,
    Step::Browser,
    Step::Extras,
    Step::Packages,
    Step::Finish,
    Step::Review,
];

// Choice tables: (value, label). The part before " - " is the short name.
pub const SWAP_SIZES: [Option<u32>; 8] = [None, Some(1), Some(2), Some(4), Some(8), Some(16), Some(32), Some(64)];
pub const BOOTLOADERS: [(Bootloader, &str); 2] = [
    (Bootloader::Grub, "GRUB - can also boot other OSes"),
    (Bootloader::SystemDBoot, "systemd-boot - minimal and fast"),
];
pub const PRIVILEGE: [(SuperUserUtility, &str); 2] = [
    (SuperUserUtility::Sudo, "sudo"),
    (SuperUserUtility::Doas, "doas - sudo stays installed (base-devel)"),
];
pub const SHELLS: [(LoginShell, &str); 3] = [(LoginShell::Bash, "bash"), (LoginShell::Zsh, "zsh"), (LoginShell::Fish, "fish")];
pub const NVIDIA: [(GpuDriver, &str); 3] = [
    (GpuDriver::Nvidia, "Proprietary - GTX 16xx, RTX and newer"),
    (GpuDriver::NvidiaLegacy, "Proprietary legacy - GTX 9xx/10xx (AUR)"),
    (GpuDriver::Nouveau, "Nouveau - open source, slower"),
];
pub const AMD: [(GpuDriver, &str); 2] = [
    (GpuDriver::Amd, "Mesa - open source, recommended"),
    (GpuDriver::AmdPro, "Mesa + AMDGPU PRO - proprietary (AUR)"),
];
pub const DESKTOPS: [(Desktop, &str); 7] = [
    (Desktop::Dwm, "DWM (Xorg) - with dmenu and st"),
    (Desktop::Hyprland, "Hyprland (Wayland) - with kitty"),
    (Desktop::Kde, "KDE Plasma"),
    (Desktop::Gnome, "GNOME"),
    (Desktop::Xfce, "Xfce"),
    (Desktop::Lxde, "LXDE"),
    (Desktop::None, "None - console only"),
];
pub const BROWSERS: [(Browser, &str); 10] = [
    (Browser::Firefox, "Firefox"),
    (Browser::Librewolf, "LibreWolf"),
    (Browser::Chromium, "Chromium"),
    (Browser::Epiphany, "Epiphany - GNOME Web"),
    (Browser::Konqueror, "Konqueror"),
    (Browser::Qutebrowser, "qutebrowser - keyboard driven"),
    (Browser::Falkon, "Falkon"),
    (Browser::Zen, "Zen - AUR"),
    (Browser::Chrome, "Google Chrome - AUR"),
    (Browser::None, "None"),
];
pub const FINISH: [(installer::config::Finish, &str); 3] = [
    (installer::config::Finish::Reboot, "Reboot into the new system"),
    (installer::config::Finish::Poweroff, "Power off"),
    (installer::config::Finish::Stay, "Stay on the ISO - the new system stays mounted at /mnt"),
];

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Extra {
    Ssh,
    Tailscale,
    Vnc,
    Docker,
    Multilib,
    Rice,
}

pub const EXTRAS: [(Extra, &str); 6] = [
    (Extra::Ssh, "SSH server"),
    (Extra::Tailscale, "Tailscale"),
    (Extra::Vnc, "VNC remote desktop"),
    (Extra::Docker, "Docker and Compose"),
    (Extra::Multilib, "multilib repo (Steam, Wine, lib32)"),
    (Extra::Rice, "ParaPsychic rice (dotfiles, dwm)"),
];

/// "GRUB - can also boot..." -> "GRUB"
pub fn short<T: PartialEq + Copy>(table: &[(T, &'static str)], value: T) -> &'static str {
    let label = table.iter().find(|(v, _)| *v == value).map_or("", |(_, l)| *l);
    label.split(" - ").next().unwrap_or(label)
}

fn index_of<T: PartialEq + Copy>(table: &[(T, &str)], value: T) -> usize {
    table.iter().position(|(v, _)| *v == value).unwrap_or(0)
}

/// "" -> "/", "boot" -> "/boot"
pub fn show_mount(key: &str) -> String {
    format!("/{key}")
}

pub enum Action {
    None,
    Quit,
    Install,
    /// Run cfdisk on this disk
    Partition(String),
}

pub struct Field {
    pub label: &'static str,
    pub value: String,
    pub secret: bool,
}

impl Field {
    fn new(label: &'static str, value: &str, secret: bool) -> Field {
        Field { label, value: value.to_string(), secret }
    }
}

/// Groups of steps, as the sidebar shows them. The install itself is the last stage.
pub const STAGES: [(&str, &[Step]); 7] = [
    ("Welcome & HW", &[Step::Welcome]),
    ("Network & Mirrors", &[Step::Wifi, Step::Mirrors]),
    ("Partitioning", &[Step::Partition, Step::Boot, Step::FormatBoot, Step::Root, Step::Home, Step::FormatHome, Step::ExtraMounts, Step::Swap]),
    ("Bootloader & Kern", &[Step::Bootloader, Step::Nvidia, Step::Amd]),
    ("User & Hostname", &[Step::Timezone, Step::Locale, Step::Accounts, Step::Shell, Step::MoreUsers, Step::Privilege, Step::Autologin]),
    ("Desktop & Pkgs", &[Step::Desktop, Step::Browser, Step::Extras, Step::Packages]),
    ("Summary & Base", &[Step::Finish, Step::Review]),
];

/// The sidebar while installing: which installer steps (by name, see installer::step) belong to each stage.
pub const INSTALL_STAGES: [(&str, &[&str]); 7] = [
    ("Welcome & HW", &[]),
    ("Network & Mirrors", &["Getting online", "pre_install hooks", "Mirrors", "Checking packages"]),
    ("Partitioning", &["Partitions"]),
    ("Base & Pacstrap", &["Installing the base system", "Swap, timezone", "Installing drivers"]),
    ("Bootloader & Kern", &["Bootloader"]),
    ("User & Desktop", &["Users", "Network, remote access", "post_install hooks", "AUR packages", "ParaPsychic rice", "Autostart"]),
    ("System Finalize", &["post_setup hooks"]),
];

/// Which INSTALL_STAGES entry a task belongs to.
pub fn install_stage(task: &str) -> usize {
    INSTALL_STAGES.iter().position(|(_, tasks)| tasks.iter().any(|t| task.starts_with(t))).unwrap_or(1)
}

pub fn stage_of(step: Step) -> usize {
    STAGES.iter().position(|(_, steps)| steps.contains(&step)).unwrap_or(0)
}

/// A form (or a list of actions) opened from a list step.
#[derive(Clone, PartialEq, Debug)]
pub enum Sub {
    /// What to do with this disk
    Disk(String),
    /// Mount point for this partition
    Mount(String),
    NewUser { admin: bool },
    Extra(Extra),
}

pub struct App<'a> {
    pub step: Step,
    pub list: ListState,
    pub filter: String,
    pub form: Vec<Field>,
    pub focus: usize,
    pub sub: Option<Sub>,
    pub error: Option<String>,
    pub done: Vec<Step>,
    /// Declarative --config-file preview: only the review, nothing to go back to
    pub preview: bool,
    /// Tab moves focus to the stage list, where enter jumps to a stage already reached
    pub stages_focused: bool,
    pub stage_cursor: usize,
    /// "/" started a search; keys type into the filter
    pub filtering: bool,
    /// Why the review can't install, from the same checks --config-file runs
    pub problems: Vec<String>,
    pub scroll: u16,

    pub cfg: Config,
    /// Mounts on existing partitions while picking them
    pub filesystem: Filesystem<'a>,
    pub sys: System,
    /// The running (or finished) install, once y is pressed
    pub install: Option<crate::term::Install>,
    logger: &'a Logger,
}

impl<'a> App<'a> {
    pub fn new(logger: &'a Logger) -> App<'a> {
        App::with_system(logger, System::probe(logger))
    }

    pub fn with_system(logger: &'a Logger, sys: System) -> App<'a> {
        let mut cfg = Config { desktop: Desktop::Dwm, ..Default::default() };
        let mut drivers = vec![];
        if let Some(nvidia) = sys.gpus.iter().find(|g| g.vendor == GpuVendor::Nvidia) {
            drivers.push(nvidia_driver_for(nvidia.device));
        }
        if sys.has_gpu(GpuVendor::Amd) {
            drivers.push(GpuDriver::Amd);
        }
        if !drivers.is_empty() {
            cfg.hardware.gpu = GpuChoice::List(drivers);
        }
        let mut app = App {
            step: Step::Welcome,
            list: ListState::default(),
            filter: String::new(),
            form: vec![],
            focus: 0,
            sub: None,
            error: None,
            done: vec![],
            preview: false,
            stages_focused: false,
            stage_cursor: 0,
            filtering: false,
            problems: vec![],
            scroll: 0,
            cfg,
            filesystem: Filesystem::new(logger),
            sys,
            install: None,
            logger,
        };
        app.enter(Step::Welcome);
        app
    }

    /// The review screen for a --config-file, before installing it.
    pub fn preview(logger: &'a Logger, sys: System, cfg: Config) -> App<'a> {
        let mut app = App::with_system(logger, sys);
        app.cfg = cfg;
        app.preview = true;
        app.step = Step::Review;
        app
    }

    /// Re-reads disks, e.g. after cfdisk. Forgets mounts whose partition vanished.
    pub fn refresh_devices(&mut self) {
        match lsblk(&mut Shell::new("TUI", self.logger)) {
            Ok(devices) => self.sys.devices = devices,
            Err(e) => self.error = Some(format!("lsblk failed: {e}")),
        }
        let paths: Vec<String> = self.sys.devices.iter().map(|d| d.path.clone()).collect();
        self.filesystem.partitions.retain(|_, p| paths.contains(p));
        self.sync_storage();
    }

    pub fn partitions(&self) -> Vec<&BlockDevice> {
        self.sys.partitions()
    }

    pub fn mount_of(&self, partition: &str) -> Option<&str> {
        self.filesystem.partitions.iter().find(|(_, p)| *p == partition).map(|(m, _)| m.as_str())
    }

    fn auto_partitioning(&self) -> bool {
        !self.cfg.storage.partitioning.is_empty()
    }

    /// cfg.storage from the wizard's picks.
    fn sync_storage(&mut self) {
        let fs = &self.filesystem;
        let storage = &mut self.cfg.storage;
        let choice = |format: bool| if format { FormatChoice::Format } else { FormatChoice::Keep };
        if storage.partitioning.is_empty() {
            storage.efi.partition = fs.get("boot").cloned().unwrap_or("auto".into());
            storage.efi.format = choice(fs.format_boot);
            storage.root = fs.get("/").map(|p| Root { partition: p.clone() });
            storage.home = fs.get("home").map(|p| Home { partition: p.clone(), format: choice(fs.format_home) });
        } else {
            storage.efi = Default::default();
            storage.root = None;
            storage.home = None;
        }
        storage.other = fs
            .partitions
            .iter()
            .filter(|(m, _)| !["", "boot", "home"].contains(&m.as_str()))
            .map(|(m, p)| OtherMount { partition: p.clone(), mountpoint: show_mount(m) })
            .collect();
    }

    pub fn skipped(&self, step: Step) -> bool {
        match step {
            Step::Boot | Step::FormatBoot | Step::Root | Step::Home => self.auto_partitioning(),
            Step::FormatHome => self.auto_partitioning() || self.filesystem.get("home").is_none(),
            Step::Nvidia => !self.sys.has_gpu(GpuVendor::Nvidia),
            Step::Amd => !self.sys.has_gpu(GpuVendor::Amd),
            Step::Wifi => self.sys.wireless.is_empty(),
            _ => false,
        }
    }

    pub fn is_form(&self) -> bool {
        matches!(self.step, Step::Accounts | Step::Wifi | Step::Packages) || matches!(self.sub, Some(Sub::Mount(_) | Sub::NewUser { .. } | Sub::Extra(_)))
    }

    /// Long lists start out searching: just type.
    fn starts_filtering(&self) -> bool {
        matches!(self.step, Step::Mirrors | Step::Timezone | Step::Locale)
    }

    /// Stages up to the furthest one reached so far.
    pub fn reachable(&self, stage: usize) -> bool {
        let furthest = self.done.iter().map(|s| stage_of(*s) + 1).max().unwrap_or(0).max(stage_of(self.step));
        stage <= furthest && !STAGES[stage].1.iter().all(|s| self.skipped(*s))
    }

    pub fn extra_on(&self, extra: Extra) -> bool {
        match extra {
            Extra::Ssh => self.cfg.ssh().is_some(),
            Extra::Tailscale => self.cfg.tailscale().is_some(),
            Extra::Vnc => self.cfg.vnc().is_some(),
            Extra::Docker => self.cfg.docker.is_some(),
            Extra::Multilib => self.cfg.packages.multilib,
            Extra::Rice => self.cfg.parapsychic_mode == ParaMode::Enabled(true),
        }
    }

    fn gpu(&self, nvidia: bool) -> Option<GpuDriver> {
        match &self.cfg.hardware.gpu {
            GpuChoice::List(list) => list.iter().copied().find(|d| d.is_nvidia() == nvidia),
            _ => None,
        }
    }

    fn set_gpu(&mut self, driver: GpuDriver) {
        let mut list: Vec<GpuDriver> = match &self.cfg.hardware.gpu {
            GpuChoice::List(list) => list.iter().copied().filter(|d| d.is_nvidia() != driver.is_nvidia()).collect(),
            _ => vec![],
        };
        list.push(driver);
        self.cfg.hardware.gpu = GpuChoice::List(list);
    }

    /// Every option of the current list step, unfiltered.
    pub fn options(&self) -> Vec<String> {
        let labels = |t: &[&str]| t.iter().map(|s| s.to_string()).collect();
        let table = |t: &[&str]| -> Vec<String> { t.iter().map(|s| s.to_string()).collect() };
        let partitions = |first: Option<&str>| {
            first
                .map(String::from)
                .into_iter()
                .chain(self.partitions().iter().map(|p| match self.mount_of(&p.path) {
                    Some(m) => format!("{}  -> {}", p.describe(), show_mount(m)),
                    None => p.describe(),
                }))
                .collect()
        };
        match self.step {
            Step::Partition if matches!(self.sub, Some(Sub::Disk(_))) => labels(&[
                "Erase it - new EFI + root, everything on it is lost",
                "Use its free space - new EFI + root next to what's there",
                "Edit it in cfdisk, then pick the partitions",
                "Back",
            ]),
            Step::Partition => self
                .sys
                .disks()
                .into_iter()
                .map(|d| format!("{} {}", d.name(), d.model.as_deref().unwrap_or("").trim()))
                .chain(["-> Continue (to select the boot partition)".to_string(), "[R] Rescan storage devices".to_string()])
                .collect(),
            Step::Welcome => labels(&["-> Start"]),
            Step::Boot | Step::Root => partitions(None),
            Step::Home => partitions(Some("No separate /home partition")),
            Step::ExtraMounts => partitions(Some("Done, continue")),
            Step::FormatBoot => labels(&["No - keep it (other OSes' bootloaders live here)", "Yes - format as FAT32, erasing everything on it"]),
            Step::FormatHome => labels(&["No - keep the existing files", "Yes - format as ext4, erasing everything on it"]),
            Step::Swap => SWAP_SIZES.iter().map(|s| s.map_or("No swap".to_string(), |gb| format!("{gb} GB"))).collect(),
            Step::Mirrors => std::iter::once("Automatic - keep the list the ISO ranked at boot".to_string()).chain(table(&MIRROR_COUNTRIES)).collect(),
            Step::Timezone => self.sys.timezones.clone(),
            Step::Locale => self.sys.locales.clone(),
            Step::Shell => SHELLS.iter().map(|(_, l)| l.to_string()).collect(),
            Step::MoreUsers => {
                let mut options = labels(&["Done, continue", "Add a user", "Add an admin user"]);
                for (i, user) in self.cfg.users.iter().enumerate().skip(1) {
                    options.push(format!("{}{} - remove", user.name, if self.cfg.is_admin(i) { " (admin)" } else { "" }));
                }
                options
            }
            Step::Bootloader => BOOTLOADERS.iter().map(|(_, l)| l.to_string()).collect(),
            Step::Privilege => PRIVILEGE.iter().map(|(_, l)| l.to_string()).collect(),
            Step::Nvidia => NVIDIA.iter().map(|(_, l)| l.to_string()).collect(),
            Step::Amd => AMD.iter().map(|(_, l)| l.to_string()).collect(),
            Step::Desktop => DESKTOPS.iter().map(|(_, l)| l.to_string()).collect(),
            Step::Autologin => std::iter::once("No - show the login screen".to_string()).chain(self.cfg.users.iter().map(|u| u.name.clone())).collect(),
            Step::Browser => BROWSERS.iter().map(|(_, l)| l.to_string()).collect(),
            Step::Extras => std::iter::once("Done, continue".to_string())
                .chain(EXTRAS.iter().map(|(e, l)| format!("[{}] {l}", if self.extra_on(*e) { "x" } else { " " })))
                .collect(),
            Step::Finish => FINISH.iter().map(|(_, l)| l.to_string()).collect(),
            Step::Accounts | Step::Wifi | Step::Packages | Step::Review => vec![],
        }
    }

    /// Indices into options() matching the filter.
    pub fn visible(&self) -> Vec<usize> {
        let filter = self.filter.to_lowercase();
        self.options().iter().enumerate().filter(|(_, o)| o.to_lowercase().contains(&filter)).map(|(i, _)| i).collect()
    }

    fn partition_index(&self, offset: usize, partition: Option<&String>) -> Option<usize> {
        let i = self.partitions().iter().position(|p| Some(&p.path) == partition)?;
        Some(i + offset)
    }

    /// Whether `mount_point`'s partition has no filesystem yet.
    fn unformatted(&self, mount_point: &str) -> bool {
        let p = self.filesystem.get(mount_point);
        self.sys.devices.iter().any(|d| Some(&d.path) == p && d.fstype.is_none())
    }

    fn enter(&mut self, step: Step) {
        self.step = step;
        self.filter.clear();
        self.sub = None;
        self.scroll = 0;
        self.filtering = self.starts_filtering();
        let c = &self.cfg;
        let main = c.users.first().cloned().unwrap_or_default();
        let selected = match step {
            Step::Boot => self
                .partition_index(0, self.filesystem.get("boot"))
                .or_else(|| self.partitions().iter().position(|p| p.parttypename.as_deref() == Some("EFI System"))),
            Step::Root => self.partition_index(0, self.filesystem.get("/")),
            Step::Home => self.partition_index(1, self.filesystem.get("home")),
            Step::FormatBoot if self.done.contains(&step) => Some(self.filesystem.format_boot as usize),
            Step::FormatBoot => Some(self.unformatted("boot") as usize),
            Step::FormatHome if self.done.contains(&step) => Some(self.filesystem.format_home as usize),
            Step::FormatHome => Some(self.unformatted("home") as usize),
            Step::Swap => SWAP_SIZES.iter().position(|s| *s == c.storage.swap.0),
            Step::Mirrors => MIRROR_COUNTRIES.iter().position(|m| *m == c.mirrors).map(|i| i + 1),
            Step::Timezone => self.sys.timezones.iter().position(|t| *t == c.timezone),
            Step::Locale => self.sys.locales.iter().position(|l| l.split(' ').next() == Some(&c.locale)),
            Step::Shell => Some(index_of(&SHELLS, main.shell)),
            Step::Bootloader => Some(index_of(&BOOTLOADERS, c.bootloader)),
            Step::Privilege => Some(index_of(&PRIVILEGE, c.admin_tool)),
            Step::Nvidia => self.gpu(true).map(|d| index_of(&NVIDIA, d)),
            Step::Amd => self.gpu(false).map(|d| index_of(&AMD, d)),
            Step::Desktop => Some(index_of(&DESKTOPS, c.desktop)),
            Step::Autologin => c.autologin.as_ref().and_then(|a| c.users.iter().position(|u| &u.name == a)).map(|i| i + 1),
            Step::Browser => Some(index_of(&BROWSERS, c.browser())),
            Step::Finish => Some(index_of(&FINISH, c.finish)),
            Step::Accounts => {
                let root = c.root_password.clone().unwrap_or_default();
                let password = main.password.clone().unwrap_or_default();
                self.form = vec![
                    Field::new("Hostname", &c.hostname, false),
                    Field::new("Username", &main.name, false),
                    Field::new("Full name (optional)", main.full_name.as_deref().unwrap_or_default(), false),
                    Field::new("Password", &password, true),
                    Field::new("Confirm password", &password, true),
                    Field::new("Root password (optional)", &root, true),
                    Field::new("Confirm root password", &root, true),
                ];
                self.focus = 0;
                None
            }
            Step::Wifi => {
                let wifi = c.network.connections.first();
                self.form = vec![
                    Field::new("Network name (SSID)", wifi.and_then(|w| w.ssid.as_deref()).unwrap_or_default(), false),
                    Field::new("Password", wifi.and_then(|w| w.password.as_deref()).unwrap_or_default(), true),
                ];
                self.focus = 0;
                None
            }
            Step::Packages => {
                self.form = vec![
                    Field::new("pacman packages", &c.packages.pacman.join(" "), false),
                    Field::new("AUR packages", &c.packages.aur.join(" "), false),
                ];
                self.focus = 0;
                None
            }
            Step::Review => {
                self.sync_storage();
                self.problems = validate::system_errors(&self.cfg, &self.sys, &installer::source::Source::Dir(".".into()));
                self.problems.extend(validate::schema_errors(&serde_json::to_value(&self.cfg).unwrap_or_default()));
                None
            }
            _ => None,
        };
        self.list.select(Some(selected.unwrap_or(0)));
    }

    fn go(&mut self, forward: bool) -> Action {
        let i = STEPS.iter().position(|s| *s == self.step).unwrap_or(0);
        let next = if forward { STEPS[i + 1..].iter().find(|s| !self.skipped(**s)) } else { STEPS[..i].iter().rev().find(|s| !self.skipped(**s)) };
        match next {
            Some(&s) => {
                self.enter(s);
                Action::None
            }
            None if forward => Action::None,
            None => Action::Quit,
        }
    }

    fn finish_step(&mut self) -> Action {
        if !self.done.contains(&self.step) {
            self.done.push(self.step);
        }
        self.sync_storage();
        self.go(true)
    }

    pub fn on_key(&mut self, key: KeyEvent) -> Action {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Action::Quit;
        }
        self.error = None;
        let result = if self.stages_focused {
            Ok(self.stages_key(key))
        } else if key.code == KeyCode::Tab && !self.is_form() && !self.preview {
            self.stages_focused = true;
            self.stage_cursor = stage_of(self.step);
            Ok(Action::None)
        } else if self.is_form() {
            self.form_key(key)
        } else if self.step == Step::Review {
            self.review_key(key)
        } else {
            self.list_key(key)
        };
        result.unwrap_or_else(|e| {
            self.error = Some(e.to_string());
            Action::None
        })
    }

    fn stages_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.stage_cursor = self.stage_cursor.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => self.stage_cursor = (self.stage_cursor + 1).min(STAGES.len() - 1),
            KeyCode::Enter if self.reachable(self.stage_cursor) => {
                if let Some(&first) = STAGES[self.stage_cursor].1.iter().find(|s| !self.skipped(**s)) {
                    self.enter(first);
                }
                self.stages_focused = false;
            }
            KeyCode::Enter => self.error = Some("Finish the stages before it first.".into()),
            KeyCode::Tab | KeyCode::Esc => self.stages_focused = false,
            _ => {}
        }
        Action::None
    }

    fn review_key(&mut self, key: KeyEvent) -> Result<Action> {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if !self.problems.is_empty() {
                    bail!("Fix the problems listed first.");
                }
                Ok(Action::Install)
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll = self.scroll.saturating_sub(1);
                Ok(Action::None)
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll += 1;
                Ok(Action::None)
            }
            KeyCode::PageUp => {
                self.scroll = self.scroll.saturating_sub(10);
                Ok(Action::None)
            }
            KeyCode::PageDown => {
                self.scroll += 10;
                Ok(Action::None)
            }
            KeyCode::Esc | KeyCode::Char('q') if self.preview => Ok(Action::Quit),
            KeyCode::Esc => Ok(self.go(false)),
            _ => bail!("Press y to install, esc to {}.", if self.preview { "quit" } else { "go back" }),
        }
    }

    fn list_key(&mut self, key: KeyEvent) -> Result<Action> {
        let visible = self.visible();
        let n = visible.len();
        let at = self.list.selected().unwrap_or(0);
        let wrap = |d: isize| ((at as isize + d).rem_euclid(n.max(1) as isize)) as usize;
        let clamp = |d: isize| (at as isize + d).clamp(0, n.saturating_sub(1) as isize) as usize;

        match key.code {
            KeyCode::Up => self.list.select(Some(wrap(-1))),
            KeyCode::Down => self.list.select(Some(wrap(1))),
            KeyCode::PageUp => self.list.select(Some(clamp(-10))),
            KeyCode::PageDown => self.list.select(Some(clamp(10))),
            KeyCode::Home => self.list.select(Some(0)),
            KeyCode::End => self.list.select(Some(n.saturating_sub(1))),
            KeyCode::Char(c) if self.filtering => {
                self.filter.push(c);
                self.list.select(Some(0));
            }
            KeyCode::Backspace if self.filtering => {
                self.filter.pop();
                self.list.select(Some(0));
            }
            KeyCode::Char('/') => {
                self.filtering = true;
                self.list.select(Some(0));
            }
            KeyCode::Char('k') => self.list.select(Some(wrap(-1))),
            KeyCode::Char('j') => self.list.select(Some(wrap(1))),
            KeyCode::Esc if self.filtering && !self.starts_filtering() || !self.filter.is_empty() => {
                self.filter.clear();
                self.filtering = self.starts_filtering();
                self.list.select(Some(0));
            }
            KeyCode::Esc if self.sub.is_some() => {
                self.sub = None;
                self.list.select(Some(0));
            }
            KeyCode::Esc => return Ok(self.go(false)),
            KeyCode::Enter => {
                if let Some(&i) = visible.get(at) {
                    self.filtering = self.starts_filtering();
                    return self.choose(i);
                }
            }
            _ => {}
        }
        Ok(Action::None)
    }

    fn open(&mut self, sub: Sub, fields: Vec<Field>) -> Action {
        self.form = fields;
        self.focus = 0;
        self.sub = Some(sub);
        Action::None
    }

    /// The user picked option `i` (index into options()).
    fn choose(&mut self, i: usize) -> Result<Action> {
        let partition = |offset: usize| self.partitions().get(i.wrapping_sub(offset)).map(|p| p.path.clone());
        match self.step {
            Step::Partition => match self.sub.take() {
                Some(Sub::Disk(disk)) => {
                    self.list.select(Some(0));
                    match i {
                        0 | 1 => {
                            self.cfg.storage.partitioning = vec![DiskPlan { disk, wipe: i == 0, add: default_layout() }];
                            for mount in ["", "boot", "home"] {
                                self.filesystem.set(mount, None)?;
                            }
                        }
                        2 => return Ok(Action::Partition(disk)),
                        _ => return Ok(Action::None),
                    }
                }
                _ => {
                    let disks = self.sys.disks().len();
                    if i < disks {
                        self.sub = Some(Sub::Disk(self.sys.disks()[i].path.clone()));
                        self.list.select(Some(0));
                        return Ok(Action::None);
                    }
                    if i > disks {
                        self.refresh_devices();
                        return Ok(Action::None);
                    }
                    self.cfg.storage.partitioning.clear();
                }
            },
            Step::Welcome => {}
            Step::Boot => self.filesystem.set("boot", partition(0).as_deref())?,
            Step::Root => self.filesystem.set("/", partition(0).as_deref())?,
            Step::Home => self.filesystem.set("home", partition(1).as_deref())?,
            Step::FormatBoot => self.filesystem.format_boot = i == 1,
            Step::FormatHome => self.filesystem.format_home = i == 1,
            Step::ExtraMounts if i > 0 => {
                let partition = partition(1).unwrap_or_default();
                match self.mount_of(&partition).map(String::from) {
                    Some(m) if ["", "boot", "home"].contains(&m.as_str()) => {
                        bail!("{partition} is your {} partition, change it in that step.", show_mount(&m))
                    }
                    Some(m) => {
                        self.filesystem.set(&m, None)?;
                        self.sync_storage();
                    }
                    None => return Ok(self.open(Sub::Mount(partition), vec![Field::new("Mount point", "/", false)])),
                }
                return Ok(Action::None);
            }
            Step::ExtraMounts => {}
            Step::Swap => self.cfg.storage.swap = Swap(SWAP_SIZES[i]),
            Step::Mirrors => self.cfg.mirrors = if i == 0 { "auto".into() } else { MIRROR_COUNTRIES[i - 1].to_string() },
            Step::Timezone => self.cfg.timezone = self.sys.timezones[i].clone(),
            Step::Locale => self.cfg.locale = self.sys.locales[i].split(' ').next().unwrap_or_default().to_string(),
            Step::Shell => self.cfg.users[0].shell = SHELLS[i].0,
            Step::MoreUsers if i == 1 || i == 2 => {
                let fields = vec![Field::new("Username", "", false), Field::new("Password", "", true), Field::new("Confirm password", "", true)];
                return Ok(self.open(Sub::NewUser { admin: i == 2 }, fields));
            }
            Step::MoreUsers if i > 2 => {
                let removed = self.cfg.users.remove(i - 2);
                if self.cfg.autologin.as_ref() == Some(&removed.name) {
                    self.cfg.autologin = None;
                }
                self.list.select(Some(0));
                return Ok(Action::None);
            }
            Step::MoreUsers => {}
            Step::Bootloader => self.cfg.bootloader = BOOTLOADERS[i].0,
            Step::Privilege => self.cfg.admin_tool = PRIVILEGE[i].0,
            Step::Nvidia => self.set_gpu(NVIDIA[i].0),
            Step::Amd => self.set_gpu(AMD[i].0),
            Step::Desktop => {
                self.cfg.desktop = DESKTOPS[i].0;
                if self.cfg.desktop == Desktop::None {
                    self.cfg.remote.vnc = None;
                }
            }
            Step::Autologin => self.cfg.autologin = (i > 0).then(|| self.cfg.users[i - 1].name.clone()),
            Step::Browser => self.cfg.browser = Some(BROWSERS[i].0),
            Step::Extras if i > 0 => return self.toggle(EXTRAS[i - 1].0),
            Step::Extras => {}
            Step::Finish => self.cfg.finish = FINISH[i].0,
            Step::Accounts | Step::Wifi | Step::Packages | Step::Review => {}
        }
        Ok(self.finish_step())
    }

    fn toggle(&mut self, extra: Extra) -> Result<Action> {
        let on = self.extra_on(extra);
        let c = &mut self.cfg;
        match (extra, on) {
            (Extra::Ssh, false) => return Ok(self.open(Sub::Extra(extra), vec![Field::new("Port", "22", false), Field::new("GitHub user for keys (optional)", "", false)])),
            (Extra::Tailscale, false) => return Ok(self.open(Sub::Extra(extra), vec![Field::new("Auth key (optional)", "", true)])),
            (Extra::Vnc, false) if c.desktop == Desktop::None => bail!("VNC needs a desktop to show."),
            (Extra::Vnc, false) => return Ok(self.open(Sub::Extra(extra), vec![Field::new("VNC password", "", true), Field::new("Confirm password", "", true)])),
            (Extra::Ssh, true) => c.remote.ssh = None,
            (Extra::Tailscale, true) => c.remote.tailscale = None,
            (Extra::Vnc, true) => c.remote.vnc = None,
            (Extra::Docker, on) => c.docker = (!on).then(Docker::default),
            (Extra::Multilib, on) => c.packages.multilib = !on,
            (Extra::Rice, on) => c.parapsychic_mode = ParaMode::Enabled(!on),
        }
        Ok(Action::None)
    }

    fn form_key(&mut self, key: KeyEvent) -> Result<Action> {
        let last = self.form.len() - 1;
        match key.code {
            KeyCode::Tab | KeyCode::Down => self.focus = (self.focus + 1).min(last),
            KeyCode::BackTab | KeyCode::Up => self.focus = self.focus.saturating_sub(1),
            KeyCode::Char(c) => self.form[self.focus].value.push(c),
            KeyCode::Backspace => {
                self.form[self.focus].value.pop();
            }
            KeyCode::Enter if self.focus < last => self.focus += 1,
            KeyCode::Enter => return self.submit_form(),
            KeyCode::Esc if self.sub.is_some() => self.sub = None,
            KeyCode::Esc => return Ok(self.go(false)),
            _ => {}
        }
        Ok(Action::None)
    }

    /// Fails with the first rule that doesn't hold, focusing its field.
    fn check(&mut self, rules: &[(usize, bool, &str)]) -> Result<()> {
        if let Some((field, _, msg)) = rules.iter().find(|r| !r.1) {
            self.focus = *field;
            bail!(msg.to_string());
        }
        Ok(())
    }

    fn submit_form(&mut self) -> Result<Action> {
        let v: Vec<String> = self.form.iter().map(|f| f.value.trim().to_string()).collect();
        match self.sub.clone() {
            Some(Sub::Mount(partition)) => {
                let mount_point = v[0].trim_matches('/').to_string();
                if !is_valid_mount_point(&mount_point) || ["boot", "home"].contains(&mount_point.as_str()) {
                    bail!("Pick a path like /data or /mnt/windows (letters, digits, - _ .).");
                }
                self.filesystem.set(&mount_point, Some(&partition))?;
                self.sync_storage();
            }
            Some(Sub::NewUser { admin }) => {
                let taken = self.cfg.users.iter().any(|u| u.name == v[0]);
                self.check(&[
                    (0, valid_username(&v[0]), "Username: lowercase letters, digits, - and _, starting with a letter."),
                    (0, !taken, "That user already exists."),
                    (1, !v[1].is_empty(), "Password can't be empty."),
                    (2, v[1] == v[2], "Passwords don't match."),
                ])?;
                self.cfg.users.push(User { name: v[0].clone(), password: Some(v[1].clone()), admin: Some(admin), ..Default::default() });
            }
            Some(Sub::Extra(Extra::Ssh)) => {
                let port = v[0].parse::<u16>().unwrap_or(0);
                self.check(&[(0, port > 0, "Port: a number from 1 to 65535."), (1, v[1].chars().all(|c| c.is_ascii_alphanumeric() || c == '-'), "GitHub user: letters, digits and -.")])?;
                self.cfg.remote.ssh = Some(Ssh { enable: true, port, ..Default::default() });
                self.cfg.users[0].github = (!v[1].is_empty()).then(|| v[1].clone());
            }
            Some(Sub::Extra(Extra::Tailscale)) => {
                self.check(&[(0, v[0].is_empty() || v[0].starts_with("tskey-"), "Tailscale auth keys start with tskey-.")])?;
                self.cfg.remote.tailscale = Some(Tailscale { enable: true, auth_key: (!v[0].is_empty()).then(|| v[0].clone()), ..Default::default() });
            }
            Some(Sub::Extra(_)) => {
                self.check(&[(0, !v[0].is_empty(), "Password can't be empty."), (1, v[0] == v[1], "Passwords don't match.")])?;
                self.cfg.remote.vnc = Some(Vnc { enable: true, password: Some(v[0].clone()), ..Default::default() });
            }
            // a list, never a form
            Some(Sub::Disk(_)) => {}
            None => return self.submit_step(&v),
        }
        self.sub = None;
        Ok(Action::None)
    }

    fn submit_step(&mut self, v: &[String]) -> Result<Action> {
        match self.step {
            Step::Accounts => {
                self.check(&[
                    (0, valid_hostname(&v[0]), "Hostname: lowercase letters, digits and -, at most 63 characters."),
                    (1, valid_username(&v[1]), "Username: lowercase letters, digits, - and _, starting with a letter."),
                    (1, !self.cfg.users.iter().skip(1).any(|u| u.name == v[1]), "That name is taken by another user."),
                    (2, !v[2].contains([':', ',']), "Full name can't contain : or ,"),
                    (3, !self.form[3].value.is_empty(), "Password can't be empty."),
                    (4, self.form[3].value == self.form[4].value, "Passwords don't match."),
                    (6, self.form[5].value == self.form[6].value, "Root passwords don't match."),
                ])?;
                let main = User {
                    name: v[1].clone(),
                    password: Some(self.form[3].value.clone()),
                    full_name: (!v[2].is_empty()).then(|| v[2].clone()),
                    ..self.cfg.users.first().cloned().unwrap_or_default()
                };
                self.cfg.hostname = v[0].clone();
                self.cfg.root_password = (!self.form[5].value.is_empty()).then(|| self.form[5].value.clone());
                if self.cfg.users.is_empty() {
                    self.cfg.users.push(main);
                } else {
                    self.cfg.users[0] = main;
                }
            }
            Step::Wifi => {
                self.cfg.network.connections = if v[0].is_empty() {
                    vec![]
                } else {
                    let mut wifi = json!({ "name": v[0], "type": "wifi", "ssid": v[0] });
                    if !self.form[1].value.is_empty() {
                        wifi["password"] = self.form[1].value.clone().into();
                    }
                    vec![serde_json::from_value(wifi)?]
                };
            }
            Step::Packages => {
                let words = |s: &str| s.split_whitespace().map(String::from).collect::<Vec<_>>();
                self.cfg.packages.pacman = words(&v[0]);
                self.cfg.packages.aur = words(&v[1]);
            }
            _ => {}
        }
        Ok(self.finish_step())
    }
}

fn valid_hostname(s: &str) -> bool {
    (1..=63).contains(&s.len()) && !s.starts_with('-') && !s.ends_with('-') && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn valid_username(s: &str) -> bool {
    let mut chars = s.chars();
    s.len() <= 32
        && s != "root"
        && chars.next().is_some_and(|c| c.is_ascii_lowercase() || c == '_')
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui;
    use installer::system::Gpu;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn validators() {
        assert!(valid_username("_arch-user1") && !valid_username("1arch") && !valid_username("Arch") && !valid_username("root"));
        assert!(valid_hostname("my-box") && !valid_hostname("-box") && !valid_hostname("My.box") && !valid_hostname(""));
    }

    fn press(app: &mut App, code: KeyCode) -> Action {
        app.on_key(KeyEvent::from(code))
    }

    fn typed(app: &mut App, text: &str) {
        text.chars().for_each(|c| {
            press(app, KeyCode::Char(c));
        });
    }

    fn until(app: &mut App, step: Step) {
        for _ in 0..30 {
            if app.step == step {
                return;
            }
            press(app, KeyCode::Enter);
            assert!(app.error.is_none(), "{:?}: {:?}", app.step, app.error);
        }
        panic!("never reached {step:?}");
    }

    fn machine() -> System {
        let dev = |path: &str, kind: &str, fstype: Option<&str>| BlockDevice {
            path: path.into(),
            kind: kind.into(),
            size: Some(100 << 30),
            fstype: fstype.map(String::from),
            parttypename: (path == "/dev/sda1").then(|| "EFI System".into()),
            pkname: (kind == "part").then(|| "sda".into()),
            ..Default::default()
        };
        System {
            devices: vec![dev("/dev/sda", "disk", None), dev("/dev/sda1", "part", Some("vfat")), dev("/dev/sda2", "part", None), dev("/dev/sda3", "part", Some("ext4")), dev("/dev/sda4", "part", Some("ntfs"))],
            gpus: vec![Gpu { vendor: GpuVendor::Amd, device: 0x15d8 }, Gpu { vendor: GpuVendor::Nvidia, device: 0x1f99 }],
            uefi: true,
            timezones: vec!["Asia/Kolkata".into(), "UTC".into()],
            locales: vec!["en_IN UTF-8".into(), "en_US.UTF-8 UTF-8".into()],
            wireless: vec!["wlan0".into()],
            interfaces: vec!["wlan0".into()],
            ..Default::default()
        }
    }

    fn render(app: &mut App, terminal: &mut Terminal<TestBackend>) -> String {
        terminal.draw(|f| ui::draw(f, app)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        let width = buffer.area.width as usize;
        let text: String = buffer.content.chunks(width).map(|row| row.iter().map(|c| c.symbol()).collect::<String>() + "\n").collect();
        if std::env::var("SHOW_SCREENS").is_ok() {
            println!("{text}");
        }
        text
    }

    /// Walks the wizard on fake hardware, rendering every screen at 80x25 (Linux console size).
    #[test]
    fn wizard_existing_partitions() {
        let logger = Logger::new(false);
        let mut app = App::with_system(&logger, machine());
        let mut terminal = Terminal::new(TestBackend::new(if std::env::var("WIDE").is_ok() { 120 } else { 80 }, if std::env::var("WIDE").is_ok() { 35 } else { 25 })).unwrap();

        // Welcome & HW, then Network & Mirrors
        render(&mut app, &mut terminal);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.step, Step::Wifi);
        typed(&mut app, "Home-5G");
        press(&mut app, KeyCode::Tab);
        typed(&mut app, "secret");
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.step, Step::Mirrors);
        typed(&mut app, "ind");
        render(&mut app, &mut terminal);
        assert_eq!(app.visible().len(), 2); // India, Indonesia
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.cfg.mirrors, "India");

        // Partitioning: sda -> cfdisk, then continue with its partitions
        assert_eq!(app.step, Step::Partition);
        render(&mut app, &mut terminal);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.sub, Some(Sub::Disk("/dev/sda".into())));
        render(&mut app, &mut terminal);
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Down);
        assert!(matches!(press(&mut app, KeyCode::Enter), Action::Partition(d) if d == "/dev/sda"));
        assert_eq!(app.sub, None);
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Enter);

        // EFI preselected, kept; root on the EFI partition is refused
        assert_eq!(app.step, Step::Boot);
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.step, Step::Root);
        press(&mut app, KeyCode::Enter);
        assert!(app.error.as_deref().unwrap().contains("already used"));
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Enter);

        // home on sda3, keep; sda4 at /data
        for _ in 0..3 {
            press(&mut app, KeyCode::Down);
        }
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.step, Step::FormatHome);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.step, Step::ExtraMounts);
        press(&mut app, KeyCode::End);
        press(&mut app, KeyCode::Enter);
        typed(&mut app, "data");
        render(&mut app, &mut terminal);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.cfg.storage.other[0].mountpoint, "/data");
        press(&mut app, KeyCode::Home);
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Enter); // swap

        // Bootloader & Kern
        assert_eq!(app.step, Step::Bootloader);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.step, Step::Nvidia);
        render(&mut app, &mut terminal);
        assert_eq!(app.list.selected(), Some(0)); // GTX 1650 -> nvidia-open
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Enter); // AMD: Mesa

        // User & Hostname
        assert_eq!(app.step, Step::Timezone);
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Up);
        press(&mut app, KeyCode::Enter); // locale en_IN
        assert_eq!(app.cfg.locale, "en_IN");
        assert_eq!(app.step, Step::Accounts);
        app.form[0].value.clear();
        typed(&mut app, "tuf");
        press(&mut app, KeyCode::Tab);
        typed(&mut app, "parapsychic");
        press(&mut app, KeyCode::Tab);
        typed(&mut app, "Para Psychic");
        press(&mut app, KeyCode::Tab);
        typed(&mut app, "pw");
        press(&mut app, KeyCode::Tab);
        typed(&mut app, "px");
        press(&mut app, KeyCode::Tab);
        press(&mut app, KeyCode::Tab);
        press(&mut app, KeyCode::Enter);
        assert_eq!((app.focus, app.error.is_some()), (4, true));
        render(&mut app, &mut terminal);
        press(&mut app, KeyCode::Backspace);
        typed(&mut app, "w");
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.step, Step::Shell);
        assert_eq!(app.cfg.root_password, None);
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.cfg.users[0].shell, LoginShell::Zsh);
        // a second, non-admin user
        assert_eq!(app.step, Step::MoreUsers);
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Enter);
        typed(&mut app, "guest");
        press(&mut app, KeyCode::Tab);
        typed(&mut app, "g");
        press(&mut app, KeyCode::Tab);
        typed(&mut app, "g");
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.cfg.admins(), ["parapsychic"]);
        press(&mut app, KeyCode::Home);
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Enter); // sudo
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.cfg.autologin.as_deref(), Some("parapsychic"));

        // Desktop & Pkgs
        assert_eq!(app.step, Step::Desktop);
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.cfg.desktop, Desktop::Hyprland);
        press(&mut app, KeyCode::Enter); // firefox
        // SSH with GitHub keys, VNC, Docker
        assert_eq!(app.step, Step::Extras);
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Tab);
        typed(&mut app, "parapsychic");
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.cfg.ssh().map(|s| s.port), Some(22));
        assert_eq!(app.cfg.users[0].github.as_deref(), Some("parapsychic"));
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Enter);
        typed(&mut app, "vnc");
        press(&mut app, KeyCode::Tab);
        typed(&mut app, "vnc");
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Enter);
        assert!(app.cfg.docker.is_some() && app.cfg.vnc().is_some());
        render(&mut app, &mut terminal);
        press(&mut app, KeyCode::Home);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.step, Step::Packages);
        typed(&mut app, "htop git");
        press(&mut app, KeyCode::Tab);
        typed(&mut app, "visual-studio-code-bin");
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Enter); // reboot

        assert_eq!(app.step, Step::Review);
        assert_eq!(app.problems, Vec::<String>::new());
        let screen = render(&mut app, &mut terminal);
        assert!(screen.contains("/dev/sda2"), "{screen}");
        assert!(matches!(press(&mut app, KeyCode::Char('y')), Action::Install));

        // what it builds reads back as a valid config file
        let value = serde_json::to_value(&app.cfg).unwrap();
        assert_eq!(validate::schema_errors(&value), Vec::<String>::new());
        let p = app.cfg.packages(&app.sys.gpus);
        for pkg in ["nvidia-open", "vulkan-radeon", "hyprland", "wayvnc", "openssh", "docker", "zsh", "htop"] {
            assert!(p.repo.iter().any(|x| x == pkg), "missing {pkg}");
        }
        assert_eq!(p.aur, ["visual-studio-code-bin"]);

        // tab to the stages, back to Partitioning
        press(&mut app, KeyCode::Tab);
        assert!(app.stages_focused);
        for _ in 0..4 {
            press(&mut app, KeyCode::Up);
        }
        render(&mut app, &mut terminal);
        press(&mut app, KeyCode::Enter);
        assert_eq!((app.step, app.stages_focused), (Step::Partition, false));
    }

    #[test]
    fn wizard_erase_disk() {
        let logger = Logger::new(false);
        let mut app = App::with_system(&logger, machine());
        let mut terminal = Terminal::new(TestBackend::new(80, 25)).unwrap();
        until(&mut app, Step::Partition);
        // later stages can't be jumped to yet
        press(&mut app, KeyCode::Tab);
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Enter);
        assert!(app.error.is_some() && app.step == Step::Partition);
        press(&mut app, KeyCode::Esc);
        // search the disk list with /
        press(&mut app, KeyCode::Char('/'));
        typed(&mut app, "rescan");
        assert_eq!(app.visible().len(), 1);
        press(&mut app, KeyCode::Esc);
        assert_eq!((app.visible().len(), app.filtering), (3, false));
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.step, Step::ExtraMounts);
        assert!(app.cfg.storage.partitioning[0].wipe);
        until(&mut app, Step::Accounts);
        typed(&mut app, "x");
        for _ in 0..3 {
            press(&mut app, KeyCode::Tab);
        }
        typed(&mut app, "arch");
        app.form[1].value = "arch".into();
        app.form[4].value = "arch".into();
        until(&mut app, Step::Review);
        let screen = render(&mut app, &mut terminal);
        assert!(screen.contains("ERASE"), "{screen}");
        assert_eq!(app.problems, Vec::<String>::new());
    }

    /// The install screen around a stand-in installer on a real pty.
    #[test]
    fn install_view() {
        let logger = Logger::new(false);
        let mut app = App::with_system(&logger, machine());
        app.cfg.storage.partitioning = vec![DiskPlan { disk: "/dev/sda".into(), wipe: true, add: default_layout() }];
        let script = r#"printf '\033]0;2lazy4arch 5/12 Installing the base system\007\033[1;36m==>\033[0m \033[1mInstalling the base system\033[0m\n'; for i in 1 2 3; do echo "($i/173) installing linux"; done; sleep 0.5"#;
        app.install = Some(crate::term::Install::spawn("sh", &["-c", script], 10, 60).unwrap());
        std::thread::sleep(std::time::Duration::from_millis(300));
        app.install.as_mut().unwrap().pump();
        let mut small = Terminal::new(TestBackend::new(80, 25)).unwrap();
        let mut big = Terminal::new(TestBackend::new(120, 35)).unwrap();
        render(&mut app, &mut small);
        let screen = render(&mut app, &mut big);
        for expected in ["Installing the base system", "Steps: 5/12", "(Step 4/7)", "(3/173 pkgs)", "(2/173) installing linux", "[STATUS: RUNNING]", "[ACTIVE]", "04. Base & Pacstrap", "PARTLABEL=arch-root"] {
            assert!(screen.contains(expected), "no {expected:?} in\n{screen}");
        }
        for _ in 0..30 {
            app.install.as_mut().unwrap().pump();
            if app.install.as_ref().unwrap().status != crate::term::Status::Running {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let screen = render(&mut app, &mut big);
        assert!(screen.contains("[DONE]") && screen.contains("100%"), "{screen}");
    }

    #[test]
    fn preview_quits_with_esc() {
        let logger = Logger::new(false);
        let cfg: Config = serde_norway::from_str(include_str!("../../unattended-config.yaml")).unwrap();
        let mut app = App::preview(&logger, machine(), cfg);
        let mut terminal = Terminal::new(TestBackend::new(80, 25)).unwrap();
        render(&mut app, &mut terminal);
        press(&mut app, KeyCode::PageDown);
        render(&mut app, &mut terminal);
        assert!(matches!(press(&mut app, KeyCode::Esc), Action::Quit));
    }
}
