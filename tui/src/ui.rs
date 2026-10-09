use installer::{
    config::{Config, ConnectionType, GpuDriver, IpMethod, Listen},
    storage::{self, Part},
    system::{BlockDevice, GpuVendor, System},
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{block::Title, Block, Borders, HighlightSpacing, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::{
    app::*,
    term::{draw_screen, Install, Status},
};

// 16 colours and plain characters: the Linux console's font has box drawing, not much else.
const BORDER: Color = Color::Cyan;
const ACCENT: Color = Color::Cyan;
const TITLE: Color = Color::Yellow;
const DIM: Color = Color::DarkGray;
const KEY_WORD: Color = Color::LightRed;
const SELECTED: Style = Style::new().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD);
const VERSION: &str = env!("CARGO_PKG_VERSION");
const SPINNER: [&str; 4] = ["|", "/", "-", "\\"];

fn dim(text: impl Into<String>) -> Span<'static> {
    Span::styled(text.into(), Style::new().fg(DIM))
}

fn boxed() -> Block<'static> {
    Block::default().borders(Borders::ALL).border_style(Style::new().fg(BORDER))
}

fn titled(title: String) -> Block<'static> {
    boxed().title(Span::styled(title, Style::new().fg(ACCENT).bold()))
}

pub fn draw(f: &mut Frame, app: &mut App) {
    let status = status(app);
    let status_height = if status.is_some() { 3 } else { 0 };
    let rows = split(Direction::Vertical, f.size(), [Constraint::Length(3), Constraint::Min(0), Constraint::Length(status_height), Constraint::Length(3)]);
    header(f, rows[0], app);

    let main = if rows[1].width >= 72 && !(app.preview && app.install.is_none()) {
        let cols = split(Direction::Horizontal, rows[1], [Constraint::Length(32), Constraint::Min(0)]);
        sidebar(f, cols[0], app);
        cols[1]
    } else {
        rows[1]
    };
    if app.install.is_some() {
        install_panel(f, main, app);
    } else {
        panel(f, main, app);
    }

    if let Some((label, text, color)) = status {
        let line = Line::from(vec![Span::styled(format!("[{label}]"), Style::new().fg(color).bold()), Span::raw(format!(": {text}"))]);
        f.render_widget(Paragraph::new(line).block(Block::default().borders(Borders::ALL).border_style(Style::new().fg(color))), rows[2]);
    }
    footer(f, rows[3], app);
}

fn split<const N: usize>(direction: Direction, area: Rect, constraints: [Constraint; N]) -> std::rc::Rc<[Rect]> {
    Layout::default().direction(direction).constraints(constraints).split(area)
}

/// "[=====>----]"
fn bar(width: usize, done: f64, fill: char, empty: char) -> String {
    let done = done.clamp(0.0, 1.0);
    let filled = (done * width as f64).round() as usize;
    let inner = if filled >= width {
        fill.to_string().repeat(width)
    } else if filled == 0 || fill != '=' {
        format!("{}{}", fill.to_string().repeat(filled), empty.to_string().repeat(width - filled))
    } else {
        format!("{}>{}", "=".repeat(filled - 1), empty.to_string().repeat(width - filled))
    };
    format!("[{inner}]")
}

/// How far the install has got, 0 to 1, from its step count.
fn install_done(install: &Install) -> f64 {
    let (step, total, _) = install.progress();
    match install.status {
        Status::Finished(true) => 1.0,
        _ if total == 0 => 0.0,
        _ => step.saturating_sub(1) as f64 / total as f64,
    }
}

/// The install's stage in INSTALL_STAGES.
fn current_install_stage(install: &Install) -> usize {
    match install.status {
        Status::Finished(true) => INSTALL_STAGES.len(),
        _ => install_stage(&install.progress().2),
    }
}

/// (done 0..1, "Step 3/7")
fn progress(app: &App) -> (f64, String) {
    let stages = STAGES.len();
    match &app.install {
        Some(install) => (install_done(install), format!("Step {}/{stages}", (current_install_stage(install) + 1).min(stages))),
        None if app.preview => (1.0, "Preview".into()),
        None => {
            let stage = stage_of(app.step);
            (stage as f64 / stages as f64, format!("Step {}/{stages}", stage + 1))
        }
    }
}

