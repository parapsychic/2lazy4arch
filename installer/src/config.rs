//! The install config: what unattended-config.yaml describes and the wizard builds.
//! Field docs live in unattended-config-schema.json; defaults match it.

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::{
    essentials::{Bootloader, SuperUserUtility},
    source::Source,
    system::{Gpu, GpuVendor},
};

/// Countries reflector knows, as the wizard lists them.
pub const MIRROR_COUNTRIES: [&str; 70] = [
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

fn yes() -> bool {
    true
}
fn is_true(v: &bool) -> bool {
    *v
}
fn is_false(v: &bool) -> bool {
    !*v
}
fn auto() -> String {
    "auto".into()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    #[serde(default)]
    pub finish: Finish,
    pub storage: Storage,
    #[serde(default = "auto")]
    pub mirrors: String,
    #[serde(default = "default_timezone")]
    pub timezone: String,
    #[serde(default = "default_locale")]
    pub locale: String,
    #[serde(default = "default_hostname")]
    pub hostname: String,
    pub users: Vec<User>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_password: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_password_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub autologin: Option<String>,
    #[serde(default)]
    pub bootloader: Bootloader,
    #[serde(default)]
    pub admin_tool: SuperUserUtility,
    #[serde(default)]
    pub hardware: Hardware,
    #[serde(default)]
    pub desktop: Desktop,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser: Option<Browser>,
    #[serde(default)]
    pub network: Network,
    #[serde(default)]
    pub remote: Remote,
    #[serde(default)]
    pub packages: Packages,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docker: Option<Docker>,
    #[serde(default)]
    pub autostart: Autostart,
    #[serde(default)]
    pub hooks: Hooks,
    #[serde(default)]
    pub parapsychic_mode: ParaMode,
}

fn default_timezone() -> String {
    "UTC".into()
}
fn default_locale() -> String {
    "en_US.UTF-8".into()
}
fn default_hostname() -> String {
    "archlinux".into()
}

impl Default for Config {
    fn default() -> Config {
        Config {
            version: 1,
            finish: Finish::default(),
            storage: Storage::default(),
            mirrors: auto(),
            timezone: default_timezone(),
            locale: default_locale(),
            hostname: default_hostname(),
            users: vec![],
            root_password: None,
            root_password_hash: None,
            autologin: None,
            bootloader: Bootloader::default(),
            admin_tool: SuperUserUtility::default(),
            hardware: Hardware::default(),
            desktop: Desktop::default(),
            browser: None,
            network: Network::default(),
            remote: Remote::default(),
            packages: Packages::default(),
            docker: None,
            autostart: Autostart::default(),
            hooks: Hooks::default(),
            parapsychic_mode: ParaMode::default(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Finish {
    #[default]
    Reboot,
    Poweroff,
    Stay,
}

// ── Storage ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Storage {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub partitioning: Vec<DiskPlan>,
    #[serde(default)]
    pub efi: Efi,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<Root>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home: Option<Home>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub other: Vec<OtherMount>,
    #[serde(default)]
    pub swap: Swap,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Efi {
    #[serde(default = "auto")]
    pub partition: String,
    #[serde(default)]
    pub format: FormatChoice,
}

impl Default for Efi {
    fn default() -> Efi {
        Efi { partition: auto(), format: FormatChoice::Auto }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Root {
    pub partition: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Home {
    pub partition: String,
    #[serde(default)]
    pub format: FormatChoice,
}

#[derive(Clone, Copy, PartialEq, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FormatChoice {
    /// Format a blank partition, keep one that has a filesystem.
    #[default]
    Auto,
    Keep,
    Format,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OtherMount {
    pub partition: String,
    pub mountpoint: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiskPlan {
    pub disk: String,
    #[serde(default)]
    pub wipe: bool,
    #[serde(default = "default_layout")]
    pub add: Vec<NewPartition>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NewPartition {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: PartitionType,
    pub size: String,
}

#[derive(Clone, Copy, PartialEq, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PartitionType {
    Efi,
    Linux,
}

/// arch-efi (efi, 1GiB) + arch-root (linux, rest)
pub fn default_layout() -> Vec<NewPartition> {
    vec![
        NewPartition { name: "arch-efi".into(), kind: PartitionType::Efi, size: "1GiB".into() },
        NewPartition { name: "arch-root".into(), kind: PartitionType::Linux, size: "rest".into() },
    ]
}

/// "1GiB" -> bytes; None for "rest".
pub fn size_bytes(size: &str) -> Option<u64> {
    let (number, unit) = size.split_at(size.find(|c: char| !c.is_ascii_digit())?);
    let shift = match unit {
        "MiB" => 20,
        "GiB" => 30,
        "TiB" => 40,
        _ => return None,
    };
    number.parse::<u64>().ok().map(|n| n << shift)
}

/// Swap file size in GB, None for "none".
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Swap(pub Option<u32>);

impl Default for Swap {
    fn default() -> Swap {
        Swap(Some(4))
    }
}

impl Serialize for Swap {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Some(gb) => s.serialize_u32(gb),
            None => s.serialize_str("none"),
        }
    }
}

impl<'de> Deserialize<'de> for Swap {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Swap, D::Error> {
        match Value::deserialize(d)? {
            Value::String(s) if s == "none" => Ok(Swap(None)),
            Value::Number(n) if n.as_u64().is_some() => Ok(Swap(Some(n.as_u64().unwrap() as u32))),
            other => Err(D::Error::custom(format!("swap: expected none or a size in GB, got {other}"))),
        }
    }
}

// ── Users ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct User {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admin: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,
    #[serde(default)]
    pub shell: LoginShell,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ssh_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github: Option<String>,
}

impl User {
    pub fn home(&self) -> String {
        format!("/home/{}", self.name)
    }

    /// Has keys to log in with over SSH.
    pub fn has_keys(&self) -> bool {
        !self.ssh_keys.is_empty() || self.github.is_some()
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LoginShell {
    #[default]
    Bash,
    Zsh,
    Fish,
}

impl LoginShell {
    pub fn path(self) -> &'static str {
        match self {
            LoginShell::Bash => "/bin/bash",
            LoginShell::Zsh => "/usr/bin/zsh",
            LoginShell::Fish => "/usr/bin/fish",
        }
    }
}

// ── Hardware ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Hardware {
    pub microcode: Microcode,
    pub gpu: GpuChoice,
}

#[derive(Clone, Copy, PartialEq, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Microcode {
    #[default]
    Auto,
    Intel,
    Amd,
    None,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum GpuChoice {
    Keyword(GpuKeyword),
    /// Installed whether or not that GPU is present.
    List(Vec<GpuDriver>),
}

impl Default for GpuChoice {
    fn default() -> GpuChoice {
        GpuChoice::Keyword(GpuKeyword::Auto)
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GpuKeyword {
    Auto,
    None,
}

#[derive(Clone, Copy, PartialEq, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuDriver {
    /// nvidia-open: Turing (GTX 16xx / RTX) and newer
    Nvidia,
    /// nvidia-580xx from the AUR: Maxwell to Volta (GTX 9xx / 10xx)
    NvidiaLegacy,
    Nouveau,
    Amd,
    /// Mesa + AMD's proprietary Vulkan driver and AMF encoder (AUR)
    AmdPro,
}

impl GpuDriver {
    pub fn is_nvidia(self) -> bool {
        matches!(self, GpuDriver::Nvidia | GpuDriver::NvidiaLegacy | GpuDriver::Nouveau)
    }
}

/// The driver `gpu: auto` picks for an NVIDIA card, by PCI device ID.
pub fn nvidia_driver_for(device: u16) -> GpuDriver {
    match device {
        0x1E00.. => GpuDriver::Nvidia,
        0x1340.. => GpuDriver::NvidiaLegacy,
        _ => GpuDriver::Nouveau,
    }
}

// ── Desktop ──────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Desktop {
    /// Built from suckless git (see Essentials::install_dwm)
    Dwm,
    Hyprland,
    Kde,
    Gnome,
    Xfce,
    Lxde,
    #[default]
    None,
}

impl Desktop {
    /// X11 desktops, which get x11vnc
    pub fn is_x11(self) -> bool {
        matches!(self, Desktop::Dwm | Desktop::Xfce | Desktop::Lxde)
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Browser {
    #[default]
    Firefox,
    Librewolf,
    Chromium,
    Epiphany,
    Konqueror,
    Qutebrowser,
    Falkon,
    Zen,
    Chrome,
    None,
}

// ── Network ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Network {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub connections: Vec<Connection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns: Option<Dns>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub hosts: BTreeMap<String, Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy: Option<Proxy>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: ConnectionType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface: Option<String>,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub autoconnect: bool,
    #[serde(default)]
    pub priority: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mac: Option<String>,
    #[serde(default)]
    pub ipv4: IpConfig,
    #[serde(default)]
    pub ipv6: IpConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssid: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub hidden: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security: Option<WifiSecurity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eap: Option<Eap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vlan: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mtu: Option<u32>,
}

impl Connection {
    /// security, defaulting to wpa-psk with a password and open without
    pub fn security(&self) -> WifiSecurity {
        self.security.unwrap_or(if self.password.is_some() { WifiSecurity::WpaPsk } else { WifiSecurity::Open })
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionType {
    Ethernet,
    Wifi,
    Pppoe,
}

#[derive(Clone, Copy, PartialEq, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WifiSecurity {
    WpaPsk,
    Sae,
    Owe,
    Open,
    Eap,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum IpConfig {
    Simple(IpMethod),
    Detailed(IpSettings),
}

impl Default for IpConfig {
    fn default() -> IpConfig {
        IpConfig::Simple(IpMethod::Auto)
    }
}

impl IpConfig {
    pub fn settings(&self) -> IpSettings {
        match self {
            IpConfig::Simple(method) => IpSettings { method: *method, ..Default::default() },
            IpConfig::Detailed(settings) => settings.clone(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum IpMethod {
    #[default]
    Auto,
    Manual,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IpSettings {
    #[serde(default)]
    pub method: IpMethod,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gateway: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dns: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Eap {
    pub method: EapMethod,
    pub identity: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anonymous_identity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(default)]
    pub phase2: Phase2,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ca_cert: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_cert: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub private_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub private_key_password: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EapMethod {
    Peap,
    Ttls,
    Tls,
}

#[derive(Clone, Copy, PartialEq, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase2 {
    #[default]
    Mschapv2,
    Mschap,
    Pap,
    Chap,
    Gtc,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Dns {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub servers: Vec<String>,
    pub over_tls: DnsOverTls,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub search: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DnsOverTls {
    #[default]
    Off,
    Opportunistic,
    Strict,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Proxy {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub https: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_proxy: Option<String>,
}

// ── Remote access ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Remote {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh: Option<Ssh>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vnc: Option<Vnc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tailscale: Option<Tailscale>,
}

#[derive(Clone, Copy, PartialEq, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Listen {
    #[default]
    Everywhere,
    Tailscale,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Ssh {
    pub enable: bool,
    pub port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password_login: Option<bool>,
    pub root_login: bool,
    pub listen: Listen,
}

impl Default for Ssh {
    fn default() -> Ssh {
        Ssh { enable: false, port: 22, password_login: None, root_login: false, listen: Listen::Everywhere }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Vnc {
    pub enable: bool,
    pub port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    pub view_only: bool,
    pub listen: Listen,
}

impl Default for Vnc {
    fn default() -> Vnc {
        Vnc { enable: false, port: 5900, password: None, view_only: false, listen: Listen::Everywhere }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Tailscale {
    pub enable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    pub ssh: bool,
    pub accept_routes: bool,
    pub advertise_exit_node: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub advertise_routes: Vec<String>,
}

// ── Packages, Docker, autostart, hooks ───────────────────────────────────────

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Packages {
    pub multilib: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pacman: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub aur: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pacman_list: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aur_list: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Docker {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub login: Vec<RegistryLogin>,
    /// docker compose services, as written
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub containers: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub volumes: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub networks: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub compose_files: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryLogin {
    #[serde(default = "docker_hub")]
    pub registry: String,
    pub username: String,
    pub password: String,
}

fn docker_hub() -> String {
    "docker.io".into()
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Autostart {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub services: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub apps: Vec<AutostartApp>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AutostartApp {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub users: Option<Vec<String>>,
}

impl AutostartApp {
    /// name, or the command's first word without its path
    pub fn name(&self) -> String {
        self.name.clone().unwrap_or_else(|| {
            let first = self.command.split_whitespace().next().unwrap_or("app");
            first.rsplit('/').next().unwrap_or(first).to_string()
        })
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Hooks {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pre_install: Vec<HookStep>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub post_install: Vec<HookStep>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub post_setup: Vec<HookStep>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HookStep {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    #[serde(default)]
    pub on_error: OnError,
    /// post_install only
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub chroot: bool,
    #[serde(rename = "as", default, skip_serializing_if = "Option::is_none")]
    pub run_as: Option<String>,
}

impl HookStep {
    pub fn label(&self) -> String {
        self.name.clone().or_else(|| self.script.clone()).unwrap_or_else(|| {
            let run = self.run.as_deref().unwrap_or_default();
            run.lines().next().unwrap_or_default().chars().take(60).collect()
        })
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OnError {
    #[default]
    Stop,
    Continue,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ParaMode {
    /// true: the first user
    Enabled(bool),
    Users(Vec<String>),
}

impl Default for ParaMode {
    fn default() -> ParaMode {
        ParaMode::Enabled(false)
    }
}

// ── Derived values ───────────────────────────────────────────────────────────

/// What to install for a config on a given machine.
#[derive(Default, Debug, PartialEq)]
pub struct PackageSet {
    pub repo: Vec<String>,
    /// Built with yay as the first user, inside the new system.
    pub aur: Vec<String>,
    /// Repo packages that depend on an AUR one (nvidia-prime with nvidia-580xx).
    pub after_aur: Vec<String>,
    pub services: Vec<String>,
}

impl Config {
    pub fn main_user(&self) -> Option<&User> {
        self.users.first()
    }

    /// admin defaults to true for the first user, false for the rest
    pub fn is_admin(&self, index: usize) -> bool {
        self.users[index].admin.unwrap_or(index == 0)
    }

    pub fn admins(&self) -> Vec<&str> {
        (0..self.users.len()).filter(|&i| self.is_admin(i)).map(|i| self.users[i].name.as_str()).collect()
    }

    /// firefox with a desktop, none without
    pub fn browser(&self) -> Browser {
        self.browser.unwrap_or(if self.desktop == Desktop::None { Browser::None } else { Browser::Firefox })
    }

    pub fn ssh(&self) -> Option<&Ssh> {
        self.remote.ssh.as_ref().filter(|s| s.enable)
    }

    pub fn vnc(&self) -> Option<&Vnc> {
        self.remote.vnc.as_ref().filter(|v| v.enable)
    }

    pub fn tailscale(&self) -> Option<&Tailscale> {
        self.remote.tailscale.as_ref().filter(|t| t.enable)
    }

    /// Ports that only Tailscale may reach.
    pub fn tailscale_only_ports(&self) -> Vec<u16> {
        let ssh = self.ssh().filter(|s| s.listen == Listen::Tailscale).map(|s| s.port);
        let vnc = self.vnc().filter(|v| v.listen == Listen::Tailscale).map(|v| v.port);
        ssh.into_iter().chain(vnc).collect()
    }

    /// Users that get the ParaPsychic rice.
    pub fn rice_users(&self) -> Vec<String> {
        match &self.parapsychic_mode {
            ParaMode::Enabled(true) => self.main_user().map(|u| u.name.clone()).into_iter().collect(),
            ParaMode::Enabled(false) => vec![],
            ParaMode::Users(users) => users.clone(),
        }
    }

    pub fn microcode(&self, cpu: Option<&str>) -> Option<&'static str> {
        match (self.hardware.microcode, cpu) {
            (Microcode::Intel, _) | (Microcode::Auto, Some("intel")) => Some("intel-ucode"),
            (Microcode::Amd, _) | (Microcode::Auto, Some("amd")) => Some("amd-ucode"),
            _ => None,
        }
    }

    /// Drivers for the GPUs found (or listed). Intel graphics are separate: see intel_graphics.
    pub fn gpu_drivers(&self, gpus: &[Gpu]) -> Vec<GpuDriver> {
        let mut drivers: Vec<GpuDriver> = match &self.hardware.gpu {
            GpuChoice::Keyword(GpuKeyword::None) => vec![],
            GpuChoice::List(list) => list.clone(),
            GpuChoice::Keyword(GpuKeyword::Auto) => gpus
                .iter()
                .filter_map(|g| match g.vendor {
                    GpuVendor::Nvidia => Some(nvidia_driver_for(g.device)),
                    GpuVendor::Amd => Some(GpuDriver::Amd),
                    _ => None,
                })
                .collect(),
        };
        drivers.dedup();
        drivers
    }

    pub fn intel_graphics(&self, gpus: &[Gpu]) -> bool {
        self.hardware.gpu != GpuChoice::Keyword(GpuKeyword::None) && gpus.iter().any(|g| g.vendor == GpuVendor::Intel)
    }

    /// The proprietary NVIDIA driver needs nouveau kept out of the initramfs.
    pub fn nvidia_proprietary(&self, gpus: &[Gpu]) -> bool {
        self.gpu_drivers(gpus).iter().any(|d| matches!(d, GpuDriver::Nvidia | GpuDriver::NvidiaLegacy))
    }

    /// Everything to install beyond the base set, for this config on this machine.
    pub fn packages(&self, gpus: &[Gpu]) -> PackageSet {
        let mut repo: Vec<&str> = vec![];
        let mut aur: Vec<&str> = vec![];
        let mut after_aur: Vec<&str> = vec![];
        let mut services: Vec<&str> = vec![];

        let drivers = self.gpu_drivers(gpus);
        let intel = self.intel_graphics(gpus);
        if intel || !drivers.is_empty() {
            repo.push("mesa");
        }
        if intel {
            repo.extend(["vulkan-intel", "intel-media-driver"]);
        }
        // laptops like the TUF 505DT: iGPU drives the screen, NVIDIA renders on demand (prime-run)
        let hybrid = gpus.iter().any(|g| matches!(g.vendor, GpuVendor::Intel | GpuVendor::Amd));
        for driver in drivers {
            match driver {
                GpuDriver::Nvidia => {
                    repo.extend(["nvidia-open", "nvidia-utils", "nvidia-settings"]);
                    if hybrid {
                        repo.push("nvidia-prime");
                    }
                }
                GpuDriver::NvidiaLegacy => {
                    aur.extend(["nvidia-580xx-dkms", "nvidia-580xx-utils"]);
                    // nvidia-prime needs an nvidia-utils, which 580xx-utils provides once built
                    if hybrid {
                        after_aur.push("nvidia-prime");
                    }
                }
                GpuDriver::Nouveau => repo.push("vulkan-nouveau"),
                GpuDriver::Amd => repo.push("vulkan-radeon"),
                GpuDriver::AmdPro => {
                    repo.push("vulkan-radeon");
                    aur.extend(["vulkan-amdgpu-pro", "amf-amdgpu-pro"]);
                }
            }
        }

        let lightdm = ["lightdm", "lightdm-gtk-greeter"];
        match self.desktop {
            Desktop::Dwm => {
                // build deps for dwm/dmenu/st + X
                repo.extend(["xorg-server", "xorg-xinit", "libx11", "libxft", "libxinerama"]);
                repo.extend(lightdm);
                services.push("lightdm");
            }
            Desktop::Hyprland => {
                // apps referenced by Hyprland's default config
                repo.extend(["hyprland", "kitty", "dolphin", "hyprlauncher", "xdg-desktop-portal-hyprland", "sddm"]);
                services.push("sddm");
            }
            Desktop::Kde => {
                repo.extend(["plasma-meta", "konsole", "dolphin"]);
                services.push("plasmalogin");
            }
            Desktop::Gnome => {
                repo.push("gnome");
                services.push("gdm");
            }
            Desktop::Xfce => {
                repo.extend(["xorg-server", "xfce4", "xfce4-goodies"]);
                repo.extend(lightdm);
                services.push("lightdm");
            }
            Desktop::Lxde => {
                // the lxde group ships lxdm
                repo.extend(["xorg-server", "lxde"]);
                services.push("lxdm");
            }
            Desktop::None => {}
        }

        match self.browser() {
            Browser::Firefox => repo.push("firefox"),
            Browser::Librewolf => repo.push("librewolf"),
            Browser::Chromium => repo.push("chromium"),
            Browser::Epiphany => repo.push("epiphany"),
            Browser::Konqueror => repo.push("konqueror"),
            Browser::Qutebrowser => repo.push("qutebrowser"),
            Browser::Falkon => repo.push("falkon"),
            Browser::Zen => aur.push("zen-browser-bin"),
            Browser::Chrome => aur.push("google-chrome"),
            Browser::None => {}
        }
        // otherwise pacman picks gnu-free-fonts for the ttf-font dependency
        if self.desktop != Desktop::None || self.browser() != Browser::None {
            repo.push("noto-fonts");
        }

        for user in &self.users {
            match user.shell {
                LoginShell::Zsh => repo.push("zsh"),
                LoginShell::Fish => repo.push("fish"),
                LoginShell::Bash => {}
            }
        }
        if self.network.connections.iter().any(|c| c.kind == ConnectionType::Pppoe) {
            repo.push("ppp");
        }
        if self.ssh().is_some() {
            repo.push("openssh");
            services.push("sshd");
        }
        if self.tailscale().is_some() {
            repo.push("tailscale");
            services.push("tailscaled");
        }
        if self.vnc().is_some() {
            repo.push(match self.desktop {
                Desktop::Hyprland => "wayvnc",
                Desktop::Gnome => "gnome-remote-desktop",
                Desktop::Kde => "krfb",
                _ => "x11vnc",
            });
        }
        if !self.tailscale_only_ports().is_empty() {
            repo.push("nftables");
            services.push("nftables");
        }
        if self.docker.is_some() {
            repo.extend(["docker", "docker-compose"]);
            services.push("docker");
        }

        let own = |v: &[&str]| -> Vec<String> {
            let mut out: Vec<String> = vec![];
            for p in v {
                if !out.iter().any(|o| o == p) {
                    out.push(p.to_string());
                }
            }
            out
        };
        let mut set = PackageSet { repo: own(&repo), aur: own(&aur), after_aur: own(&after_aur), services: own(&services) };
        for p in &self.packages.pacman {
            if !set.repo.contains(p) {
                set.repo.push(p.clone());
            }
        }
        for p in &self.packages.aur {
            if !set.aur.contains(p) {
                set.aur.push(p.clone());
            }
        }
        set
    }

    /// Every password and key in the config, to keep out of the install's output.
    pub fn secrets(&self) -> Vec<String> {
        let mut secrets: Vec<Option<String>> = vec![self.root_password.clone(), self.vnc().and_then(|v| v.password.clone()), self.tailscale().and_then(|t| t.auth_key.clone())];
        secrets.extend(self.users.iter().map(|u| u.password.clone()));
        for c in &self.network.connections {
            secrets.push(c.password.clone());
            if let Some(eap) = &c.eap {
                secrets.extend([eap.password.clone(), eap.private_key_password.clone()]);
            }
        }
        if let Some(docker) = &self.docker {
            secrets.extend(docker.login.iter().map(|l| Some(l.password.clone())));
        }
        secrets.into_iter().flatten().collect()
    }

    /// Merges packages.pacman_list / aur_list into pacman / aur.
    pub fn load_lists(&mut self, source: &Source) -> Result<()> {
        let read = |path: &Option<String>| -> Result<Vec<String>> {
            let Some(path) = path else { return Ok(vec![]) };
            let local = source.fetch(path)?;
            let text = std::fs::read_to_string(&local).with_context(|| format!("reading {path}"))?;
            Ok(text
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .map(String::from)
                .collect())
        };
        let pacman = read(&self.packages.pacman_list)?;
        let aur = read(&self.packages.aur_list)?;
        self.packages.pacman.extend(pacman);
        self.packages.aur.extend(aur);
        Ok(())
    }
}

/// The config without its secrets: every password, the Tailscale key and registry
/// logins. Container settings are kept as written.
pub fn strip_secrets(value: &mut Value) {
    const SECRETS: [&str; 6] = ["password", "password_hash", "root_password", "root_password_hash", "private_key_password", "auth_key"];
    fn strip(value: &mut Value) {
        match value {
            Value::Object(map) => {
                map.retain(|k, _| !SECRETS.contains(&k.as_str()));
                for (k, v) in map.iter_mut() {
                    if k != "containers" {
                        strip(v);
                    }
                }
            }
            Value::Array(items) => items.iter_mut().for_each(strip),
            _ => {}
        }
    }
    if let Some(docker) = value.get_mut("docker").and_then(Value::as_object_mut) {
        docker.remove("login");
    }
    strip(value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use GpuVendor::*;

    const AMD: Gpu = Gpu { vendor: Amd, device: 0x15d8 };
    const GTX_1650: Gpu = Gpu { vendor: Nvidia, device: 0x1f99 };
    const GTX_1060: Gpu = Gpu { vendor: Nvidia, device: 0x1c03 };
    const INTEL: Gpu = Gpu { vendor: Intel, device: 0x9a49 };

    fn has(p: &PackageSet, pkg: &str) -> bool {
        p.repo.iter().any(|x| x == pkg)
    }

    #[test]
    fn nvidia_generations() {
        assert_eq!(nvidia_driver_for(0x1f99), GpuDriver::Nvidia); // GTX 1650
        assert_eq!(nvidia_driver_for(0x2684), GpuDriver::Nvidia); // RTX 4090
        assert_eq!(nvidia_driver_for(0x1c03), GpuDriver::NvidiaLegacy); // GTX 1060
        assert_eq!(nvidia_driver_for(0x1380), GpuDriver::NvidiaLegacy); // GTX 750 Ti
        assert_eq!(nvidia_driver_for(0x1180), GpuDriver::Nouveau); // GTX 680
    }

    #[test]
    fn tuf_505dt_auto() {
        let cfg = Config { desktop: Desktop::Kde, browser: Some(Browser::Zen), ..Default::default() };
        let p = cfg.packages(&[AMD, GTX_1650]);
        for pkg in ["vulkan-radeon", "nvidia-open", "nvidia-utils", "nvidia-prime", "plasma-meta", "mesa"] {
            assert!(has(&p, pkg), "missing {pkg}");
        }
        assert_eq!(p.aur, ["zen-browser-bin"]);
        assert_eq!(p.services, ["plasmalogin"]);
        assert!(cfg.nvidia_proprietary(&[AMD, GTX_1650]));
    }

    #[test]
    fn intel_only_no_desktop() {
        let cfg = Config::default();
        let p = cfg.packages(&[INTEL]);
        assert_eq!(p.repo, ["mesa", "vulkan-intel", "intel-media-driver"]);
        assert!(p.aur.is_empty() && p.services.is_empty());
        assert_eq!(cfg.browser(), Browser::None);
    }

    #[test]
    fn legacy_nvidia_and_amd_pro_go_through_yay() {
        let cfg = Config {
            hardware: Hardware { gpu: GpuChoice::List(vec![GpuDriver::NvidiaLegacy, GpuDriver::AmdPro]), ..Default::default() },
            ..Default::default()
        };
        let p = cfg.packages(&[INTEL, GTX_1060]);
        assert!(!p.repo.iter().any(|x| x.starts_with("nvidia")));
        assert_eq!(p.aur, ["nvidia-580xx-dkms", "nvidia-580xx-utils", "vulkan-amdgpu-pro", "amf-amdgpu-pro"]);
        assert_eq!(p.after_aur, ["nvidia-prime"]);
        assert!(has(&p, "vulkan-intel"));
    }

    #[test]
    fn gpu_none_installs_nothing() {
        let cfg = Config { hardware: Hardware { gpu: GpuChoice::Keyword(GpuKeyword::None), ..Default::default() }, ..Default::default() };
        assert!(cfg.packages(&[INTEL, GTX_1650]).repo.is_empty());
    }

    #[test]
    fn example_config_parses() {
        let value: Value = serde_norway::from_str(include_str!("../../unattended-config.yaml")).unwrap();
        let cfg: Config = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(cfg.users.len(), 3);
        assert_eq!(cfg.admins(), ["parapsychic", "alice"]);
        assert_eq!(cfg.storage.swap, Swap(Some(8)));
        assert_eq!(cfg.network.connections[1].ipv4.settings().method, IpMethod::Manual);
        assert_eq!(cfg.autostart.apps[1].name(), "nm-applet");
        assert_eq!(cfg.tailscale_only_ports(), [22, 5900]);
        let p = cfg.packages(&[INTEL]);
        for pkg in ["hyprland", "wayvnc", "openssh", "tailscale", "nftables", "docker", "zsh", "discord"] {
            assert!(has(&p, pkg), "missing {pkg}");
        }

        // round trip: what the wizard writes reads back the same
        let again: Config = serde_json::from_value(serde_json::to_value(&cfg).unwrap()).unwrap();
        assert_eq!(serde_json::to_value(&again).unwrap(), serde_json::to_value(&cfg).unwrap());

        let mut stripped = value;
        strip_secrets(&mut stripped);
        assert!(stripped["users"][0].get("password_hash").is_none());
        assert!(stripped["remote"]["tailscale"].get("auth_key").is_none());
        assert_eq!(stripped["docker"]["containers"]["postgres"]["environment"]["POSTGRES_PASSWORD"], "REPLACE_ME");
    }
}
