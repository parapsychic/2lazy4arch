mod app;
mod ui;

use std::{
    fs::OpenOptions,
    io::{self, Write},
    process::{Command, ExitCode},
    thread,
    time::Duration,
};

use app::{Action, App};
use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use installer::{
    config::{Config, Desktop, Finish},
    post_install::PostInstall,
    source::{self, Source},
    system::System,
    utils::{tee_output, LOG_FILE},
    validate, Install, SKIPPED_FILE,
};
use nix::unistd::Uid;
use ratatui::{backend::CrosstermBackend, Terminal};
use serde_json::Value;
use shell_iface::logger::Logger;

type Term = Terminal<CrosstermBackend<io::Stdout>>;

const HELP: &str = "2lazy4arch installs Arch Linux.

  2lazy4arch                            step-by-step installer
  2lazy4arch --config-file <path|url>   install from a config file (see unattended-config.yaml)
      --no-confirm                      skip the preview, install straight away
      --no-validate                     skip the schema and system checks

Run it as root from the Arch ISO, booted in UEFI mode.

On an installed system, as a normal user:
  2lazy4arch --user-setup [--rice] [AUR_PACKAGE...]
                                        AUR packages with yay, and/or the ParaPsychic rice";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |f: &str| args.iter().any(|a| a == f);
    if flag("--help") || flag("-h") {
        println!("{HELP}");
        return ExitCode::SUCCESS;
    }
    if flag("--user-setup") {
        return user_setup(&args);
    }

    let config = args.iter().position(|a| a == "--config-file").map(|i| args.get(i + 1));
    let known = |i: usize, a: &String| ["--config-file", "--no-confirm", "--no-validate"].contains(&a.as_str()) || (i > 0 && args[i - 1] == "--config-file");
    if let Some((_, unknown)) = args.iter().enumerate().find(|(i, a)| !known(*i, a)) {
        eprintln!("Unknown argument {unknown}, see --help.");
        return ExitCode::FAILURE;
    }
    if !Uid::effective().is_root() {
        eprintln!("Run 2lazy4arch as root, from the Arch ISO.");
        return ExitCode::FAILURE;
    }
    match config {
        Some(Some(location)) => declarative(location, !flag("--no-confirm"), !flag("--no-validate")),
        Some(None) => {
            eprintln!("--config-file needs a path or a URL.");
            ExitCode::FAILURE
        }
        None if flag("--no-confirm") || flag("--no-validate") => {
            eprintln!("--no-confirm and --no-validate go with --config-file.");
            ExitCode::FAILURE
        }
        None => wizard(),
    }
}

fn wizard() -> ExitCode {
    let logger = Logger::new(false);
    let mut app = App::new(&logger);
    match run_tui(&mut app) {
        Ok(true) => {}
        Ok(false) => return ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Terminal error: {e}");
            return ExitCode::FAILURE;
        }
    }
    let value = serde_json::to_value(&app.cfg).unwrap_or_default();
    run_install(&app.cfg, &app.sys, &Source::Dir(".".into()), &logger, value, true)
}

fn bullets(lines: &[String]) -> String {
    lines.iter().map(|l| format!("  - {l}")).collect::<Vec<_>>().join("\n")
}

fn declarative(location: &str, confirm: bool, validate: bool) -> ExitCode {
    let logger = Logger::new(false);
    let fail = |msg: String| {
        eprintln!("{msg}");
        ExitCode::FAILURE
    };
    let (text, source) = match source::load(location) {
        Ok(loaded) => loaded,
        Err(e) => return fail(format!("Couldn't read the config: {e:#}")),
    };
    let value: Value = match serde_norway::from_str(&text) {
        Ok(value) => value,
        Err(e) => return fail(format!("The config isn't valid YAML: {e}")),
    };
    if validate {
        let errors = validate::schema_errors(&value);
        if !errors.is_empty() {
            return fail(format!("Configuration is invalid:\n{}", bullets(&errors)));
        }
    }
    let mut cfg: Config = match serde_json::from_value(value.clone()) {
        Ok(cfg) => cfg,
        Err(e) => return fail(format!("Configuration is invalid: {e}")),
    };
    if let Err(e) = cfg.load_lists(&source) {
        return fail(format!("Configuration is invalid: {e:#}"));
    }

    let sys = System::probe(&logger);
    if validate {
        let errors = validate::system_errors(&cfg, &sys, &source);
        if !errors.is_empty() {
            return fail(format!("Configuration would not work with this system:\n{}", bullets(&errors)));
        }
    }

    if confirm {
        let mut app = App::preview(&logger, sys.clone(), cfg.clone());
        match run_tui(&mut app) {
            Ok(true) => {}
            Ok(false) => {
                println!("Nothing was changed.");
                return ExitCode::SUCCESS;
            }
            Err(e) => return fail(format!("Terminal error: {e}")),
        }
    }
    run_install(&cfg, &sys, &source, &logger, value, confirm)
}