fn header(f: &mut Frame, area: Rect, app: &App) {
    let (done, label) = progress(app);
    let block = boxed();
    let inner = pad(block.inner(area));
    f.render_widget(block, area);
    let width = inner.width as usize;

    let left = |full: bool, bar_width: usize| {
        Line::from(vec![
            Span::styled(if full { "2Lazy4Arch: Install Arch Fast" } else { "2Lazy4Arch" }, Style::new().fg(TITLE).bold()),
            dim(if full { format!("   v{VERSION}   ") } else { "  ".into() }),
            "Progress: ".into(),
            Span::styled(bar(bar_width, done, '=', '-'), Style::new().fg(Color::Green)),
            Span::styled(format!(" {:.0}%", done * 100.0), Style::new().bold()),
            dim(format!(" ({label})")),
        ])
    };
    let (firmware, color) = if app.sys.uefi { ("Active", Color::Green) } else { ("BIOS!", Color::Red) };
    let right = |arch: bool| {
        let mut spans = vec![];
        if arch {
            spans.extend([Span::raw(format!("Arch Linux ({})", std::env::consts::ARCH)), dim(" | ")]);
        }
        spans.extend([
            "UEFI: ".into(),
            Span::styled(firmware, Style::new().fg(color).bold()),
            dim(" | "),
            Span::raw("Kernel: "),
            Span::styled(app.sys.kernel.split('-').next().unwrap_or("?").to_string(), Style::new().fg(ACCENT)),
        ]);
        Line::from(spans)
    };
    // most to least complete, the first that fits
    let layouts = [(left(true, 20), Some(right(true))), (left(true, 14), Some(right(false))), (left(false, 14), Some(right(false))), (left(false, 12), None)];
    let (left, right) = layouts.into_iter().find(|(l, r)| l.width() + r.as_ref().map_or(0, |r| r.width() + 3) <= width).unwrap_or((left(false, 8), None));
    if let Some(right) = right {
        f.render_widget(Paragraph::new(right).alignment(Alignment::Right), inner);
    }
    f.render_widget(Paragraph::new(left), inner);
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

fn step_label(step: Step) -> &'static str {
    match step {
        Step::Welcome => "Welcome",
        Step::Partition => "Disk",
        Step::Boot | Step::FormatBoot => "EFI partition",
        Step::Root => "Root partition",
        Step::Home | Step::FormatHome => "Home partition",
        Step::ExtraMounts => "Other mounts",
        Step::Swap => "Swap",
        Step::Mirrors => "Mirrors",
        Step::Timezone => "Timezone",
        Step::Locale => "Locale",
        Step::Accounts => "Users",
        Step::Shell => "Shell",
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
        Step::Review => "Summary",
    }
}

fn device_name(path: Option<&String>) -> String {
    path.map_or("none".into(), |p| p.trim_start_matches("/dev/").to_string())
}

/// One stage row: "01. Partitioning      [OK]"
fn stage_row(index: usize, name: &str, width: usize, current: bool, cursor: bool, badge: &str) -> Line<'static> {
    let label = format!("{}{:02}. {name}", if current { ">> " } else { "" }, index + 1);
    let pad = width.saturating_sub(label.len() + badge.len());
    let (label_style, badge_style) = if cursor {
        (Style::new().fg(Color::Black).bg(ACCENT).bold(), Style::new().fg(Color::Black).bg(ACCENT).bold())
    } else if current {
        (SELECTED, SELECTED)
    } else if badge == "[OK]" {
        (Style::new().fg(DIM).add_modifier(Modifier::CROSSED_OUT), Style::new().fg(Color::Green).bold())
    } else if badge == "[!!]" {
        (Style::new().fg(Color::Red), Style::new().fg(Color::Red).bold())
    } else {
        (Style::new().fg(Color::Gray), Style::new().fg(DIM))
    };
    Line::from(vec![Span::styled(format!("{label}{}", " ".repeat(pad)), label_style), Span::styled(badge.to_string(), badge_style)])
}

