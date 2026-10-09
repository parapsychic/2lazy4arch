use installer::{
    config::NvidiaDriver,
    utils::GpuVendor,
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

    let main = if rows[1].width >= 72 {
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

fn header(f: &mut Frame, area: Rect, app: &App) {
    let gpus = app
        .cfg
        .gpus
        .iter()
        .map(|g| match g {
            GpuVendor::Intel => "Intel",
            GpuVendor::Amd => "AMD",
            GpuVendor::Nvidia => "NVIDIA",
            GpuVendor::Other => "other",
        })
        .collect::<Vec<_>>()
        .join(" + ");
    let cpu = match app.cpu.as_deref() {
        Some("amd") => "AMD",
        Some("intel") => "Intel",
        _ => "unknown",
    };
    let firmware = if app.uefi {
        Span::styled("UEFI", Style::new().fg(Color::Green))
    } else {
        Span::styled("BIOS - not supported, boot the ISO in UEFI mode", Style::new().fg(Color::Red).bold())
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" 2lazy4arch ", Style::new().fg(Color::Black).bg(ACCENT).bold()),
            format!("  CPU {cpu}   GPU {}   ", if gpus.is_empty() { "none found" } else { &gpus }).into(),
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
        Step::Mirrors => "Mirrors",
        Step::Swap => "Swap",
        Step::Timezone => "Timezone",
        Step::Locale => "Locale",
        Step::Accounts => "Users",
        Step::Bootloader => "Bootloader",
        Step::Privilege => "Admin tool",
        Step::Nvidia => "NVIDIA driver",
        Step::Amd => "AMD driver",
        Step::Desktop => "Desktop",
        Step::Browser => "Browser",
        Step::Review => "Install",
        Step::FormatBoot | Step::FormatHome => return None,
    })
}

fn device_name(path: Option<&String>) -> String {
    path.map_or("none".into(), |p| p.trim_start_matches("/dev/").to_string())
}

fn summary(app: &App, step: Step) -> String {
    let fs = &app.filesystem;
    let format = |yes: bool| if yes { " (format)" } else { "" };
    match step {
        Step::Boot => format!("{}{}", device_name(fs.get("boot")), format(fs.format_boot)),
        Step::Root => device_name(fs.get("/")),
        Step::Home => format!("{}{}", device_name(fs.get("home")), format(fs.format_home && fs.get("home").is_some())),
        Step::ExtraMounts => match fs.partitions.keys().filter(|k| !["", "boot", "home"].contains(&k.as_str())).count() {
            0 => "none".into(),
            n => n.to_string(),
        },
        Step::Mirrors => app.cfg.mirror_country.clone(),
        Step::Swap if app.cfg.swap_gb == 0 => "none".into(),
        Step::Swap => format!("{} GB", app.cfg.swap_gb),
        Step::Timezone => app.cfg.timezone.clone(),
        Step::Locale => app.cfg.locale.split_whitespace().next().unwrap_or_default().into(),
        Step::Accounts => app.cfg.username.clone(),
        Step::Bootloader => short(&BOOTLOADERS, app.cfg.bootloader).into(),
        Step::Privilege => short(&PRIVILEGE, app.cfg.super_user_utility).into(),
        Step::Nvidia => short(&NVIDIA, app.cfg.nvidia).into(),
        Step::Amd => short(&AMD, app.cfg.amd).into(),
        Step::Desktop => short(&DESKTOPS, app.cfg.desktop).into(),
        Step::Browser => short(&BROWSERS, app.cfg.browser).into(),
        _ => String::new(),
    }
}

