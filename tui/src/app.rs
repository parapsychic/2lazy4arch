use std::{fs, path::Path};

use anyhow::{bail, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use installer::{
    config::{AmdDriver, Browser, Config, Desktop, NvidiaDriver},
    essentials::{Bootloader, SuperUserUtility},
    filesystem_tasks::{BlockDevice, Filesystem},
    utils::{detect_gpus, get_processor_make, is_valid_mount_point, GpuVendor},
};
use ratatui::widgets::ListState;
use shell_iface::logger::Logger;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    Partition,
    Boot,
    FormatBoot,
    Root,
    Home,
    FormatHome,
    ExtraMounts,
    Mirrors,
    Swap,
    Timezone,
    Locale,
    Accounts,
    Bootloader,
    Privilege,
    Nvidia,
    Amd,
    Desktop,
    Browser,
    Review,
}

pub const STEPS: [Step; 19] = [
    Step::Partition,
    Step::Boot,
    Step::FormatBoot,
    Step::Root,
    Step::Home,
    Step::FormatHome,
    Step::ExtraMounts,
    Step::Mirrors,
    Step::Swap,
    Step::Timezone,
    Step::Locale,
    Step::Accounts,
    Step::Bootloader,
    Step::Privilege,
    Step::Nvidia,
    Step::Amd,
    Step::Desktop,
    Step::Browser,
    Step::Review,
];

// Choice tables: (value, label). The part before " - " is the short name.
pub const SWAP_SIZES: [usize; 8] = [0, 1, 2, 4, 8, 16, 32, 64];
pub const BOOTLOADERS: [(Bootloader, &str); 2] = [
    (Bootloader::Grub, "GRUB - can also boot other OSes"),
    (Bootloader::SystemDBoot, "systemd-boot - minimal and fast"),
];
pub const PRIVILEGE: [(SuperUserUtility, &str); 2] = [
    (SuperUserUtility::Sudo, "sudo"),
    (SuperUserUtility::Doas, "doas - sudo stays installed (base-devel)"),
];
pub const NVIDIA: [(NvidiaDriver, &str); 3] = [
    (NvidiaDriver::Open, "Proprietary - GTX 16xx, RTX and newer"),
    (NvidiaDriver::Legacy, "Proprietary legacy - GTX 9xx/10xx (AUR)"),
    (NvidiaDriver::Nouveau, "Nouveau - open source, slower"),
];
pub const AMD: [(AmdDriver, &str); 2] = [
    (AmdDriver::Mesa, "Mesa - open source, recommended"),
    (AmdDriver::Pro, "Mesa + AMDGPU PRO - proprietary (AUR)"),
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
pub const BROWSERS: [(Browser, &str); 8] = [
    (Browser::Firefox, "Firefox"),
    (Browser::LibreWolf, "LibreWolf"),
    (Browser::Chromium, "Chromium"),
    (Browser::Vivaldi, "Vivaldi"),
    (Browser::Brave, "Brave - AUR, part 2"),
    (Browser::Zen, "Zen - AUR, part 2"),
    (Browser::Chrome, "Google Chrome - AUR, part 2"),
    (Browser::None, "None"),
];
pub const COUNTRIES: [&str; 70] = [
    "Australia", "Austria", "Azerbaijan", "Bangladesh", "Belarus", "Belgium", "Bosnia and Herzegovina",
    "Brazil", "Bulgaria", "Cambodia", "Canada", "Chile", "China", "Colombia", "Croatia", "Czechia",
    "Denmark", "Ecuador", "Estonia", "Finland", "France", "Georgia", "Germany", "Greece", "Hong Kong",
    "Hungary", "Iceland", "India", "Indonesia", "Iran", "Israel", "Italy", "Japan", "Kazakhstan", "Kenya",
    "Latvia", "Lithuania", "Luxembourg", "Mauritius", "Mexico", "Moldova", "Monaco", "Netherlands",
    "New Caledonia", "New Zealand", "North Macedonia", "Norway", "Paraguay", "Poland", "Portugal",
    "Romania", "Russia", "Réunion", "Serbia", "Singapore", "Slovakia", "Slovenia", "South Africa",
    "South Korea", "Spain", "Sweden", "Switzerland", "Taiwan", "Thailand", "Türkiye", "Ukraine",
    "United Kingdom", "United States", "Uzbekistan", "Vietnam",
];

/// "GRUB - also boots..." -> "GRUB"
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

pub struct App<'a> {
    pub step: Step,
    pub list: ListState,
    pub filter: String,
    pub form: Vec<Field>,
    pub focus: usize,
    /// ExtraMounts: partition waiting for its mount point
    pub mount_for: Option<String>,
    pub error: Option<String>,
    pub done: Vec<Step>,

    pub cfg: Config,
    pub filesystem: Filesystem<'a>,
    pub devices: Vec<BlockDevice>,
    pub cpu: Option<String>,
    pub uefi: bool,
    pub timezones: Vec<String>,
    pub locales: Vec<String>,
}