fn sidebar(f: &mut Frame, area: Rect, app: &App) {
    let block = titled("[ INSTALLER STAGES ]".into());
    let inner = pad(block.inner(area));
    f.render_widget(block, area);
    let width = inner.width as usize;

    let mut lines = vec![Line::default()];
    let foot: [Line; 2] = match &app.install {
        Some(install) => {
            let current = current_install_stage(install);
            let spinner = SPINNER[(install.started.elapsed().as_millis() / 150) as usize % 4];
            for (i, (name, _)) in INSTALL_STAGES.iter().enumerate() {
                let badge = match (i.cmp(&current), install.status) {
                    (std::cmp::Ordering::Less, _) => "[OK]".to_string(),
                    (std::cmp::Ordering::Equal, Status::Running) => format!("[{spinner}]"),
                    (std::cmp::Ordering::Equal, _) => "[!!]".into(),
                    _ => "[ ]".into(),
                };
                lines.push(stage_row(i, name, width, i == current, false, &badge));
            }
            let (text, color) = match install.status {
                Status::Running if install.paused => ("PAUSED", Color::Yellow),
                Status::Running => ("RUNNING", Color::Green),
                Status::Finished(true) => ("DONE", Color::Green),
                Status::Finished(false) => ("FAILED", Color::Red),
            };
            [
                Line::from(vec![dim("Execution: "), Span::styled(text, Style::new().fg(color).bold())]),
                Line::from(vec![Span::styled("[P]", Style::new().bold()), dim(if install.paused { " resume stream" } else { " pause stream" })]),
            ]
        }
        None => {
            let current = if app.preview { STAGES.len() } else { stage_of(app.step) };
            for (i, (name, steps)) in STAGES.iter().enumerate() {
                let done = app.preview || steps.iter().all(|s| app.skipped(*s) || app.done.contains(s));
                let badge = if done && i != current { "[OK]" } else { "[ ]" };
                lines.push(stage_row(i, name, width, i == current, app.stages_focused && i == app.stage_cursor, badge));
            }
            let focus = if app.stages_focused { "Stages".to_string() } else { format!("Main [{}]", step_label(app.step)) };
            [
                Line::from(vec![dim("Pane Focus: "), Span::styled(focus, Style::new().fg(TITLE))]),
                Line::from(vec![Span::styled("[Tab]", Style::new().bold()), dim(" switch pane")]),
            ]
        }
    };
    f.render_widget(Paragraph::new(lines), inner);
    if inner.height > 10 {
        let bottom = Rect { y: inner.y + inner.height - 2, height: 2, ..inner };
        f.render_widget(Paragraph::new(foot.to_vec()), bottom);
    }
}

fn title(app: &App) -> String {
    match (&app.sub, app.step) {
        (Some(Sub::Disk(disk)), _) => format!("What to do with {disk}"),
        (Some(Sub::Mount(_)), _) => "Mount point".into(),
        (Some(Sub::NewUser { admin: true }), _) => "New admin user".into(),
        (Some(Sub::NewUser { .. }), _) => "New user".into(),
        (Some(Sub::Extra(extra)), _) => EXTRAS.iter().find(|(e, _)| e == extra).map_or("", |(_, l)| *l).into(),
        (None, Step::Welcome) => "Welcome to 2Lazy4Arch".into(),
        (None, Step::Partition) => "Select a disk to partition".into(),
        (None, Step::FormatBoot) => "Format the EFI partition?".into(),
        (None, Step::FormatHome) => "Format the home partition?".into(),
        (None, Step::Mirrors) => "Mirror country".into(),
        (None, Step::Swap) => "Swap file".into(),
        (None, Step::Shell) => "Your shell".into(),
        (None, Step::Wifi) => "Wi-Fi for the new system".into(),
        (None, Step::Review) if app.preview => "Preview".into(),
        (None, Step::Review) => "Summary: ready to install".into(),
        (None, s) => step_label(s).into(),
    }
}

