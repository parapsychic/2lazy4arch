use installer::{
    config::{Config, ConnectionType, GpuDriver, IpMethod, Listen},
    storage::{self, Part},
    system::{GpuVendor, System},
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, HighlightSpacing, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::app::*;

// Only glyphs the Linux console's default font can draw (no ✔, no rounded corners).
const ACCENT: Color = Color::Cyan;
const DIM: Color = Color::DarkGray;

pub fn draw(f: &mut Frame, app: &mut App) {
    let rows = split(Direction::Vertical, f.size(), [Constraint::Length(1), Constraint::Min(0), Constraint::Length(1), Constraint::Length(1)]);
    header(f, rows[0], app);

    let main = if rows[1].width >= 72 && !app.preview {
        let cols = split(Direction::Horizontal, rows[1], [Constraint::Length(30), Constraint::Min(0)]);
        sidebar(f, cols[0], app);
        cols[1]
    } else {
        rows[1]
    };
    panel(f, main, app);

    if let Some(e) = &app.error {
        f.render_widget(Paragraph::new(format!(" {e}")).fg(Color::Red).bold(), rows[2]);
    }
    f.render_widget(Paragraph::new(format!(" {}", hints(app))).fg(DIM), rows[3]);
}

fn split<const N: usize>(direction: Direction, area: Rect, constraints: [Constraint; N]) -> std::rc::Rc<[Rect]> {
    Layout::default().direction(direction).constraints(constraints).split(area)
}

fn gpu_names(sys: &System) -> String {
    let names: Vec<&str> = sys
        .gpus
        .iter()
        .map(|g| match g.vendor {
            GpuVendor::Intel => "Intel",
            GpuVendor::Amd => "AMD",
            GpuVendor::Nvidia => "NVIDIA",
            GpuVendor::Other => "other",
        })
        .collect();
    if names.is_empty() {
        "none found".into()
    } else {
        names.join(" + ")
    }
}

fn header(f: &mut Frame, area: Rect, app: &App) {
    let cpu = match app.sys.cpu.as_deref() {
        Some("amd") => "AMD",
        Some("intel") => "Intel",
        _ => "unknown",
    };
    let firmware = if app.sys.uefi {
        Span::styled("UEFI", Style::new().fg(Color::Green))
    } else {
        Span::styled("BIOS - not supported, boot the ISO in UEFI mode", Style::new().fg(Color::Red).bold())
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" 2lazy4arch ", Style::new().fg(Color::Black).bg(ACCENT).bold()),
            format!("  CPU {cpu}   GPU {}   ", gpu_names(&app.sys)).into(),
            firmware,
        ])),
        area,
    );
}

fn sidebar_label(step: Step) -> Option<&'static str> {
    Some(match step {
        Step::Partition => "Partitioning",
        Step::Boot => "EFI partition",
        Step::Root => "Root partition",
        Step::Home => "Home partition",
        Step::ExtraMounts => "Other mounts",
        Step::Swap => "Swap",
        Step::Mirrors => "Mirrors",
        Step::Timezone => "Timezone",
        Step::Locale => "Locale",
        Step::Accounts => "Users",
        Step::MoreUsers => "More users",
        Step::Bootloader => "Bootloader",
        Step::Privilege => "Admin tool",
        Step::Nvidia => "NVIDIA driver",
        Step::Amd => "AMD driver",
        Step::Desktop => "Desktop",
        Step::Autologin => "Autologin",
        Step::Browser => "Browser",
        Step::Wifi => "Wi-Fi",
        Step::Extras => "Extras",
        Step::Packages => "Packages",
        Step::Finish => "When done",
        Step::Review => "Install",
        Step::FormatBoot | Step::FormatHome | Step::Shell => return None,
    })
}

fn device_name(path: Option<&String>) -> String {
    path.map_or("none".into(), |p| p.trim_start_matches("/dev/").to_string())
}