impl<'a> App<'a> {
    pub fn new(logger: &'a Logger) -> App<'a> {
        let timezones = parse_tzdata(&fs::read_to_string("/usr/share/zoneinfo/tzdata.zi").unwrap_or_default());
        let locales = parse_locale_gen(&fs::read_to_string("/etc/locale.gen").unwrap_or_default());
        let mut app = App {
            step: Step::Partition,
            list: ListState::default(),
            filter: String::new(),
            form: vec![],
            focus: 0,
            mount_for: None,
            error: None,
            done: vec![],
            cfg: Config {
                swap_gb: 4,
                timezone: "UTC".into(),
                locale: "en_US.UTF-8 UTF-8".into(),
                gpus: detect_gpus(),
                ..Default::default()
            },
            filesystem: Filesystem::new(logger),
            devices: vec![],
            cpu: get_processor_make(),
            uefi: Path::new("/sys/firmware/efi").exists(),
            timezones: if timezones.is_empty() { vec!["UTC".into()] } else { timezones },
            locales: if locales.is_empty() { vec!["en_US.UTF-8 UTF-8".into()] } else { locales },
        };
        app.refresh_devices();
        app.enter(Step::Partition);
        app
    }

    /// Re-reads disks, e.g. after cfdisk. Forgets mounts whose partition vanished.
    pub fn refresh_devices(&mut self) {
        match self.filesystem.lsblk() {
            Ok(devices) => self.devices = devices,
            Err(e) => self.error = Some(format!("lsblk failed: {e}")),
        }
        let paths: Vec<String> = self.devices.iter().map(|d| d.path.clone()).collect();
        self.filesystem.partitions.retain(|_, p| paths.contains(p));
    }

    pub fn disks(&self) -> Vec<&BlockDevice> {
        self.devices.iter().filter(|d| d.kind == "disk").collect()
    }

    pub fn partitions(&self) -> Vec<&BlockDevice> {
        self.devices.iter().filter(|d| d.kind == "part").collect()
    }

    pub fn mount_of(&self, partition: &str) -> Option<&str> {
        self.filesystem.partitions.iter().find(|(_, p)| *p == partition).map(|(m, _)| m.as_str())
    }

    pub fn skipped(&self, step: Step) -> bool {
        match step {
            Step::FormatHome => self.filesystem.get("home").is_none(),
            Step::Nvidia => !self.cfg.has_gpu(GpuVendor::Nvidia),
            Step::Amd => !self.cfg.has_gpu(GpuVendor::Amd),
            _ => false,
        }
    }

    pub fn is_form(&self) -> bool {
        self.step == Step::Accounts || self.mount_for.is_some()
    }

    pub fn filterable(&self) -> bool {
        matches!(self.step, Step::Mirrors | Step::Timezone | Step::Locale)
    }

    /// Every option of the current list step, unfiltered.
    pub fn options(&self) -> Vec<String> {
        let labels = |t: &[&str]| t.iter().map(|s| s.to_string()).collect();
        let partitions = |first: Option<&str>| {
            first.map(String::from).into_iter().chain(self.partitions().iter().map(|p| {
                match self.mount_of(&p.path) {
                    Some(m) => format!("{}  → {}", p.describe(), show_mount(m)),
                    None => p.describe(),
                }
            }))
            .collect()
        };
        match self.step {
            Step::Partition => std::iter::once("Done partitioning, continue".to_string())
                .chain(self.disks().iter().map(|d| format!("Edit {}", d.describe())))
                .collect(),
            Step::Boot | Step::Root => partitions(None),
            Step::Home => partitions(Some("No separate /home partition")),
            Step::ExtraMounts => partitions(Some("Done, continue")),
            Step::FormatBoot => labels(&[
                "No - keep it (other OSes' bootloaders live here)",
                "Yes - format as FAT32, erasing everything on it",
            ]),
            Step::FormatHome => labels(&["No - keep the existing files", "Yes - format as ext4, erasing everything on it"]),
            Step::Mirrors => labels(&COUNTRIES),
            Step::Swap => SWAP_SIZES
                .iter()
                .map(|&s| if s == 0 { "No swap".to_string() } else { format!("{s} GB") })
                .collect(),
            Step::Timezone => self.timezones.clone(),
            Step::Locale => self.locales.clone(),
            Step::Bootloader => BOOTLOADERS.iter().map(|(_, l)| l.to_string()).collect(),
            Step::Privilege => PRIVILEGE.iter().map(|(_, l)| l.to_string()).collect(),
            Step::Nvidia => NVIDIA.iter().map(|(_, l)| l.to_string()).collect(),
            Step::Amd => AMD.iter().map(|(_, l)| l.to_string()).collect(),
            Step::Desktop => DESKTOPS.iter().map(|(_, l)| l.to_string()).collect(),
            Step::Browser => BROWSERS.iter().map(|(_, l)| l.to_string()).collect(),
            Step::Accounts | Step::Review => vec![],
        }
    }

    /// Indices into options() matching the filter.
    pub fn visible(&self) -> Vec<usize> {
        let filter = self.filter.to_lowercase();
        self.options()
            .iter()
            .enumerate()
            .filter(|(_, o)| o.to_lowercase().contains(&filter))
            .map(|(i, _)| i)
            .collect()
    }

    fn partition_index(&self, offset: usize, partition: Option<&String>) -> Option<usize> {
        let i = self.partitions().iter().position(|p| Some(&p.path) == partition)?;
        Some(i + offset)
    }

    /// Whether `mount_point`'s partition has no filesystem yet.
    fn unformatted(&self, mount_point: &str) -> bool {
        let p = self.filesystem.get(mount_point);
        self.devices.iter().any(|d| Some(&d.path) == p && d.fstype.is_none())
    }

    fn enter(&mut self, step: Step) {
        self.step = step;
        self.filter.clear();
        self.mount_for = None;
        let selected = match step {
            Step::Boot => self.partition_index(0, self.filesystem.get("boot")).or_else(|| {
                self.partitions().iter().position(|p| p.parttypename.as_deref() == Some("EFI System"))
            }),
            Step::Root => self.partition_index(0, self.filesystem.get("/")),
            Step::Home => self.partition_index(1, self.filesystem.get("home")),
            Step::FormatBoot if self.done.contains(&step) => Some(self.filesystem.format_boot as usize),
            Step::FormatBoot => Some(self.unformatted("boot") as usize),
            Step::FormatHome if self.done.contains(&step) => Some(self.filesystem.format_home as usize),
            Step::FormatHome => Some(self.unformatted("home") as usize),
            Step::Mirrors => COUNTRIES.iter().position(|c| *c == self.cfg.mirror_country),
            Step::Swap => SWAP_SIZES.iter().position(|s| *s == self.cfg.swap_gb),
            Step::Timezone => self.timezones.iter().position(|t| *t == self.cfg.timezone),
            Step::Locale => self.locales.iter().position(|l| *l == self.cfg.locale),
            Step::Bootloader => Some(index_of(&BOOTLOADERS, self.cfg.bootloader)),
            Step::Privilege => Some(index_of(&PRIVILEGE, self.cfg.super_user_utility)),
            Step::Nvidia => Some(index_of(&NVIDIA, self.cfg.nvidia)),
            Step::Amd => Some(index_of(&AMD, self.cfg.amd)),
            Step::Desktop => Some(index_of(&DESKTOPS, self.cfg.desktop)),
            Step::Browser => Some(index_of(&BROWSERS, self.cfg.browser)),
            Step::Accounts => {
                let c = &self.cfg;
                self.form = vec![
                    Field::new("Hostname", &c.hostname, false),
                    Field::new("Username", &c.username, false),
                    Field::new("Password", &c.password, true),
                    Field::new("Confirm password", &c.password, true),
                    Field::new("Root password", &c.root_password, true),
                    Field::new("Confirm root password", &c.root_password, true),
                ];
                self.focus = 0;
                None
            }
            _ => None,
        };
        self.list.select(Some(selected.unwrap_or(0)));
    }

    fn go(&mut self, forward: bool) -> Action {
        let i = STEPS.iter().position(|s| *s == self.step).unwrap_or(0);
        let mut candidates: Box<dyn Iterator<Item = &Step>> = if forward {
            Box::new(STEPS[i + 1..].iter())
        } else {
            Box::new(STEPS[..i].iter().rev())
        };
        match candidates.find(|s| !self.skipped(**s)) {
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
        self.go(true)
    }

    pub fn on_key(&mut self, key: KeyEvent) -> Action {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Action::Quit;
        }
        self.error = None;
        let result = if self.is_form() {
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

    fn review_key(&mut self, key: KeyEvent) -> Result<Action> {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if !self.uefi {
                    bail!("This machine booted in BIOS mode. 2lazy4arch only installs on UEFI.");
                }
                if let Some(step) = STEPS.iter().find(|s| !self.skipped(**s) && **s != Step::Review && !self.done.contains(s)) {
                    bail!("{step:?} isn't set yet. Go back with esc.");
                }
                Ok(Action::Install)
            }
            KeyCode::Esc => Ok(self.go(false)),
            _ => bail!("Press y to install, esc to go back."),
        }
    }

    fn list_key(&mut self, key: KeyEvent) -> Result<Action> {
        let visible = self.visible();
        let n = visible.len();
        let filterable = self.filterable();
        let at = self.list.selected().unwrap_or(0);
        let wrap = |d: isize| ((at as isize + d).rem_euclid(n.max(1) as isize)) as usize;
        let clamp = |d: isize| (at as isize + d).clamp(0, n.saturating_sub(1) as isize) as usize;

        match key.code {
            KeyCode::Up => self.list.select(Some(wrap(-1))),
            KeyCode::Down => self.list.select(Some(wrap(1))),
            KeyCode::Char('k') if !filterable => self.list.select(Some(wrap(-1))),
            KeyCode::Char('j') if !filterable => self.list.select(Some(wrap(1))),
            KeyCode::PageUp => self.list.select(Some(clamp(-10))),
            KeyCode::PageDown => self.list.select(Some(clamp(10))),
            KeyCode::Home => self.list.select(Some(0)),
            KeyCode::End => self.list.select(Some(n.saturating_sub(1))),
            KeyCode::Char(c) if filterable => {
                self.filter.push(c);
                self.list.select(Some(0));
            }
            KeyCode::Backspace if filterable => {
                self.filter.pop();
                self.list.select(Some(0));
            }
            KeyCode::Esc if !self.filter.is_empty() => {
                self.filter.clear();
                self.list.select(Some(0));
            }
            KeyCode::Esc => return Ok(self.go(false)),
            KeyCode::Enter => {
                if let Some(&i) = visible.get(at) {
                    return self.choose(i);
                }
            }
            _ => {}
        }
        Ok(Action::None)
    }

    /// The user picked option `i` (index into options()).
    fn choose(&mut self, i: usize) -> Result<Action> {
        let partition = |offset: usize| self.partitions().get(i.wrapping_sub(offset)).map(|p| p.path.clone());
        match self.step {
            Step::Partition if i > 0 => return Ok(Action::Partition(self.disks()[i - 1].path.clone())),
            Step::Partition => {}
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
                    Some(m) => self.filesystem.set(&m, None)?,
                    None => {
                        self.form = vec![Field::new("Mount point", "/", false)];
                        self.focus = 0;
                        self.mount_for = Some(partition);
                    }
                }
                return Ok(Action::None);
            }
            Step::ExtraMounts => {}
            Step::Mirrors => self.cfg.mirror_country = COUNTRIES[i].to_string(),
            Step::Swap => self.cfg.swap_gb = SWAP_SIZES[i],
            Step::Timezone => self.cfg.timezone = self.timezones[i].clone(),
            Step::Locale => self.cfg.locale = self.locales[i].clone(),
            Step::Bootloader => self.cfg.bootloader = BOOTLOADERS[i].0,
            Step::Privilege => self.cfg.super_user_utility = PRIVILEGE[i].0,
            Step::Nvidia => self.cfg.nvidia = NVIDIA[i].0,
            Step::Amd => self.cfg.amd = AMD[i].0,
            Step::Desktop => self.cfg.desktop = DESKTOPS[i].0,
            Step::Browser => self.cfg.browser = BROWSERS[i].0,
            Step::Accounts | Step::Review => {}
        }
        Ok(self.finish_step())
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
            KeyCode::Esc if self.mount_for.is_some() => self.mount_for = None,
            KeyCode::Esc => return Ok(self.go(false)),
            _ => {}
        }
        Ok(Action::None)
    }

    fn submit_form(&mut self) -> Result<Action> {
        if let Some(partition) = self.mount_for.clone() {
            let mount_point = self.form[0].value.trim().trim_matches('/').to_string();
            if !is_valid_mount_point(&mount_point) || ["boot", "home"].contains(&mount_point.as_str()) {
                bail!("Pick a path like /data or /mnt/windows (letters, digits, - _ .).");
            }
            self.filesystem.set(&mount_point, Some(&partition))?;
            self.mount_for = None;
            return Ok(Action::None);
        }

        let v: Vec<&str> = self.form.iter().map(|f| f.value.as_str()).collect();
        let checks = [
            (0, valid_hostname(v[0]), "Hostname: lowercase letters, digits and -, at most 63 characters."),
            (1, valid_username(v[1]), "Username: lowercase letters, digits, - and _, starting with a letter."),
            (2, !v[2].is_empty(), "Password can't be empty."),
            (3, v[2] == v[3], "Passwords don't match."),
            (4, !v[4].is_empty(), "Root password can't be empty."),
            (5, v[4] == v[5], "Root passwords don't match."),
        ];
        if let Some((field, _, msg)) = checks.iter().find(|c| !c.1) {
            self.focus = *field;
            bail!(*msg);
        }
        self.cfg.hostname = v[0].to_string();
        self.cfg.username = v[1].to_string();
        self.cfg.password = v[2].to_string();
        self.cfg.root_password = v[4].to_string();
        Ok(self.finish_step())
    }
}

