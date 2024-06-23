use crate::app::{App, Screens, SubScreens};
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use std::rc::Rc;

pub fn start_screen_events(app: &mut App, key: KeyEvent) {
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
                app.current_sub_screen = SubScreens::Partitioning;
                app.list_selection.select(Some(0));
            }
            1 => {
                app.current_screen = Screens::Pacman;
                app.list_selection.select(Some(0));
            }
            2 => {
                app.current_screen = Screens::Essentials;
                app.current_sub_screen = SubScreens::SetupSwap;
                app.list_selection.select(Some(0));
            }
            3 => {
                if app.pacman_setup_complete
                    && app.filesystem_setup_complete
                    && app.essentials_setup_complete
                {
                    app.current_screen = Screens::Installing;
                    app.current_sub_screen = SubScreens::ConfirmInstallation;
                    app.list_selection.select(Some(0));
                } else {
                    app.error_console = "Some components are not set up. Please setup all components before installing...".to_string();
                }
            }
            4 => {
                app.current_screen = Screens::Exiting;
            }
            _ => {}
        },
        KeyCode::Esc | KeyCode::Char('q') => {
            app.current_screen = Screens::Exiting;
        }
        _ => {}
    }
}