fn summary(app: &App, step: Step) -> String {
    let fs = &app.filesystem;
    let c = &app.cfg;
    let format = |yes: bool| if yes { " (format)" } else { "" };
    match step {
        Step::Partition => match c.storage.partitioning.first() {
            Some(plan) => format!("{} {}", if plan.wipe { "erase" } else { "free space" }, device_name(Some(&plan.disk))),
            None => "existing".into(),
        },
        Step::Boot => format!("{}{}", device_name(fs.get("boot")), format(fs.format_boot)),
        Step::Root => device_name(fs.get("/")),
        Step::Home => format!("{}{}", device_name(fs.get("home")), format(fs.format_home && fs.get("home").is_some())),
        Step::ExtraMounts => match c.storage.other.len() {
            0 => "none".into(),
            n => n.to_string(),
        },
        Step::Swap => c.storage.swap.0.map_or("none".into(), |gb| format!("{gb} GB")),
        Step::Mirrors => c.mirrors.clone(),
        Step::Timezone => c.timezone.clone(),
        Step::Locale => c.locale.clone(),
        Step::Accounts => c.users.first().map(|u| u.name.clone()).unwrap_or_default(),
        Step::MoreUsers => match c.users.len().saturating_sub(1) {
            0 => "none".into(),
            n => n.to_string(),
        },
        Step::Bootloader => short(&BOOTLOADERS, c.bootloader).into(),
        Step::Privilege => short(&PRIVILEGE, c.admin_tool).into(),
        Step::Nvidia => c.gpu_drivers(&app.sys.gpus).into_iter().find(|d| d.is_nvidia()).map_or("", |d| short(&NVIDIA, d)).into(),
        Step::Amd => c.gpu_drivers(&app.sys.gpus).into_iter().find(|d| !d.is_nvidia()).map_or("", |d| short(&AMD, d)).into(),
        Step::Desktop => short(&DESKTOPS, c.desktop).into(),
        Step::Autologin => c.autologin.clone().unwrap_or("no".into()),
        Step::Browser => short(&BROWSERS, c.browser()).into(),
        Step::Wifi => c.network.connections.first().and_then(|w| w.ssid.clone()).unwrap_or("none".into()),
        Step::Extras => {
            let on: Vec<&str> = EXTRAS.iter().filter(|(e, _)| app.extra_on(*e)).map(|(_, l)| l.split(' ').next().unwrap_or(l)).collect();
            if on.is_empty() {
                "none".into()
            } else {
                on.join(", ")
            }
        }
        Step::Packages => format!("{} + {} AUR", c.packages.pacman.len(), c.packages.aur.len()),
        Step::Finish => short(&FINISH, c.finish).into(),
        _ => String::new(),
    }
}

fn sidebar(f: &mut Frame, area: Rect, app: &App) {
    let current = match app.step {
        Step::FormatBoot => Step::Boot,
        Step::FormatHome => Step::Home,
        Step::Shell => Step::Accounts,
        s => s,
    };
    let mut current_line = 0;
    let lines: Vec<Line> = STEPS
        .iter()
        .filter(|s| !app.skipped(**s))
        .filter_map(|&s| Some((s, sidebar_label(s)?)))
        .enumerate()
        .map(|(i, (s, label))| {
            let (marker, style) = if s == current {
                current_line = i;
                ("► ", Style::new().fg(ACCENT).bold())
            } else if app.done.contains(&s) {
                ("√ ", Style::new().fg(Color::Green))
            } else {
                ("· ", Style::new().fg(DIM))
            };
            let detail = if app.done.contains(&s) { summary(app, s) } else { String::new() };
            Line::from(vec![Span::styled(format!("{marker}{label:<15}"), style), Span::styled(detail, Style::new().fg(DIM))])
        })
        .collect();
    // keep the current step in view on small screens
    let height = area.height.saturating_sub(2) as usize;
    let scroll = current_line.saturating_sub(height.saturating_sub(3)).min(lines.len().saturating_sub(height));
    f.render_widget(Paragraph::new(lines).scroll((scroll as u16, 0)).block(Block::default().borders(Borders::ALL).title(" Steps ")), area);
}

fn title(app: &App) -> String {
    match (&app.sub, app.step) {
        (Some(Sub::Mount(_)), _) => "Mount point".into(),
        (Some(Sub::NewUser { admin: true }), _) => "New admin user".into(),
        (Some(Sub::NewUser { .. }), _) => "New user".into(),
        (Some(Sub::Extra(extra)), _) => EXTRAS.iter().find(|(e, _)| e == extra).map_or("", |(_, l)| *l).into(),
        (None, Step::FormatBoot) => "Format the EFI partition?".into(),
        (None, Step::FormatHome) => "Format the home partition?".into(),
        (None, Step::Mirrors) => "Mirror country".into(),
        (None, Step::Swap) => "Swap file".into(),
        (None, Step::Shell) => "Your shell".into(),
        (None, Step::Wifi) => "Wi-Fi for the new system".into(),
        (None, Step::Review) if app.preview => "Preview".into(),
        (None, Step::Review) => "Ready to install".into(),
        (None, s) => sidebar_label(s).unwrap_or_default().into(),
    }
}