fn help(app: &App) -> String {
    let fs = &app.filesystem;
    match (&app.sub, app.step) {
        (Some(Sub::Disk(_)), _) => "Erase and free space make a 1 GiB EFI partition and a root partition. cfdisk lets you do it yourself; pick the partitions next.".into(),
        (Some(Sub::Mount(partition)), _) => format!("Where should {} be mounted? It's mounted as-is, never formatted.", device_name(Some(partition))),
        (Some(Sub::NewUser { .. }), _) => "Admins are in the wheel group and can use sudo.".into(),
        (Some(Sub::Extra(Extra::Ssh)), _) => "OpenSSH, on at boot. With a GitHub user, its public keys go into your authorized_keys and you log in with them instead of a password.".into(),
        (Some(Sub::Extra(Extra::Tailscale)), _) => "With an auth key the machine joins your tailnet on first boot, then the key is deleted. Without one, run sudo tailscale up yourself.".into(),
        (Some(Sub::Extra(_)), _) => "Classic VNC only uses the first 8 characters. VNC isn't encrypted, so keep it on a trusted network.".into(),
        (None, Step::Welcome) => "This is what was found. Nothing is written to disk until you confirm the summary at the end.".into(),
        (None, Step::Partition) => "Pick a disk to erase, use its free space or edit it in cfdisk. Continue if your partitions are ready: you pick the EFI and root partitions next.".into(),
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

/// The coloured bar above the footer: errors first, then what the screen is about to do.
fn status(app: &App) -> Option<(&'static str, String, Color)> {
    if let Some(install) = &app.install {
        return Some(match install.status {
            Status::Running if install.abort_armed => ("ABORT?", "Press q again to stop the install. The disks may be left half-installed.".into(), Color::Red),
            Status::Running if install.paused => ("PAUSED", "Output is paused; the install keeps running. P resumes.".into(), Color::Yellow),
            Status::Running => ("ACTIVE", "Changes are currently being written to disk. Do not power off the system or remove the boot media.".into(), Color::Red),
            Status::Finished(true) => ("DONE", "Installation finished. q to leave.".into(), Color::Green),
            Status::Finished(false) => ("FAILED", "The install stopped; scroll up for why. The log is at /var/log/2lazy4arch.log. q to leave.".into(), Color::Red),
        });
    }
    if let Some(e) = &app.error {
        return Some(("ERROR", e.clone(), Color::Red));
    }
    match app.step {
        Step::Welcome if !app.sys.uefi => Some(("WARN", "This machine booted in BIOS mode. Boot the ISO in UEFI mode to install.".into(), Color::Red)),
        Step::Partition if app.sub.is_none() => Some(("WARN", "All partitioning changes made with cfdisk are immediate and permanent.".into(), Color::Red)),
        Step::Review if !app.problems.is_empty() => Some(("BLOCKED", format!("{} problem(s) to fix before installing, listed above.", app.problems.len()), Color::Red)),
        Step::Review => Some(("WARN", "y erases everything marked ERASE or FORMAT.".into(), Color::Yellow)),
        _ => None,
    }
}

/// (key, what it does) pairs for the footer, and the right-hand quit hint.
fn hints(app: &App) -> (Vec<(&'static str, &'static str)>, &'static str) {
    if let Some(install) = &app.install {
        return match install.status {
            Status::Running => (
                vec![("[P]", if install.paused { "resume stream" } else { "pause stream" }), ("[S]", "auto-scroll"), ("[C]", "clear screen"), ("(pgup/pgdn)", "scroll")],
                "(q) abort install",
            ),
            Status::Finished(_) => (vec![("(pgup/pgdn)", "scroll")], "(q) leave"),
        };
    }
    let quit = if app.step == Step::Welcome || app.preview { "(esc) quit" } else { "(ctrl+c) quit" };
    let keys = if app.stages_focused {
        vec![("(up/down)", "move"), ("(enter)", "jump to stage"), ("[tab]", "back")]
    } else if app.is_form() {
        vec![("(tab)", "next field"), ("(enter)", "confirm"), ("(esc)", "back")]
    } else if app.step == Step::Review {
        let mut keys = vec![("(pgup/pgdn)", "scroll"), ("(y)", "install")];
        if !app.preview {
            keys.extend([("[tab]", "switch pane"), ("(esc)", "back")]);
        }
        keys
    } else if app.filtering {
        vec![("type", "to search"), ("(enter)", "select"), ("(esc)", "clear")]
    } else {
        vec![("[/]", "search"), ("[tab]", "switch pane"), ("(enter)", "select"), ("(esc)", "back")]
    };
    (keys, quit)
}

fn footer(f: &mut Frame, area: Rect, app: &App) {
    let cols = split(Direction::Horizontal, area, [Constraint::Length(36), Constraint::Min(0)]);
    let (stage, step) = match &app.install {
        Some(install) => {
            let stage = current_install_stage(install).min(INSTALL_STAGES.len() - 1);
            (INSTALL_STAGES[stage].0.to_string(), install.progress().2)
        }
        None if app.preview => ("Config file".into(), "Preview".into()),
        None => (STAGES[stage_of(app.step)].0.to_string(), step_label(app.step).to_string()),
    };
    let crumbs = Line::from(vec![Span::styled(stage, Style::new().fg(TITLE).bold()), dim(" | "), Span::raw(step)]);
    f.render_widget(Paragraph::new(crumbs).block(boxed()), cols[0]);

    let block = boxed();
    let inner = block.inner(cols[1]);
    f.render_widget(block, cols[1]);
    let (keys, quit) = hints(app);
    let mut spans = vec![];
    for (i, (key, what)) in keys.iter().enumerate() {
        if i > 0 {
            spans.push(dim(" / "));
        }
        spans.push(Span::styled(*key, Style::new().bold()));
        spans.push(Span::styled(format!(" {what}"), Style::new().fg(KEY_WORD)));
    }
    let line = Line::from(spans);
    if line.width() + quit.len() + 2 <= inner.width as usize {
        f.render_widget(Paragraph::new(Span::styled(quit, Style::new().fg(DIM))).alignment(Alignment::Right), inner);
    }
    f.render_widget(Paragraph::new(line), inner);
}

fn panel(f: &mut Frame, area: Rect, app: &mut App) {
    let block = titled(format!("[ {} ]", title(app).to_uppercase()));
    let inner = pad(block.inner(area));
    f.render_widget(block, area);

    let help = help(app);
    let width = inner.width.max(1);
    let help_height = help.chars().count() as u16 / width + 2;
    let rows = split(Direction::Vertical, inner, [Constraint::Length(help_height), Constraint::Min(0)]);
    f.render_widget(Paragraph::new(help).wrap(Wrap { trim: true }).fg(Color::Gray), rows[0]);

    let content = rows[1];
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
    } else if app.step == Step::Welcome {
        let rows = split(Direction::Vertical, content, [Constraint::Min(0), Constraint::Length(2)]);
        f.render_widget(Paragraph::new(hardware_lines(&app.sys)), rows[0]);
        list(f, rows[1], app);
    } else if app.step == Step::Partition && app.sub.is_none() && content.height > 8 {
        let rows = split(Direction::Vertical, content, [Constraint::Min(0), Constraint::Length(3)]);
        list(f, rows[0], app);
        disk_details(f, rows[1], app);
    } else {
        list(f, content, app);
    }
}

/// The Welcome screen: what was found on this machine.
fn hardware_lines(sys: &System) -> Vec<Line<'static>> {
    let ok = |good: bool| if good { Span::styled("  [OK]", Style::new().fg(Color::Green).bold()) } else { Span::styled("  [!!]", Style::new().fg(Color::Red).bold()) };
    let row = |label: &str, value: String| vec![dim(format!("{label:<11}")), Span::styled(value, Style::new().bold())];
    let mut lines = vec![];
    let mut system = row("System", format!("Arch Linux ISO ({}), kernel {}", std::env::consts::ARCH, sys.kernel));
    system.push(Span::raw(""));
    lines.push(Line::from(system));
    let mut firmware = row("Firmware", if sys.uefi { "UEFI".into() } else { "BIOS (not supported)".into() });
    firmware.push(ok(sys.uefi));
    lines.push(Line::from(firmware));
    let cpu = match sys.cpu.as_deref() {
        Some("amd") => "AMD, gets amd-ucode",
        Some("intel") => "Intel, gets intel-ucode",
        _ => "unknown, no microcode",
    };
    lines.push(Line::from(row("CPU", cpu.into())));
    lines.push(Line::from(row("Memory", format!("{:.1} GB", sys.memory_kib as f64 / 1_048_576.0))));
    lines.push(Line::from(row("Graphics", gpu_names(sys))));
    let disks: Vec<String> = sys.disks().iter().map(|d| format!("{} {}", d.name(), gb(d.size))).collect();
    lines.push(Line::from(row("Storage", if disks.is_empty() { "no disks found".into() } else { disks.join(", ") })));
    let mut network = row("Network", if sys.links_up.is_empty() { "no link up (iwctl for wifi)".into() } else { format!("link up on {}", sys.links_up.join(", ")) });
    network.push(ok(!sys.links_up.is_empty()));
    lines.push(Line::from(network));
    lines
}

