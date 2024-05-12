mod app;
mod ui;

use crate::app::App;
use crate::ui::ui;
use app::{Screens, SubScreens};
use crossterm::event::{self, EnableMouseCapture, Event, KeyEvent};
use crossterm::event::{DisableMouseCapture, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, LeaveAlternateScreen};
use crossterm::terminal::{enable_raw_mode, EnterAlternateScreen};
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::Terminal;
use shell_iface::logger::Logger;
use std::error::Error;
use std::io;
use std::rc::Rc;

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

fn start_screen_events(app: &mut App, key: KeyEvent) {
    let total_list_item = 5;
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            match app.list_selection.selected() {
                Some(x) => {
                    let index: usize;
                    if x == 0 {
                        index = total_list_item - 1;
                    } else {
                        index = x - 1;
                    }
                    app.list_selection.select(Some(index));
                }
                None => {
                    app.list_selection.select(Some(0));
                }
            };
        }
        KeyCode::Down | KeyCode::Char('j') => {
            match app.list_selection.selected() {
                Some(x) => {
                    app.list_selection.select(Some((x + 1) % total_list_item));
                }
                None => {
                    app.list_selection.select(Some(0));
                }
            };
        }
        KeyCode::Enter => match app.list_selection.selected().unwrap() {
            0 => {
                if let Ok(output) = app.filesystem.get_disks() {
                    let mut disks = output
                        .trim()
                        .split('\n')
                        .map(|x| x.to_string())
                        .collect::<Vec<String>>();
                    disks.push("Continue (to select boot disk)".to_string());
                    app.filesystem_drives_list = Rc::new(disks);
                }

                app.current_screen = Screens::Filesystem;
                app.list_selection.select(Some(0));
                app.current_sub_screen = SubScreens::None;
            }
            1 => {
                app.current_screen = Screens::Pacman;
                app.list_selection.select(Some(0));
            }
            2 => {
                app.current_screen = Screens::Essentials;
                app.list_selection.select(Some(0));
            }
            3 => {
                app.current_screen = Screens::Installing;
                app.list_selection.select(Some(0));
            }
            4 => {
                app.current_screen = Screens::Exiting;
            }
            _ => {}
        },
        KeyCode::Backspace => {
            app.text_controller.pop();
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.current_screen = Screens::Exiting;
        }
        _ => {}
    }
}

fn filesystem_screen_events(app: &mut App, key: KeyEvent) {
    match app.current_sub_screen {
        SubScreens::Partitioning => partitioning_events(app, key),
        SubScreens::MountBoot => mount_boot_events(app, key),
        SubScreens::MountHome => mount_home_events(app, key),
        SubScreens::MountRoot => mount_root_events(app, key),
        SubScreens::ConfirmPartitions => confirm_partitions_events(app, key),
        SubScreens::EraseEFI => erase_efi_events(app, key),
        SubScreens::EraseHome => erase_home_events(app, key),
        SubScreens::None => match key.code {
            _ => app.current_sub_screen = SubScreens::Partitioning,
        },
        _ => {}
    }
}

fn partitioning_events(app: &mut App, key: KeyEvent) {
    let total_list_item = app.filesystem_drives_list.len();
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            match app.list_selection.selected() {
                Some(x) => {
                    let index: usize;
                    if x == 0 {
                        index = total_list_item - 1;
                    } else {
                        index = x - 1;
                    }
                    app.list_selection.select(Some(index));
                }
                None => {
                    app.list_selection.select(Some(0));
                }
            };
        }
        KeyCode::Down | KeyCode::Char('j') => {
            match app.list_selection.selected() {
                Some(x) => {
                    app.list_selection.select(Some((x + 1) % total_list_item));
                }
                None => {
                    app.list_selection.select(Some(0));
                }
            };
        }
        KeyCode::Enter => {
            let selection = app.list_selection.selected().unwrap();
            if selection == total_list_item - 1 {
                let list = app
                    .filesystem
                    .lsblk()
                    .unwrap()
                    .iter()
                    .map(|x| {
                        x.children
                            .as_ref()
                            .unwrap()
                            .iter()
                            .map(|x| {
                                let mount_points = x
                                    .mountpoints
                                    .clone()
                                    .unwrap_or_default()
                                    .iter()
                                    .map(|x| x.clone().unwrap_or_default())
                                    .collect::<Vec<String>>()
                                    .join(", ");
                                format!(
                                    "{} {} {}",
                                    x.name,
                                    x.size.clone().unwrap_or_default(),
                                    mount_points
                                )
                            })
                            .collect::<Vec<String>>()
                    })
                    .collect::<Vec<Vec<String>>>()
                    .into_iter()
                    .flatten()
                    .collect::<Vec<String>>();

                app.filesystem_partitions_list = Rc::new(list);
                app.current_sub_screen = SubScreens::MountBoot;
                app.list_selection.select(Some(0));
            } else {
                let disk = format!("/dev/{}", &app.filesystem_drives_list.clone()[selection]);
                let _ = app.filesystem.partition_disks(&disk);
                app.redraw_next_frame = true;
            }
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.current_screen = Screens::StartScreen;
            app.list_selection.select(Some(0));
        }
        _ => {}
    }
}

