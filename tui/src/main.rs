mod app;
mod essentials_events;
mod essentials_ui;
mod filesystem_events;
mod filesystem_ui;
mod install_events;
mod install_ui;
mod pacman_events;
mod pacman_ui;
mod post_install;
mod start_screen_events;
mod ui;
mod ui_utils;

use crate::app::App;
use crate::ui::ui;
use app::{Screens, SubScreens};
use crossterm::event::{self, EnableMouseCapture, Event};
use crossterm::event::{DisableMouseCapture, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, LeaveAlternateScreen};
use crossterm::terminal::{enable_raw_mode, EnterAlternateScreen};
use essentials_events::essentials_events;
use filesystem_events::filesystem_screen_events;
use install_events::{install_screen_events, start_install_screen_events};
use installer::install;
use installer::utils::INSTALL_SUCCESS_FLAG;
use pacman_events::pacman_screen_events;
use post_install::run_post_install;
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::Terminal;
use shell_iface::logger::Logger;
use start_screen_events::start_screen_events;
use std::error::Error;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

fn main() -> Result<(), Box<dyn Error>> {
    // if this is an installed system
    let file_path = INSTALL_SUCCESS_FLAG;
    if check_if_installed(file_path) {
        // Run postinstall
        run_post_install();
        // stop running program
        return Ok(());
    }
    // setup terminal
    enable_raw_mode()?;
    let mut stderr = io::stderr(); // This is a special case. Normally using stdout is fine
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let logger = Logger::new(false);
    let mut app = App::new(&logger);
    let res = run_app(&mut terminal, &mut app);
    let _ = terminal.clear();

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Ok(do_install) = res {
        if do_install {
            println!("\nStarting installation");

            // Create or open the file
            match File::create("log.txt") {
                Ok(file) => file,
                Err(e) => {
                    eprintln!("Failed to create file: {}", e);
                    return Ok(());
                }
            };

            // install here
            install(
                &mut app.filesystem,
                &mut app.base_installer,
                &mut app.essentials,
                &mut app.pacman,
                &app.selected_reflector_country,
                &app.selected_timezone,
                &app.selected_locale,
                &app.selected_encoding,
                app.swap_size,
                &app.username,
                &app.password,
                &app.root_password,
                &app.hostname,
            );
        }
    } else if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

fn check_if_installed(file_path: &str) -> bool {
    // Check if the file exists
    if Path::new(file_path).exists() {
        // Read the contents of the file
        let mut file = match std::fs::File::open(file_path) {
            Ok(x) => x,
            Err(_) => return false,
        };
        let mut content = String::new();
        if file.read_to_string(&mut content).is_err() {
            return false;
        }

        // Check if the content is "true"
        if content.trim() == "true" {
            return true;
        }
    }

    return false;
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<bool> {
    terminal.clear()?;
    loop {
        if app.start_installation {
            terminal.clear()?;
            return Ok(true);
        }
        terminal.draw(|f| ui(f, app))?;
        if let Event::Key(key) = event::read()? {
            app.error_console.clear();
            if key.kind == event::KeyEventKind::Release {
                // Skip events that are not KeyEventKind::Press
                continue;
            }
            match app.current_screen {
                Screens::StartScreen if key.kind == KeyEventKind::Press => {
                    start_screen_events(app, key)
                }
                Screens::Filesystem if key.kind == KeyEventKind::Press => {
                    filesystem_screen_events(app, key)
                }
                Screens::Pacman if key.kind == KeyEventKind::Press => {
                    pacman_screen_events(app, key)
                }
                Screens::Essentials if key.kind == KeyEventKind::Press => {
                    essentials_events(app, key)
                }
                Screens::Installing => match app.current_sub_screen {
                    SubScreens::StartInstallation => {
                        start_install_screen_events(app);
                    }
                    SubScreens::ConfirmInstallation if key.kind == KeyEventKind::Press => {
                        install_screen_events(app, key)
                    }
                    _ => {
                        app.current_screen = Screens::StartScreen;
                        app.list_selection.select(Some(0));
                    }
                },
                Screens::Exiting => match key.code {
                    KeyCode::Char('y') => {
                        return Ok(false);
                    }
                    _ => {
                        app.current_screen = Screens::StartScreen;
                        app.list_selection.select(Some(0));
                    }
                },
                _ => {
                    app.current_screen = Screens::StartScreen;
                    app.list_selection.select(Some(0));
                }
            };
        }
        if app.redraw_next_frame {
            terminal.clear()?;
            app.redraw_next_frame = false;
        }
    }
}