/// Disk sizes the way vendors print them: "512.1 GB"
fn gb(bytes: Option<u64>) -> String {
    format!("{:.1} GB", bytes.unwrap_or(0) as f64 / 1e9)
}

fn disk_tag(disk: &BlockDevice) -> String {
    let kind = if disk.rota { "HDD" } else { "SSD" };
    match disk.tran.as_deref() {
        Some("nvme") => "NVMe SSD".into(),
        Some("sata") | Some("ata") => format!("SATA {kind}"),
        Some("usb") => "USB".into(),
        Some(other) => other.to_uppercase(),
        None if disk.name().starts_with("vd") => "virtio".into(),
        None => "disk".into(),
    }
}

fn table_type(disk: &BlockDevice) -> (&'static str, Color) {
    match disk.pttype.as_deref() {
        Some("gpt") => ("GPT", Color::Green),
        Some("dos") => ("MBR", Color::Yellow),
        Some(_) => ("other", Color::Yellow),
        None => ("empty", DIM),
    }
}

/// ">> sda [SATA SSD]      Size: 512.1 GB   Model: Samsung SSD 870   Type: GPT"
fn disk_line(disk: &BlockDevice, width: usize) -> Line<'static> {
    let (table, color) = table_type(disk);
    let model = disk.model.as_deref().unwrap_or("").trim().to_string();
    let right = vec![
        dim("Size: "),
        Span::styled(format!("{:<10}", gb(disk.size)), Style::new().bold()),
        dim("  Model: "),
        Span::raw(format!("{model:<18}")),
        dim("  Type: "),
        Span::styled(table, Style::new().fg(color).bold()),
    ];
    let left = vec![Span::styled(format!("{:<9}", disk.name()), Style::new().bold()), Span::styled(format!("[{}]", disk_tag(disk)), Style::new().fg(Color::Gray).bg(DIM))];
    let used: usize = left.iter().chain(right.iter()).map(|s| s.width()).sum();
    let mut spans = left;
    spans.push(Span::raw(" ".repeat(width.saturating_sub(used + 1).max(2))));
    spans.extend(right);
    Line::from(spans)
}