fn help(app: &App) -> String {
    let fs = &app.filesystem;
    match (&app.sub, app.step) {
        (Some(Sub::Mount(partition)), _) => format!("Where should {} be mounted? It's mounted as-is, never formatted.", device_name(Some(partition))),
        (Some(Sub::NewUser { .. }), _) => "Admins are in the wheel group and can use sudo.".into(),
        (Some(Sub::Extra(Extra::Ssh)), _) => "OpenSSH, on at boot. With a GitHub user, its public keys go into your authorized_keys and you log in with them instead of a password.".into(),
        (Some(Sub::Extra(Extra::Tailscale)), _) => "With an auth key the machine joins your tailnet on first boot, then the key is deleted. Without one, run sudo tailscale up yourself.".into(),
        (Some(Sub::Extra(_)), _) => "Classic VNC only uses the first 8 characters. VNC isn't encrypted, so keep it on a trusted network.".into(),
        (None, Step::Partition) => "Erase turns a whole disk into a 1 GiB EFI partition and a root partition, losing everything on it. \
            Free space adds the same next to another OS. Or edit a disk in cfdisk (written immediately) and pick an EFI \
            partition (FAT32, type \"EFI System\") and a root partition next."
            .into(),
        (None, Step::Boot) => "Mounted at /boot. If another OS is installed, pick its EFI partition and keep it.".into(),
        (None, Step::FormatBoot) => format!("{}: formatting erases other OSes' bootloaders. Only format a new or unused EFI partition.", device_name(fs.get("boot"))),
        (None, Step::Root) => "Mounted at /. It will be FORMATTED as ext4, erasing everything on it.".into(),
        (None, Step::Home) => "Optional separate /home. Keep an existing one to carry your files over.".into(),
        (None, Step::FormatHome) => format!("{} is mounted at /home.", device_name(fs.get("home"))),
        (None, Step::ExtraMounts) => "Optional: mount more partitions, like shared data or Windows drives. They're never formatted. Pick one to add or remove it.".into(),
        (None, Step::Swap) => "Size of /swapfile. Match your RAM if you want to hibernate.".into(),
        (None, Step::Mirrors) => "Pacman downloads from the fastest mirrors in this country, ranked by reflector.".into(),
        (None, Step::Timezone) => "Type a city, e.g. kolkata or new_york.".into(),
        (None, Step::Locale) => "System language and encoding. Pick a UTF-8 one unless you know otherwise.".into(),
        (None, Step::Accounts) => "You're an admin (sudo). Leave the root password empty to lock root; you'll use sudo instead.".into(),
        (None, Step::Shell) => "Your login shell.".into(),
        (None, Step::MoreUsers) => "Optional: more people on this machine. Pick one to remove it.".into(),
        (None, Step::Bootloader) => "Both boot through UEFI.".into(),
        (None, Step::Privilege) => "How admins run commands as root.".into(),
        (None, Step::Nvidia) => {
            let hybrid = app.sys.has_gpu(GpuVendor::Intel) || app.sys.has_gpu(GpuVendor::Amd);
            let prefix = if hybrid { "Hybrid graphics: the screen runs on the integrated GPU, apps use NVIDIA through prime-run. " } else { "" };
            format!("{prefix}Picked from your card's generation. Legacy is nvidia-580xx, built from the AUR during the install.")
        }
        (None, Step::Amd) => "The kernel driver is always amdgpu. PRO adds AMD's proprietary Vulkan driver and AMF encoder from the AUR.".into(),
        (None, Step::Desktop) => "Everything except None boots to a login screen. DWM is built from suckless git, sources in /usr/local/src.".into(),
        (None, Step::Autologin) => "Skip the login screen and go straight to the desktop as this user.".into(),
        (None, Step::Browser) => "AUR browsers are built during the install.".into(),
        (None, Step::Wifi) => "Saved for the new system. Leave the name empty to skip; ethernet works without setup.".into(),
        (None, Step::Extras) => "Enter turns an item on or off.".into(),
        (None, Step::Packages) => "Optional, separated by spaces. Names that don't exist are skipped and listed at the end.".into(),
        (None, Step::Finish) => "What happens once everything is installed. Reboot and power off wait 10 seconds; ctrl+c stays.".into(),
        (None, Step::Review) if app.preview => "This is what the config will do. y installs, esc quits without changing anything.".into(),
        (None, Step::Review) => "Press y to start. Esc goes back to change something.".into(),
    }
}

