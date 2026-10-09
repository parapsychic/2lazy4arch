mod app;
mod post_install;
mod ui;

use std::{
    fs,
    io::{self, Write},
    process::ExitCode,
};

use app::{Action, App};
use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use installer::{
    config::Desktop,
    utils::{INSTALL_SUCCESS_FLAG, AUR_QUEUE},
    INSTALLED_BINARY,
};
use ratatui::{backend::CrosstermBackend, Terminal};
use shell_iface::logger::Logger;

type Term = Terminal<CrosstermBackend<io::Stdout>>;

fn main() -> ExitCode {
    // Part 2 on the installed system
    if fs::read_to_string(INSTALL_SUCCESS_FLAG).is_ok_and(|c| c.trim() == "true") {
        return post_install::run_post_install();
    }

    let logger = Logger::new(false);
    let mut app = App::new(&logger);

    // Don't leave the console in raw mode if we crash.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal();
        default_hook(info);
    }));

    let result = setup_terminal().and_then(|mut terminal| run_app(&mut terminal, &mut app));
    let _ = restore_terminal();
    match result {
        Ok(true) => {}
        Ok(false) => return ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Terminal error: {e}");
            return ExitCode::FAILURE;
        }
    }

    println!("Starting installation. Commands are logged to shell_log.txt.");
    let missing = match installer::check_packages(&logger, &app.cfg) {
        Ok(missing) => missing,
        Err(e) => {
            eprintln!("\n\x1b[1;31mCouldn't sync the package databases:\x1b[0m {e:#}");
            eprintln!("Is the network up? Nothing was changed.");
            return ExitCode::FAILURE;
        }
    };
    if !missing.is_empty() {
        println!("\n\x1b[1;33mNot found in the repos:\x1b[0m {}", missing.join(" "));
        print!("They will be skipped. Nothing has been changed yet. Continue? [y/N] ");
        let _ = io::stdout().flush();
        let mut answer = String::new();
        let _ = io::stdin().read_line(&mut answer);
        if !answer.trim().eq_ignore_ascii_case("y") {
            println!("Stopped. Nothing was changed.");
            return ExitCode::SUCCESS;
        }
    }

    if let Err(e) = installer::install(&mut app.filesystem, &logger, &app.cfg) {
        eprintln!("\n\x1b[1;31mInstallation failed:\x1b[0m {e:#}");
        eprintln!("See shell_log.txt for every command that ran.");
        return ExitCode::FAILURE;
    }

    println!("\n\x1b[1;32mInstallation finished.\x1b[0m Reboot and log in as {}.", app.cfg.username);
    let aur = fs::read_to_string(AUR_QUEUE).unwrap_or_default();
    if !aur.trim().is_empty() {
        println!("Then run `2lazy4arch` to install from the AUR: {}", aur.split_whitespace().collect::<Vec<_>>().join(" "));
    } else {
        println!("Optional: run `2lazy4arch` ({INSTALLED_BINARY}) after rebooting to set up yay and your own package lists.");
    }
    if app.cfg.desktop == Desktop::Dwm {
        println!("dwm, dmenu and st sources are in /usr/local/src. Edit config.h, then `sudo make install`.");
    }
    ExitCode::SUCCESS
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

/// Ok(true) means install.
fn run_app(terminal: &mut Term, app: &mut App) -> io::Result<bool> {
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