/// Under the disk list: about the highlighted disk.
fn disk_details(f: &mut Frame, area: Rect, app: &App) {
    let disks = app.sys.disks();
    let Some(disk) = app.list.selected().and_then(|i| app.visible().get(i).copied()).and_then(|i| disks.get(i)) else { return };
    let parts = app.sys.partitions().into_iter().filter(|p| p.pkname.as_deref() == Some(disk.name())).count();
    let first = Line::from(vec![Span::styled("Target Device Details: ", Style::new().bold()), Span::styled(disk.path.clone(), Style::new().fg(ACCENT).bold())]);
    let sectors = format!("{} B / {} B", disk.log_sec.unwrap_or(512), disk.phy_sec.unwrap_or(512));
    let second = Line::from(vec![
        dim("Path: "),
        Span::raw(format!("{}   ", disk.path)),
        dim("Sector Size: "),
        Span::raw(format!("{sectors}   ")),
        dim("Partitions: "),
        Span::raw(format!("{parts} existing   ")),
        dim("Read-Only: "),
        if disk.ro { Span::styled("Yes", Style::new().fg(Color::Red)) } else { Span::styled("No", Style::new().fg(Color::Green)) },
    ]);
    let block = Block::default().borders(Borders::TOP).border_style(Style::new().fg(BORDER));
    let inner = block.inner(area);
    f.render_widget(block, area);
    if disk.pttype.is_some() {
        let warning = Span::styled("[!] Selected drive contains existing tables", Style::new().fg(Color::Yellow));
        if first.width() + warning.width() + 2 <= inner.width as usize {
            f.render_widget(Paragraph::new(warning).alignment(Alignment::Right), Rect { height: 1, ..inner });
        }
    }
    f.render_widget(Paragraph::new(vec![first, second]), inner);
}

/// One column of breathing room on each side.
fn pad(area: Rect) -> Rect {
    Rect { x: area.x + 1, width: area.width.saturating_sub(2), ..area }
}

fn list(f: &mut Frame, area: Rect, app: &mut App) {
    let options = app.options();
    let visible = app.visible();
    let what = if app.step == Step::Partition && app.sub.is_none() { "devices" } else { "items" };
    let area = if app.step != Step::Welcome && area.height >= 6 {
        // search box, boxed when there's room
        let tall = area.height >= 12;
        let rows = split(Direction::Vertical, area, [Constraint::Length(if tall { 3 } else { 1 }), Constraint::Min(0)]);
        let cols = split(Direction::Horizontal, rows[0], [Constraint::Min(0), Constraint::Length(24)]);
        let filter = Line::from(vec![
            Span::styled("[/] ", Style::new().fg(TITLE).bold()),
            dim("Filter: "),
            Span::styled(format!("{}{}", app.filter, if app.filtering { "_" } else { "" }), Style::new().bold()),
            dim(if app.filtering { "  (esc to clear)" } else if app.filter.is_empty() { "  (/ to search)" } else { "" }),
        ]);
        let input = if tall { Paragraph::new(filter).block(Block::default().borders(Borders::ALL).border_style(Style::new().fg(if app.filtering { TITLE } else { DIM }))) } else { Paragraph::new(filter) };
        f.render_widget(input, cols[0]);
        let count = Rect { y: cols[1].y + cols[1].height / 2, height: 1, ..cols[1] };
        f.render_widget(Paragraph::new(dim(format!("Showing {} of {} {what}", visible.len(), options.len()))).alignment(Alignment::Right), count);
        rows[1]
    } else {
        area
    };

    let disks = app.sys.disks();
    let width = area.width.saturating_sub(3) as usize;
    let items: Vec<ListItem> = visible
        .iter()
        .map(|&i| {
            if app.step == Step::Partition && app.sub.is_none() {
                if let Some(disk) = disks.get(i) {
                    return ListItem::new(disk_line(disk, width));
                }
                let style = if options[i].starts_with("->") { Style::new().fg(ACCENT).bold() } else { Style::new().fg(Color::Gray) };
                return ListItem::new(Span::styled(options[i].clone(), style));
            }
            if options[i].starts_with("->") {
                return ListItem::new(Span::styled(options[i].clone(), Style::new().fg(ACCENT).bold()));
            }
            ListItem::new(options[i].clone())
        })
        .collect();
    if items.is_empty() {
        f.render_widget(Paragraph::new("No matches. Backspace to edit the filter.").fg(DIM), area);
        return;
    }
    let list = List::new(items).highlight_style(SELECTED).highlight_symbol(">> ").highlight_spacing(HighlightSpacing::Always);
    f.render_stateful_widget(list, area, &mut app.list);
}