fn hints(app: &App) -> &'static str {
    if app.is_form() {
        "tab/↑↓ switch field   enter next/confirm   esc back   ctrl+c quit"
    } else if app.step == Step::Review {
        if app.preview {
            "↑↓ pgup pgdn scroll   y install   esc quit"
        } else {
            "↑↓ pgup pgdn scroll   y install   esc back   ctrl+c quit"
        }
    } else if app.filterable() {
        "type to filter   ↑↓ move   enter select   esc back   ctrl+c quit"
    } else if app.step == Step::Partition {
        "↑↓/jk move   enter select   esc quit"
    } else {
        "↑↓/jk move   enter select   esc back   ctrl+c quit"
    }
}

fn panel(f: &mut Frame, area: Rect, app: &mut App) {
    let block = Block::default().borders(Borders::ALL).title(Line::from(format!(" {} ", title(app))).fg(ACCENT).bold());
    let inner = block.inner(area);
    f.render_widget(block, area);

    let help = help(app);
    let width = inner.width.saturating_sub(2).max(1);
    let help_height = help.chars().count() as u16 / width + 2;
    let rows = split(Direction::Vertical, inner, [Constraint::Length(help_height), Constraint::Min(0)]);
    f.render_widget(Paragraph::new(help).wrap(Wrap { trim: true }).fg(Color::Gray), pad(rows[0]));

    let content = pad(rows[1]);
    if app.is_form() {
        form(f, content, app);
    } else if app.step == Step::Review {
        let mut lines: Vec<Line> = app.problems.iter().map(|p| Line::from(Span::styled(format!("! {p}"), Style::new().fg(Color::Red).bold()))).collect();
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        lines.extend(review_lines(&app.cfg, &app.sys));
        let max = (lines.len() as u16).saturating_sub(content.height);
        app.scroll = app.scroll.min(max);
        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((app.scroll, 0)), content);
    } else {
        list(f, content, app);
    }
}

/// One column of breathing room on each side.
fn pad(area: Rect) -> Rect {
    Rect { x: area.x + 1, width: area.width.saturating_sub(2), ..area }
}

fn list(f: &mut Frame, area: Rect, app: &mut App) {
    let area = if app.filterable() {
        let rows = split(Direction::Vertical, area, [Constraint::Length(2), Constraint::Min(0)]);
        let filter = if app.filter.is_empty() {
            Line::from(Span::styled("Filter: type to search", Style::new().fg(DIM)))
        } else {
            Line::from(vec!["Filter: ".fg(DIM), app.filter.clone().bold(), "█".fg(ACCENT)])
        };
        f.render_widget(Paragraph::new(filter), rows[0]);
        rows[1]
    } else {
        area
    };

    let options = app.options();
    let items: Vec<ListItem> = app.visible().iter().map(|&i| ListItem::new(options[i].clone())).collect();
    if items.is_empty() {
        f.render_widget(Paragraph::new("No matches. Backspace to edit the filter.").fg(DIM), area);
        return;
    }
    let list = List::new(items)
        .highlight_style(Style::new().fg(Color::Black).bg(ACCENT).add_modifier(Modifier::BOLD))
        .highlight_symbol("► ")
        .highlight_spacing(HighlightSpacing::Always);
    f.render_stateful_widget(list, area, &mut app.list);
}

fn form(f: &mut Frame, area: Rect, app: &App) {
    let lines: Vec<Line> = app
        .form
        .iter()
        .enumerate()
        .flat_map(|(i, field)| {
            let focused = i == app.focus;
            let value = if field.secret { "•".repeat(field.value.chars().count()) } else { field.value.clone() };
            let label_style = if focused { Style::new().fg(ACCENT).bold() } else { Style::new() };
            [
                Line::from(vec![
                    Span::styled(format!("{}{:<26}", if focused { "► " } else { "  " }, field.label), label_style),
                    Span::raw(value),
                    if focused { "█".fg(ACCENT) } else { "".into() },
                ]),
                Line::default(),
            ]
        })
        .collect();
    f.render_widget(Paragraph::new(lines), area);
}