fn valid_hostname(s: &str) -> bool {
    (1..=63).contains(&s.len())
        && !s.starts_with('-')
        && !s.ends_with('-')
        && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn valid_username(s: &str) -> bool {
    let mut chars = s.chars();
    s.len() <= 32
        && s != "root"
        && chars.next().is_some_and(|c| c.is_ascii_lowercase() || c == '_')
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// Zone names from tzdata.zi ("Z name ..." and "L target name"), what timedatectl lists.
fn parse_tzdata(s: &str) -> Vec<String> {
    let mut zones: Vec<String> = s
        .lines()
        .filter_map(|line| {
            let mut words = line.split(' ');
            match words.next()? {
                "Z" => words.next(),
                "L" => words.nth(1),
                _ => None,
            }
        })
        .map(String::from)
        .collect();
    zones.sort();
    zones.dedup();
    zones
}

/// "#en_US.UTF-8 UTF-8  " -> "en_US.UTF-8 UTF-8". Skips the comment header.
fn parse_locale_gen(s: &str) -> Vec<String> {
    s.lines()
        .filter_map(|line| {
            let line = line.strip_prefix('#').unwrap_or(line);
            let words: Vec<&str> = line.split_whitespace().collect();
            (!line.starts_with(char::is_whitespace) && words.len() == 2).then(|| words.join(" "))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui;
    use crossterm::event::KeyEvent;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn parsers() {
        assert_eq!(
            parse_tzdata("# version\nZ Asia/Kolkata 5:53:28 - LMT 1854\nL Asia/Kolkata Asia/Calcutta\nR x 1 2"),
            ["Asia/Calcutta", "Asia/Kolkata"]
        );
        assert_eq!(
            parse_locale_gen("# Configuration file\n#\n#  en_US ISO-8859-1\n#     <locale> <charset>\n#aa_DJ.UTF-8 UTF-8  \nen_US.UTF-8 UTF-8"),
            ["aa_DJ.UTF-8 UTF-8", "en_US.UTF-8 UTF-8"]
        );
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

    /// Walks the whole wizard on fake hardware, rendering every screen at 80x25 (Linux console size).
    #[test]
    fn wizard_end_to_end() {
        let logger = Logger::new(false);
        let mut app = App::new(&logger);
        let part = |path: &str, fstype: Option<&str>, kind: &str| BlockDevice {
            path: path.into(),
            kind: kind.into(),
            size: Some("100G".into()),
            fstype: fstype.map(String::from),
            parttypename: (path == "/dev/sda1").then(|| "EFI System".into()),
            label: None,
            model: None,
        };
        app.devices = vec![
            part("/dev/sda", None, "disk"),
            part("/dev/sda1", Some("vfat"), "part"),
            part("/dev/sda2", None, "part"),
            part("/dev/sda3", Some("ext4"), "part"),
            part("/dev/sda4", Some("ntfs"), "part"),
        ];
        app.cfg.gpus = vec![GpuVendor::Amd, GpuVendor::Nvidia];
        app.uefi = true;
        let mut terminal = Terminal::new(TestBackend::new(80, 25)).unwrap();
        let mut render = |app: &mut App| {
            terminal.draw(|f| ui::draw(f, app)).unwrap();
            let buffer = terminal.backend().buffer().clone();
            let text: String = buffer.content.chunks(80).map(|row| row.iter().map(|c| c.symbol()).collect::<String>() + "\n").collect();
            if std::env::var("SHOW_SCREENS").is_ok() {
                println!("{text}");
            }
            text
        };

        // Partition: picking the disk asks main to run cfdisk
        render(&mut app);
        press(&mut app, KeyCode::Down);
        assert!(matches!(press(&mut app, KeyCode::Enter), Action::Partition(d) if d == "/dev/sda"));
        press(&mut app, KeyCode::Up);
        press(&mut app, KeyCode::Enter);

        // Boot: the EFI partition is preselected and already formatted, so "keep" is the default
        assert_eq!(app.step, Step::Boot);
        render(&mut app);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.list.selected(), Some(0));
        press(&mut app, KeyCode::Enter);

        // Root: picking the EFI partition again is refused
        assert_eq!(app.step, Step::Root);
        press(&mut app, KeyCode::Enter);
        assert!(app.error.as_deref().unwrap().contains("already used"));
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Enter);

        // Home on sda3, keep data
        assert_eq!(app.step, Step::Home);
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.step, Step::FormatHome);
        press(&mut app, KeyCode::Enter);

        // Extra mount: sda4 at /data
        assert_eq!(app.step, Step::ExtraMounts);
        press(&mut app, KeyCode::End);
        press(&mut app, KeyCode::Enter);
        assert!(app.is_form());
        typed(&mut app, "data");
        render(&mut app);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.filesystem.get("data").map(String::as_str), Some("/dev/sda4"));
        press(&mut app, KeyCode::Home);
        press(&mut app, KeyCode::Enter);

        // Mirrors: filter
        assert_eq!(app.step, Step::Mirrors);
        typed(&mut app, "ind");
        render(&mut app);
        assert_eq!(app.visible().len(), 2); // India, Indonesia
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.cfg.mirror_country, "India");

        press(&mut app, KeyCode::Enter); // swap
        press(&mut app, KeyCode::Enter); // timezone
        press(&mut app, KeyCode::Enter); // locale

        // Accounts: validation then success
        assert_eq!(app.step, Step::Accounts);
        typed(&mut app, "box");
        press(&mut app, KeyCode::Tab);
        typed(&mut app, "arch");
        press(&mut app, KeyCode::Tab);
        typed(&mut app, "pw");
        press(&mut app, KeyCode::Tab);
        typed(&mut app, "px");
        press(&mut app, KeyCode::Tab);
        typed(&mut app, "root");
        press(&mut app, KeyCode::Tab);
        typed(&mut app, "root");
        press(&mut app, KeyCode::Enter);
        assert_eq!((app.focus, app.error.is_some()), (3, true));
        render(&mut app);
        press(&mut app, KeyCode::Backspace);
        typed(&mut app, "w");
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.step, Step::Bootloader);

        press(&mut app, KeyCode::Enter); // grub
        press(&mut app, KeyCode::Enter); // sudo
        assert_eq!(app.step, Step::Nvidia);
        render(&mut app);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.step, Step::Amd);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.step, Step::Desktop);
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.cfg.desktop, Desktop::Kde);
        press(&mut app, KeyCode::Up);
        press(&mut app, KeyCode::Up);
        press(&mut app, KeyCode::Up);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.cfg.browser, Browser::Zen);

        assert_eq!(app.step, Step::Review);
        let screen = render(&mut app);
        assert!(screen.contains("/dev/sda2"), "{screen}");
        assert!(matches!(press(&mut app, KeyCode::Char('y')), Action::Install));

        // Esc walks back, skipping nothing that applies
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.step, Step::Browser);
    }
}
