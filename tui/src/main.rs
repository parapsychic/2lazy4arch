mod app;
mod term;
mod ui;

use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
    process::{Command, ExitCode},
    thread,
    time::Duration,
};

use app::{Action, App};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
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
    if let Some(i) = args.iter().position(|a| a == "--run-install") {
        return run_installer(args.get(i + 1).map(String::as_str).unwrap_or_default());
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
    let start = |app: &App| spawn_installer(serde_json::to_value(&app.cfg)?, &Source::Dir(".".into()));
    tui_result(run_tui(&mut app, &start))
}

/// After the TUI closes: what happened, in the normal terminal.
fn tui_result(result: io::Result<Option<bool>>) -> ExitCode {
    match result {
        Ok(None) => ExitCode::SUCCESS,
        Ok(Some(true)) => {
            println!("Installation finished. The log is at {LOG_FILE}.");
            ExitCode::SUCCESS
        }
        Ok(Some(false)) => {
            eprintln!("The install stopped before finishing. The log is at {LOG_FILE}.");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("Terminal error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Starts this binary with --run-install on a pseudo-terminal. The config goes
/// through a root-only file the child deletes once read.
fn spawn_installer(config: Value, source: &Source) -> anyhow::Result<term::Install> {
    let dir = "/tmp/2lazy4arch";
    fs::create_dir_all(dir)?;
    let path = format!("{dir}/run.json");
    let job = serde_json::json!({ "config": config, "source": source.base() });
    OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&path)?.write_all(job.to_string().as_bytes())?;
    let exe = std::env::current_exe()?;
    term::Install::spawn(&exe.display().to_string(), &["--run-install", &path], 24, 80)
}

/// The child side of spawn_installer: the install itself, printing to the pty.
fn run_installer(path: &str) -> ExitCode {
    let job = fs::read_to_string(path);
    let _ = fs::remove_file(path);
    let parsed = job.map_err(anyhow::Error::from).and_then(|text| {
        let job: Value = serde_json::from_str(&text)?;
        let source = Source::from_base(job["source"].as_str().unwrap_or("."));
        let mut cfg: Config = serde_json::from_value(job["config"].clone())?;
        cfg.load_lists(&source)?;
        Ok((cfg, source, job["config"].clone()))
    });
    match parsed {
        Ok((cfg, source, value)) => {
            let logger = Logger::new(false);
            let sys = System::probe(&logger);
            run_install(&cfg, &sys, &source, &logger, value, true)
        }
        Err(e) => {
            eprintln!("Couldn't read the install job: {e:#}");
            ExitCode::FAILURE
        }
    }
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
        let mut app = App::preview(&logger, sys, cfg);
        let start = |_: &App| spawn_installer(value.clone(), &source);
        let result = run_tui(&mut app, &start);
        if let Ok(None) = result {
            println!("Nothing was changed.");
        }
        return tui_result(result);
    }
    run_install(&cfg, &sys, &source, &logger, value, false)
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

/// Runs the TUI, the install included once y is pressed. Ok(None): quit before
/// installing; Ok(Some(ok)): the install ran and succeeded or not.
fn run_tui(app: &mut App, start: &dyn Fn(&App) -> anyhow::Result<term::Install>) -> io::Result<Option<bool>> {
    // Don't leave the console in raw mode if we crash.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal();
        default_hook(info);
    }));
    let result = setup_terminal().and_then(|mut terminal| event_loop(&mut terminal, app, start));
    let _ = restore_terminal();
    let _ = std::panic::take_hook();
    result
}

fn event_loop(terminal: &mut Term, app: &mut App, start: &dyn Fn(&App) -> anyhow::Result<term::Install>) -> io::Result<Option<bool>> {
    loop {
        if let Some(install) = &mut app.install {
            install.pump();
        }
        terminal.draw(|f| ui::draw(f, app))?;
        // redraw often while installing, for the output and the spinner
        if !event::poll(Duration::from_millis(80))? {
            continue;
        }
        let Event::Key(key) = event::read()? else { continue };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        if let Some(install) = &mut app.install {
            let running = install.status == term::Status::Running;
            if key.code != KeyCode::Char('q') {
                install.abort_armed = false;
            }
            match (install.status, key.code) {
                (_, KeyCode::PageUp) => install.scroll(10),
                (_, KeyCode::PageDown) => install.scroll(-10),
                (term::Status::Finished(ok), KeyCode::Char('q') | KeyCode::Enter | KeyCode::Esc) => return Ok(Some(ok)),
                // q twice: ctrl+c to the installer
                (_, KeyCode::Char('q')) if install.abort_armed => install.send(crossterm::event::KeyEvent::new(KeyCode::Char('c'), crossterm::event::KeyModifiers::CONTROL)),
                (_, KeyCode::Char('q')) => install.abort_armed = true,
                (_, KeyCode::Char('p') | KeyCode::Char('P')) if running => install.paused = !install.paused,
                (_, KeyCode::Char('s') | KeyCode::Char('S')) => install.toggle_autoscroll(),
                (_, KeyCode::Char('c') | KeyCode::Char('C')) if key.modifiers.is_empty() => install.clear(),
                // the rest (y/n for its prompt, ctrl+c) goes to the installer
                _ if running => install.send(key),
                _ => {}
            }
            continue;
        }

        match app.on_key(key) {
            Action::None => {}
            Action::Quit => return Ok(None),
            Action::Install => match start(app) {
                Ok(install) => app.install = Some(install),
                Err(e) => app.error = Some(format!("Couldn't start the install: {e:#}")),
            },
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
