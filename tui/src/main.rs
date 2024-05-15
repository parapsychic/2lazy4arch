mod app;
mod ui;
mod filesystem_events;
mod pacman_events;
mod events_handler;
mod filesystem_ui;
mod pacman_ui;
mod essentials_ui;
mod ui_utils;
mod essentials_events;

use crate::app::App;
use crate::ui::ui;
use app::Screens;
use crossterm::event::{self, EnableMouseCapture, Event};
use crossterm::event::{DisableMouseCapture, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, LeaveAlternateScreen};
use crossterm::terminal::{enable_raw_mode, EnterAlternateScreen};
use essentials_events::essentials_events;
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::Terminal;
use shell_iface::logger::Logger;
use std::error::Error;
use std::io;
use filesystem_events::filesystem_screen_events;
use events_handler::start_screen_events;
use pacman_events::pacman_screen_events;


fn main() -> Result<(), Box<dyn Error>> {
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

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Ok(do_install) = res {
        if do_install {
            // install here
        }
    } else if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<bool> {
    loop {
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
                Screens::Exiting => match key.code {
                    KeyCode::Char('y') => {
                        return Ok(true);
                    }
                    _ => {
                        app.current_screen = Screens::StartScreen;
                        app.list_selection.select(Some(0));
                    }
                },
                _ => {}
            };
        }
        if app.redraw_next_frame {
            terminal.clear()?;
            app.redraw_next_frame = false;
        }
    }
}