/// Installs, logging everything; then finishes the way the config says.
fn run_install(cfg: &Config, sys: &System, source: &Source, logger: &Logger, value: Value, interactive: bool) -> ExitCode {
    let _tee = match tee_output(LOG_FILE) {
        Ok(tee) => Some(tee),
        Err(e) => {
            eprintln!("Not logging to {LOG_FILE}: {e}");
            None
        }
    };
    println!("Installing. Everything is logged to {LOG_FILE}.");

    let confirm = |missing: &[String]| -> bool {
        println!("\n\x1b[1;33mNot found in the repos:\x1b[0m {}", missing.join(" "));
        if !interactive {
            println!("Skipping them.");
            return true;
        }
        print!("They will be skipped. Nothing has been changed yet. Continue? [y/N] ");
        let _ = io::stdout().flush();
        let mut answer = String::new();
        let _ = io::stdin().read_line(&mut answer);
        answer.trim().eq_ignore_ascii_case("y")
    };

    let job = Install { cfg, sys, source, logger, config_value: value };
    match installer::install(&job, &confirm) {
        Ok(summary) => {
            if !summary.skipped.is_empty() {
                println!("\n\x1b[1;33mSkipped, not found:\x1b[0m {}", summary.skipped.join(" "));
            }
            println!("\n\x1b[1;32mInstallation finished.\x1b[0m The log is in the new system at {LOG_FILE}.");
            if cfg.desktop == Desktop::Dwm {
                println!("dwm, dmenu and st sources are in /usr/local/src. Edit config.h, then `sudo make install`.");
            }
            if cfg.tailscale().is_some_and(|t| t.auth_key.is_none()) {
                println!("Tailscale: run `sudo tailscale up` after booting.");
            }
            installer::copy_log();
            finish(cfg.finish)
        }
        Err(e) => {
            eprintln!("\n\x1b[1;31mInstallation failed:\x1b[0m {e:#}");
            eprintln!("The log is at {LOG_FILE}.");
            ExitCode::FAILURE
        }
    }
}

fn finish(finish: Finish) -> ExitCode {
    let (verb, command) = match finish {
        Finish::Stay => {
            println!("The new system is still mounted at /mnt.");
            return ExitCode::SUCCESS;
        }
        Finish::Reboot => ("Rebooting", "reboot"),
        Finish::Poweroff => ("Powering off", "poweroff"),
    };
    for seconds in (1..=10).rev() {
        print!("\r{verb} in {seconds:>2}s, ctrl+c to stay... ");
        let _ = io::stdout().flush();
        thread::sleep(Duration::from_secs(1));
    }
    println!();
    let _ = Command::new("umount").args(["-R", "/mnt"]).status();
    let _ = Command::new("systemctl").arg(command).status();
    ExitCode::SUCCESS
}

/// As a user inside the new system (the install runs this through arch-chroot), or
/// later by hand: AUR packages with yay, and the ParaPsychic rice.
fn user_setup(args: &[String]) -> ExitCode {
    if Uid::effective().is_root() {
        eprintln!("--user-setup runs as a normal user; it uses sudo when it needs to.");
        return ExitCode::FAILURE;
    }
    let packages: Vec<&str> = args.iter().filter(|a| !a.starts_with("--")).map(String::as_str).collect();
    let logger = Logger::new(true);
    let mut post_install = PostInstall::new(&logger);
    let mut ok = true;
    let mut report = |what: &str, result: anyhow::Result<()>| {
        if let Err(e) = result {
            ok = false;
            eprintln!("\x1b[1;31m{what} failed:\x1b[0m {e:#}");
        }
    };
    report("Installing AUR packages", post_install.install_aur(&packages));
    if args.iter().any(|a| a == "--rice") {
        report("The ParaPsychic rice", post_install.misc_options());
    }
    if !post_install.skipped().is_empty() {
        let skipped = post_install.skipped().join(" ");
        println!("\x1b[1;33mSkipped, not found:\x1b[0m {skipped}");
        // the installer collects these into its summary
        if let Ok(mut file) = OpenOptions::new().append(true).create(true).open(SKIPPED_FILE) {
            let _ = writeln!(file, "{skipped}");
        }
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn setup_terminal() -> io::Result<Term> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    terminal.clear()?;
    Ok(terminal)
}

fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, crossterm::cursor::Show)
}

/// Runs the TUI; Ok(true) means install.
fn run_tui(app: &mut App) -> io::Result<bool> {
    // Don't leave the console in raw mode if we crash.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal();
        default_hook(info);
    }));
    let result = setup_terminal().and_then(|mut terminal| event_loop(&mut terminal, app));
    let _ = restore_terminal();
    let _ = std::panic::take_hook();
    result
}

fn event_loop(terminal: &mut Term, app: &mut App) -> io::Result<bool> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;
        let Event::Key(key) = event::read()? else { continue };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match app.on_key(key) {
            Action::None => {}
            Action::Quit => return Ok(false),
            Action::Install => return Ok(true),
            Action::Partition(disk) => {
                restore_terminal()?;
                if let Err(e) = app.filesystem.partition_disks(&disk) {
                    app.error = Some(e.to_string());
                }
                *terminal = setup_terminal()?;
                app.refresh_devices();
            }
        }
    }
}