fn form(f: &mut Frame, area: Rect, app: &App) {
    // a blank line between fields when there's room, and the focused field kept in view when not
    let spaced = area.height as usize >= app.form.len() * 2;
    let scroll = if spaced { 0 } else { (app.focus + 1).saturating_sub(area.height as usize) as u16 };
    let lines: Vec<Line> = app
        .form
        .iter()
        .enumerate()
        .flat_map(|(i, field)| {
            let focused = i == app.focus;
            let value = if field.secret { "*".repeat(field.value.chars().count()) } else { field.value.clone() };
            let label_style = if focused { Style::new().fg(ACCENT).bold() } else { Style::new() };
            [
                Line::from(vec![
                    Span::styled(format!("{}{:<26}", if focused { ">> " } else { "   " }, field.label), label_style),
                    Span::raw(value),
                    if focused { Span::styled("_", Style::new().fg(ACCENT).bold()) } else { "".into() },
                ]),
                Line::default(),
            ]
            .into_iter()
            .take(if spaced { 2 } else { 1 })
        })
        .collect();
    f.render_widget(Paragraph::new(lines).scroll((scroll, 0)), area);
}

fn clock(seconds: u64) -> String {
    format!("{:02}:{:02}:{:02}", seconds / 3600, seconds / 60 % 60, seconds % 60)
}

/// "/dev/sda (Samsung SSD 870 512.1 GB, GPT)" for the disk holding root.
fn target_disk(app: &App, plan: &storage::Plan) -> Option<String> {
    let root = plan.mounts.iter().find(|(m, _)| m.is_empty())?;
    let disk_path = match &root.1 {
        Part::New { disk, .. } => disk.clone(),
        Part::Existing { path, .. } => {
            let part = app.sys.devices.iter().find(|d| &d.path == path)?;
            format!("/dev/{}", part.pkname.as_deref()?)
        }
    };
    let disk = app.sys.disks().into_iter().find(|d| d.path == disk_path)?;
    let model = disk.model.as_deref().map(str::trim).filter(|m| !m.is_empty()).map(|m| format!("{m} ")).unwrap_or_default();
    Some(format!("{disk_path} ({model}{}, {})", gb(disk.size), table_type(disk).0))
}