fn sidebar(f: &mut Frame, area: Rect, app: &App) {
    let current = match app.step {
        Step::FormatBoot => Step::Boot,
        Step::FormatHome => Step::Home,
        s => s,
    };
    let lines: Vec<Line> = STEPS
        .iter()
        .filter(|s| !app.skipped(**s))
        .filter_map(|&s| Some((s, sidebar_label(s)?)))
        .map(|(s, label)| {
            let (marker, style) = if s == current {
                ("► ", Style::new().fg(ACCENT).bold())
            } else if app.done.contains(&s) {
                ("√ ", Style::new().fg(Color::Green))
            } else {
                ("· ", Style::new().fg(DIM))
            };
            let detail = if app.done.contains(&s) { summary(app, s) } else { String::new() };
            Line::from(vec![
                Span::styled(format!("{marker}{label:<15}"), style),
                Span::styled(detail, Style::new().fg(DIM)),
            ])
        })
        .collect();
    f.render_widget(Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(" Steps ")), area);
}

fn title(app: &App) -> String {
    match app.step {
        Step::FormatBoot => "Format the EFI partition?".into(),
        Step::FormatHome => "Format the home partition?".into(),
        Step::ExtraMounts if app.mount_for.is_some() => "Mount point".into(),
        Step::Mirrors => "Mirror country".into(),
        Step::Swap => "Swap file".into(),
        Step::Review => "Ready to install".into(),
        s => sidebar_label(s).unwrap_or_default().into(),
    }
}

fn help(app: &App) -> String {
    let fs = &app.filesystem;
    match app.step {
        Step::Partition => "Pick a disk to edit its partitions in cfdisk (changes are written immediately), or continue if they're ready. \
            You need an EFI partition (FAT32, ~1 GB, type \"EFI System\") and a root partition.".into(),
        Step::Boot => "Mounted at /boot. If another OS is installed, pick its EFI partition and keep it.".into(),
        Step::FormatBoot => format!("{}: formatting erases other OSes' bootloaders. Only format a new or unused EFI partition.", device_name(fs.get("boot"))),
        Step::Root => "Mounted at /. It will be FORMATTED as ext4, erasing everything on it.".into(),
        Step::Home => "Optional separate /home. Keep an existing one to carry your files over.".into(),
        Step::FormatHome => format!("{} is mounted at /home.", device_name(fs.get("home"))),
        Step::ExtraMounts if app.mount_for.is_some() => {
            format!("Where should {} be mounted? It's mounted as-is, never formatted.", device_name(app.mount_for.as_ref()))
        }
        Step::ExtraMounts => "Optional: mount more partitions, like shared data or Windows drives. They're never formatted. Pick one to add or remove it.".into(),
        Step::Mirrors => "Pacman downloads from the fastest mirrors in this country, ranked by reflector.".into(),
        Step::Swap => "Size of /swapfile. Match your RAM if you want to hibernate.".into(),
        Step::Timezone => "Type a city, e.g. kolkata or new_york.".into(),
        Step::Locale => "System language and encoding. Pick a UTF-8 one unless you know otherwise.".into(),
        Step::Accounts => "Your user joins the wheel group and can use sudo.".into(),
        Step::Bootloader => "Both boot through UEFI.".into(),
        Step::Privilege => "How your user runs commands as root.".into(),
        Step::Nvidia if app.cfg.has_gpu(GpuVendor::Intel) || app.cfg.has_gpu(GpuVendor::Amd) => {
            "Hybrid graphics: the screen runs on the integrated GPU, apps use NVIDIA through prime-run.             Proprietary installs nvidia-open; legacy is nvidia-580xx, built by part 2.".into()
        }
        Step::Nvidia => "An NVIDIA GPU was detected. Proprietary installs nvidia-open; legacy is nvidia-580xx, built by part 2.".into(),
        Step::Amd => "The kernel driver is always amdgpu. PRO adds AMD's proprietary Vulkan driver and AMF encoder, installed by part 2.".into(),
        Step::Desktop => "Everything except None boots to a login screen. DWM is built from suckless git, sources in /usr/local/src.".into(),
        Step::Browser => "AUR browsers are installed by part 2: run 2lazy4arch again after the first boot.".into(),
        Step::Review => "Press y to start. Esc goes back to change something.".into(),
    }
}