fn mount_boot_events(app: &mut App, key: KeyEvent) {
    let total_list_item = app.filesystem_partitions_list.len();
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            match app.list_selection.selected() {
                Some(x) => {
                    let index: usize;
                    if x == 0 {
                        index = total_list_item - 1;
                    } else {
                        index = x - 1;
                    }
                    app.list_selection.select(Some(index));
                }
                None => {
                    app.list_selection.select(Some(0));
                }
            };
        }
        KeyCode::Down | KeyCode::Char('j') => {
            match app.list_selection.selected() {
                Some(x) => {
                    app.list_selection.select(Some((x + 1) % total_list_item));
                }
                None => {
                    app.list_selection.select(Some(0));
                }
            };
        }
        KeyCode::Enter => {
            let selected_index = app.list_selection.selected().unwrap();
            let selected = &app.filesystem_partitions_list.clone()[selected_index];
            let selected = selected.split(' ').collect::<Vec<&str>>();

            let disk = format!("/dev/{}", selected.first().unwrap());

            match app.filesystem.set_boot(&disk) {
                Ok(_) => {},
                Err(x) => {
                    app.error_console = x.to_string();
                    return;
                },
            }
            app.list_selection.select(Some(0));
            app.current_sub_screen = SubScreens::MountRoot;
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.current_screen = Screens::Filesystem;
            app.current_sub_screen = SubScreens::Partitioning;
            app.list_selection.select(Some(0));
        }
        _ => {}
    }
}

fn mount_root_events(app: &mut App, key: KeyEvent) {
    let total_list_item = app.filesystem_partitions_list.len();
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            match app.list_selection.selected() {
                Some(x) => {
                    let index: usize;
                    if x == 0 {
                        index = total_list_item - 1;
                    } else {
                        index = x - 1;
                    }
                    app.list_selection.select(Some(index));
                }
                None => {
                    app.list_selection.select(Some(0));
                }
            };
        }
        KeyCode::Down | KeyCode::Char('j') => {
            match app.list_selection.selected() {
                Some(x) => {
                    app.list_selection.select(Some((x + 1) % total_list_item));
                }
                None => {
                    app.list_selection.select(Some(0));
                }
            };
        }
        KeyCode::Enter => {
            let selected_index = app.list_selection.selected().unwrap();
            let selected = &app.filesystem_partitions_list.clone()[selected_index];
            let selected = selected.split(' ').collect::<Vec<&str>>();

            let disk = format!("/dev/{}", selected.first().unwrap());

            match app.filesystem.set_root(&disk) {
                Ok(_) => {},
                Err(x) => {
                    app.error_console = x.to_string();
                    return;
                },
            }

            app.list_selection.select(Some(0));
            app.current_sub_screen = SubScreens::MountHome;
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.current_screen = Screens::Filesystem;
            app.list_selection.select(Some(0));
            app.current_sub_screen = SubScreens::MountBoot;
            app.list_selection.select(Some(0));
        }
        _ => {}
    }
}