fn section(title: &str) -> Line<'static> {
    Line::from(Span::styled(title.to_string(), Style::new().fg(ACCENT).bold()))
}

fn row(label: &str, value: impl Into<Span<'static>>) -> Line<'static> {
    Line::from(vec![Span::styled(format!("  {label:<13}"), Style::new().fg(DIM)), value.into()])
}

/// Everything a config will do, on this machine. The wizard's last screen and the
/// --config-file preview.
pub fn review_lines(cfg: &Config, sys: &System) -> Vec<Line<'static>> {
    let erase = |what: String| Span::styled(what, Style::new().fg(Color::Red).bold());
    let mut lines = vec![section("Disks")];
    for plan in &cfg.storage.partitioning {
        let parts: Vec<String> = plan.add.iter().map(|p| format!("{} {}", p.name, p.size)).collect();
        if plan.wipe {
            lines.push(row("ERASE", erase(format!("{}: everything on it, then {}", plan.disk, parts.join(", ")))));
        } else {
            lines.push(row("new", format!("{} free space: {}", plan.disk, parts.join(", "))));
        }
    }
    match storage::plan(&cfg.storage, sys) {
        Ok(plan) => {
            for (mount, part) in &plan.mounts {
                let action = match mount.as_str() {
                    "" => erase("FORMAT ext4".into()),
                    "boot" if plan.format_boot => erase("FORMAT FAT32".into()),
                    "home" if plan.format_home => erase("FORMAT ext4".into()),
                    _ => Span::styled("keep", Style::new().fg(Color::Green)),
                };
                let target = match part {
                    Part::Existing { path, .. } => path.clone(),
                    Part::New { name, .. } => format!("new {name}"),
                };
                lines.push(Line::from(vec![Span::styled(format!("  {:<13}", show_mount(mount)), Style::new().fg(DIM)), Span::raw(format!("{target:<20}")), action]));
            }
        }
        Err(errors) => lines.extend(errors.into_iter().map(|e| row("problem", erase(e)))),
    }
    lines.push(row("Swap", cfg.storage.swap.0.map_or("none".into(), |gb| format!("{gb} GB"))));

    lines.push(section("System"));
    let mut graphics: Vec<&str> = cfg
        .gpu_drivers(&sys.gpus)
        .iter()
        .map(|d| match d {
            GpuDriver::Nvidia => "NVIDIA nvidia-open",
            GpuDriver::NvidiaLegacy => "NVIDIA nvidia-580xx",
            GpuDriver::Nouveau => "NVIDIA nouveau",
            GpuDriver::Amd => "AMD Mesa",
            GpuDriver::AmdPro => "AMD Mesa + PRO",
        })
        .collect();
    if cfg.intel_graphics(&sys.gpus) {
        graphics.insert(0, "Intel Mesa");
    }
    let system = [
        ("Mirrors", cfg.mirrors.clone()),
        ("Timezone", cfg.timezone.clone()),
        ("Locale", cfg.locale.clone()),
        ("Hostname", cfg.hostname.clone()),
        ("Boot / admin", format!("{} / {}", short(&BOOTLOADERS, cfg.bootloader), short(&PRIVILEGE, cfg.admin_tool))),
        ("Microcode", cfg.microcode(sys.cpu.as_deref()).unwrap_or("none").into()),
        ("Graphics", if graphics.is_empty() { "none".into() } else { graphics.join(", ") }),
        ("Desktop", short(&DESKTOPS, cfg.desktop).into()),
        ("Browser", short(&BROWSERS, cfg.browser()).into()),
        ("Autologin", cfg.autologin.clone().unwrap_or("no".into())),
    ];
    lines.extend(system.into_iter().map(|(l, v)| row(l, v)));

    lines.push(section("Users"));
    for (i, user) in cfg.users.iter().enumerate() {
        let mut notes = vec![if cfg.is_admin(i) { "admin" } else { "user" }.to_string(), short(&SHELLS, user.shell).to_string()];
        if !user.groups.is_empty() {
            notes.push(user.groups.join(","));
        }
        if user.has_keys() {
            notes.push(format!("ssh keys{}", user.github.as_ref().map_or(String::new(), |g| format!(" + github.com/{g}"))));
        }
        lines.push(row(&user.name, notes.join(", ")));
    }
    let root = if cfg.root_password.is_some() || cfg.root_password_hash.is_some() { "password set" } else { "locked, use sudo" };
    lines.push(row("root", root));

    let net = &cfg.network;
    if !net.connections.is_empty() || net.dns.is_some() || !net.hosts.is_empty() || net.proxy.is_some() {
        lines.push(section("Network"));
        for c in &net.connections {
            let what = match c.kind {
                ConnectionType::Wifi => format!("wifi {}", c.ssid.as_deref().unwrap_or_default()),
                ConnectionType::Ethernet => format!("ethernet {}", c.interface.as_deref().unwrap_or("any port")),
                ConnectionType::Pppoe => format!("PPPoE on {}", c.interface.as_deref().unwrap_or_default()),
            };
            let ip = c.ipv4.settings();
            let ip = if ip.method == IpMethod::Manual { format!(", static {}", ip.address.unwrap_or_default()) } else { String::new() };
            lines.push(row(&c.name, format!("{what}{ip}")));
        }
        if let Some(dns) = &net.dns {
            let tls = serde_json::to_value(dns.over_tls).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
            lines.push(row("DNS", format!("{} (over TLS: {tls})", dns.servers.join(" "))));
        }
        if !net.hosts.is_empty() {
            lines.push(row("hosts", format!("{} extra", net.hosts.len())));
        }
        if net.proxy.is_some() {
            lines.push(row("proxy", "yes"));
        }
    }

    let listen = |l: Listen| if l == Listen::Tailscale { " (Tailscale only)" } else { "" };
    let mut remote = vec![];
    if let Some(ssh) = cfg.ssh() {
        remote.push(row("SSH", format!("port {}{}", ssh.port, listen(ssh.listen))));
    }
    if let Some(vnc) = cfg.vnc() {
        remote.push(row("VNC", format!("port {}{}{}", vnc.port, if vnc.view_only { ", view only" } else { "" }, listen(vnc.listen))));
    }
    if let Some(ts) = cfg.tailscale() {
        remote.push(row("Tailscale", if ts.auth_key.is_some() { "joins on first boot" } else { "installed, run tailscale up" }));
    }
    if !remote.is_empty() {
        lines.push(section("Remote access"));
        lines.extend(remote);
    }

    let packages = cfg.packages(&sys.gpus);
    lines.push(section("Packages"));
    lines.push(row("multilib", if cfg.packages.multilib || !cfg.rice_users().is_empty() { "yes" } else { "no" }));
    if !cfg.packages.pacman.is_empty() {
        lines.push(row("extra", cfg.packages.pacman.join(" ")));
    }
    if !packages.aur.is_empty() {
        lines.push(row("AUR", Span::styled(packages.aur.join(" "), Style::new().fg(Color::Yellow))));
    }

    if let Some(docker) = &cfg.docker {
        lines.push(section("Docker"));
        let containers: Vec<&str> = docker.containers.keys().map(String::as_str).collect();
        lines.push(row("containers", if containers.is_empty() { "none".into() } else { containers.join(", ") }));
        if !docker.compose_files.is_empty() {
            lines.push(row("compose", docker.compose_files.join(", ")));
        }
    }

    let auto = &cfg.autostart;
    if !auto.services.is_empty() || !auto.apps.is_empty() {
        lines.push(section("Autostart"));
        if !auto.services.is_empty() {
            lines.push(row("services", auto.services.join(", ")));
        }
        if !auto.apps.is_empty() {
            lines.push(row("apps", auto.apps.iter().map(|a| a.name()).collect::<Vec<_>>().join(", ")));
        }
    }

    let hooks = [("pre_install", &cfg.hooks.pre_install), ("post_install", &cfg.hooks.post_install), ("post_setup", &cfg.hooks.post_setup)];
    if hooks.iter().any(|(_, steps)| !steps.is_empty()) {
        lines.push(section("Hooks"));
        for (phase, steps) in hooks.iter().filter(|(_, s)| !s.is_empty()) {
            lines.push(row(phase, steps.iter().map(|s| s.label()).collect::<Vec<_>>().join("; ")));
        }
    }

    let rice = cfg.rice_users();
    if !rice.is_empty() {
        lines.push(section("Rice"));
        lines.push(row("ParaPsychic", rice.join(", ")));
    }
    lines.push(section("When done"));
    lines.push(row("then", short(&FINISH, cfg.finish)));
    lines
}