fn hints(app: &App) -> &'static str {
    if app.is_form() {
        "tab/↑↓ switch field   enter next/confirm   esc back   ctrl+c quit"
    } else if app.step == Step::Review {
        "y install   esc back   ctrl+c quit"
    } else if app.filterable() {
        "type to filter   ↑↓ move   enter select   esc back   ctrl+c quit"
    } else if app.step == Step::Partition {
        "↑↓/jk move   enter select   esc quit"
    } else {
        "↑↓/jk move   enter select   esc back   ctrl+c quit"
    }
}

fn panel(f: &mut Frame, area: Rect, app: &mut App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Line::from(format!(" {} ", title(app))).fg(ACCENT).bold());
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
        review(f, content, app);
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
                    Span::styled(format!("{}{:<24}", if focused { "► " } else { "  " }, field.label), label_style),
                    Span::raw(value),
                    if focused { "█".fg(ACCENT) } else { "".into() },
                ]),
                Line::default(),
            ]
        })
        .collect();
    f.render_widget(Paragraph::new(lines), area);
}

fn review(f: &mut Frame, area: Rect, app: &App) {
    let fs = &app.filesystem;
    let cfg = &app.cfg;
    let row = |label: &str, value: Span<'static>| Line::from(vec![Span::styled(format!("{label:<13}"), Style::new().fg(DIM)), value]);
    let erase = |what: &str| Span::styled(format!("FORMAT {what}"), Style::new().fg(Color::Red).bold());
    let keep = || Span::styled("keep", Style::new().fg(Color::Green));

    let mut lines = vec![];
    for (mount, partition) in &fs.partitions {
        let action = match mount.as_str() {
            "" => erase("ext4"),
            "boot" if fs.format_boot => erase("FAT32"),
            "home" if fs.format_home => erase("ext4"),
            _ => keep(),
        };
        let label = Span::styled(format!("{:<13}", show_mount(mount)), Style::new().fg(DIM));
        lines.push(Line::from(vec![label, Span::raw(format!("{partition:<16}")), action]));
    }

    let mut graphics = vec![];
    if cfg.has_gpu(GpuVendor::Intel) {
        graphics.push("Intel: Mesa".to_string());
    }
    if cfg.has_gpu(GpuVendor::Amd) {
        graphics.push(format!("AMD: {}", short(&AMD, cfg.amd)));
    }
    if cfg.has_gpu(GpuVendor::Nvidia) {
        let driver = match cfg.nvidia {
            NvidiaDriver::Open => "nvidia-open",
            NvidiaDriver::Legacy => "nvidia-580xx",
            NvidiaDriver::Nouveau => "nouveau",
        };
        graphics.push(format!("NVIDIA: {driver}"));
    }
    if graphics.is_empty() {
        graphics.push("Mesa".into());
    }

    let swap = if cfg.swap_gb == 0 { "none".into() } else { format!("{} GB", cfg.swap_gb) };
    for (label, value) in [
        ("Mirrors", cfg.mirror_country.clone()),
        ("Swap", swap),
        ("Timezone", cfg.timezone.clone()),
        ("Locale", cfg.locale.clone()),
        ("User", format!("{}@{}", cfg.username, cfg.hostname)),
        ("Boot / admin", format!("{} / {}", short(&BOOTLOADERS, cfg.bootloader), short(&PRIVILEGE, cfg.super_user_utility))),
        ("Graphics", graphics.join(", ")),
        ("Desktop", short(&DESKTOPS, cfg.desktop).into()),
        ("Browser", short(&BROWSERS, cfg.browser).into()),
    ] {
        lines.push(row(label, Span::raw(value)));
    }

    let aur = cfg.packages().aur;
    if !aur.is_empty() {
        lines.push(row("Part 2 (AUR)", Span::styled(aur.join(" "), Style::new().fg(Color::Yellow))));
    }
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}