fn mount_home_events(app: &mut App, key: KeyEvent) {
    // adding + 1 for "No separate home partition"
    let total_list_item = app.filesystem_partitions_list.len() + 1;
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            match app.list_selection.selected() {
                Some(x) => {
                    let index: usize;
                    if x == 0 {
                        index = total_list_item - 1;
                    } else {
                        index = x - 1;
                    }
                    app.list_selection.select(Some(index));
                }
                None => {
                    app.list_selection.select(Some(0));
                }
            };
        }
        KeyCode::Down | KeyCode::Char('j') => {
            match app.list_selection.selected() {
                Some(x) => {
                    app.list_selection.select(Some((x + 1) % total_list_item));
                }
                None => {
                    app.list_selection.select(Some(0));
                }
            };
        }
        KeyCode::Enter => {
            let selected_index = app.list_selection.selected().unwrap();
            if selected_index == app.filesystem_partitions_list.len() {
                app.filesystem.set_home(None).unwrap();
                app.current_sub_screen = SubScreens::EraseEFI;
            } else {
                let selected = &app.filesystem_partitions_list.clone()[selected_index];
                let selected = selected.split(' ').collect::<Vec<&str>>();

                let disk = format!("/dev/{}", selected.first().unwrap());

                match app.filesystem.set_home(Some(&disk)) {
                    Ok(_) => {},
                    Err(x) => {
                        app.error_console = x.to_string();
                        return;
                    },
                }
                app.current_sub_screen = SubScreens::EraseEFI;
            }
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.current_screen = Screens::Filesystem;
            app.current_sub_screen = SubScreens::MountRoot;
            app.list_selection.select(Some(0));
        }
        _ => {}
    }
}

fn erase_efi_events(app: &mut App, key: KeyEvent) {
    let next_screen = match app.filesystem.get_home() {
        Some(_) => SubScreens::EraseHome,
        None => SubScreens::ConfirmPartitions,
    };

    match key.code {
        KeyCode::Char('y') => {
            app.filesystem.format_boot = true;
            app.current_screen = Screens::Filesystem;
            app.current_sub_screen = next_screen;
        }
        KeyCode::Char('n') => {
            app.filesystem.format_boot = false;
            app.current_screen = Screens::Filesystem;
            app.current_sub_screen = next_screen;
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.current_screen = Screens::Filesystem;
            app.current_sub_screen = SubScreens::MountHome;
            app.list_selection.select(Some(0));
        }
        _ => {}
    }
}

fn erase_home_events(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('y') => {
            app.filesystem.format_home = true;
            app.current_screen = Screens::Filesystem;
            app.current_sub_screen = SubScreens::ConfirmPartitions;
        }
        KeyCode::Char('n') => {
            app.filesystem.format_home = false;
            app.current_screen = Screens::Filesystem;
            app.current_sub_screen = SubScreens::ConfirmPartitions;
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.current_screen = Screens::Filesystem;
            app.current_sub_screen = SubScreens::MountHome;
            app.list_selection.select(Some(0));
        }
        _ => {}
    }
}

fn confirm_partitions_events(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('y') => {
            app.current_screen = Screens::StartScreen;
            app.filesystem_setup_complete = true;
            app.list_selection.select(Some(0));
            app.current_sub_screen = SubScreens::None;
        }
        KeyCode::Enter => {}
        _ => {
            app.current_screen = Screens::Filesystem;
            app.list_selection.select(Some(0));
            app.current_sub_screen = SubScreens::MountBoot;
        }
    }
}

fn pacman_screen_events(app: &mut App, key: KeyEvent) {
   let total_list_item = app.reflector_countries.len();
    match key.code{
        KeyCode::Up | KeyCode::Char('k') => {
            match app.list_selection.selected() {
                Some(x) => {
                    let index: usize;
                    if x == 0 {
                        index = total_list_item - 1;
                    } else {
                        index = x - 1;
                    }
                    app.list_selection.select(Some(index));
                }
                None => {
                    app.list_selection.select(Some(0));
                }
            };
        }
        KeyCode::Down | KeyCode::Char('j') => {
            match app.list_selection.selected() {
                Some(x) => {
                    app.list_selection.select(Some((x + 1) % total_list_item));
                }
                None => {
                    app.list_selection.select(Some(0));
                }
            };
        }
        KeyCode::Enter => {
            let selected_index = app.list_selection.selected().unwrap();
            let selected = app.reflector_countries.clone()[selected_index];
            app.selected_reflector_country = String::from(selected);
            app.current_screen = Screens::StartScreen;
            app.list_selection.select(Some(0));
            app.pacman_setup_complete = true;
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.current_screen = Screens::StartScreen;
            app.current_sub_screen = SubScreens::None;
            app.list_selection.select(Some(0));
        }
        _ => {}
    }
}