/// The install: what it's doing, how far along, its terminal, and where it's writing.
fn install_panel(f: &mut Frame, area: Rect, app: &mut App) {
    let plan = storage::plan(&app.cfg.storage, &app.sys).ok();
    let target = plan.as_ref().and_then(|p| target_disk(app, p)).unwrap_or_default();
    let mounts: Vec<(String, String)> = plan
        .as_ref()
        .map(|p| {
            p.mounts
                .iter()
                .map(|(mount, part)| {
                    let what = match part {
                        Part::Existing { path, .. } => path.clone(),
                        Part::New { name, .. } => format!("PARTLABEL={name}"),
                    };
                    (show_mount(mount), what)
                })
                .collect()
        })
        .unwrap_or_default();
    let swap = app.cfg.storage.swap.0.map_or("none".into(), |gb| format!("{gb} GB file"));
    let Some(install) = &mut app.install else { return };

    let (status, color) = match install.status {
        Status::Running if install.paused => ("PAUSED", Color::Yellow),
        Status::Running => ("RUNNING", Color::Green),
        Status::Finished(true) => ("DONE", Color::Green),
        Status::Finished(false) => ("FAILED", Color::Red),
    };
    // the long title and both badges, dropping what doesn't fit
    let status_badge = Span::styled(format!("[STATUS: {status}]"), Style::new().fg(color).bold());
    let scroll_badge = Span::styled(format!("[AUTO-SCROLL: {}]", if install.autoscroll { "ON" } else { "OFF" }), Style::new().fg(ACCENT));
    let long_title = "+-[ TERMINAL EXECUTION: ARCH LINUX INSTALLATION ]-+";
    let room = area.width as usize - 4;
    let title = if long_title.len() + status_badge.width() + 2 <= room { long_title } else { "+-[ INSTALLING ]-+" };
    let mut badges = vec![status_badge];
    if title.len() + Line::from(badges.clone()).width() + scroll_badge.width() + 5 <= room {
        badges.extend([dim(" | "), scroll_badge]);
    }
    let block = boxed().title(Span::styled(title, Style::new().fg(TITLE).bold())).title(Title::from(Line::from(badges)).alignment(Alignment::Right));
    let inner = pad(block.inner(area));
    f.render_widget(block, area);
    let roomy = inner.height >= 18;
    let rows = split(
        Direction::Vertical,
        inner,
        [Constraint::Length(if roomy { 5 } else { 3 }), Constraint::Min(3), Constraint::Length(if roomy { 2 } else { 1 })],
    );

    // the info box
    let (step, total, task) = install.progress();
    let done = install_done(install);
    let stage = (current_install_stage(install) + 1).min(INSTALL_STAGES.len());
    let info_area = if roomy {
        let b = boxed();
        let inner = pad(b.inner(rows[0]));
        f.render_widget(b, rows[0]);
        inner
    } else {
        rows[0]
    };
    let lines = split(Direction::Vertical, info_area, [Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)]);
    let task_line = Line::from(vec![Span::styled("Current Task: ", Style::new().fg(TITLE).bold()), Span::styled(task, Style::new().bold())]);
    let root = mounts.iter().find(|(m, _)| m == "/").map(|(_, d)| d.clone()).unwrap_or_default();
    let root_line = Line::from(vec![Span::raw("Target Root: "), Span::styled(root, Style::new().fg(ACCENT)), dim(" on /mnt")]);
    if task_line.width() + root_line.width() + 2 <= lines[0].width as usize {
        f.render_widget(Paragraph::new(root_line).alignment(Alignment::Right), lines[0]);
    }
    f.render_widget(Paragraph::new(task_line), lines[0]);

    // both bars share what's left after their labels (about 62 columns)
    let half = (lines[1].width as usize).saturating_sub(62).div_ceil(2).clamp(6, 40);
    let mut progress = vec![
        Span::raw("Overall: "),
        Span::styled(bar(half, done, '=', '-'), Style::new().fg(Color::Green)),
        Span::styled(format!(" {:.0}%", done * 100.0), Style::new().bold()),
        dim(format!(" (Step {stage}/{}) ", INSTALL_STAGES.len())),
    ];
    if let Some((n, m)) = install.subtask().filter(|(n, m)| n <= m && *m > 0) {
        let sub = n as f64 / m as f64;
        let sub_line = vec![
            Span::raw("   Sub-task: "),
            Span::styled(bar(half, sub, '#', '.'), Style::new().fg(ACCENT)),
            Span::styled(format!(" {:.0}%", sub * 100.0), Style::new().bold()),
            dim(format!(" ({n}/{m} pkgs)")),
        ];
        if Line::from(progress.clone()).width() + Line::from(sub_line.clone()).width() <= lines[1].width as usize {
            progress.extend(sub_line);
        }
    }
    f.render_widget(Paragraph::new(Line::from(progress)), lines[1]);

    let field = |label: &str, value: String, color: Color| vec![dim(format!("{label}: ")), Span::styled(value, Style::new().fg(color)), dim(" | ")];
    let mut stats = vec![];
    if let Some(speed) = install.speed() {
        stats.extend(field("Speed", speed, Color::Green));
    }
    stats.extend(field("Elapsed", clock(install.elapsed()), Color::Green));
    stats.extend(field("Steps", format!("{step}/{total}"), Color::White));
    if let Some(pid) = install.pid {
        stats.extend(field("Worker PID", pid.to_string(), Color::White));
    }
    stats.extend([dim("Log: "), Span::raw("/var/log/2lazy4arch.log")]);
    f.render_widget(Paragraph::new(Line::from(stats)), lines[2]);

    // the terminal
    let frame = Block::default().borders(Borders::ALL).border_style(Style::new().fg(DIM));
    let screen_area = frame.inner(rows[1]);
    f.render_widget(frame, rows[1]);
    install.resize(screen_area.height, screen_area.width);
    draw_screen(install.parser.screen(), screen_area, f.buffer_mut());

    // where it's writing
    let lock = if install.status == Status::Running { Span::styled("[WRITE-LOCK ACTIVE]", Style::new().fg(Color::Green).bold()) } else { dim("[RELEASED]") };
    let target_line = Line::from(vec![Span::styled("Execution Target: ", Style::new().fg(TITLE).bold()), Span::styled(target, Style::new().fg(ACCENT))]);
    let mut mount_spans: Vec<Span> = mounts.iter().flat_map(|(m, d)| [dim(format!("{m}: ")), Span::raw(format!("{d}   "))]).collect();
    mount_spans.extend([dim("Swap: "), Span::raw(format!("{swap}   ")), dim("Chroot Path: "), Span::raw("/mnt")]);
    if roomy {
        let parts = split(Direction::Vertical, rows[2], [Constraint::Length(1), Constraint::Length(1)]);
        if target_line.width() + lock.width() + 2 <= parts[0].width as usize {
            f.render_widget(Paragraph::new(lock).alignment(Alignment::Right), parts[0]);
        }
        f.render_widget(Paragraph::new(target_line), parts[0]);
        f.render_widget(Paragraph::new(Line::from(mount_spans)), parts[1]);
    } else {
        f.render_widget(Paragraph::new(target_line), rows[2]);
    }
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
