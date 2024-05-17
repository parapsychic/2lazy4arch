use crossterm::event::{KeyCode, KeyEvent};
use installer::essentials::Bootloader;

use crate::app::{App, Screens, SubScreens};

pub fn essentials_events(app: &mut App, key: KeyEvent) {
    match app.current_sub_screen {
        SubScreens::SetupSwap => setup_swap_events(app, key),
        SubScreens::SelectTimezone => setup_timezone_events(app, key),
        SubScreens::SelectLocale => setup_locale_events(app, key),
        SubScreens::SetupHostname => setup_hostname_events(app, key),
        SubScreens::SetupRootPassword => setup_root_password_events(app, key),
        SubScreens::SetupExtraPrograms => setup_extra_packages_events(app, key),
        SubScreens::SetupBootloader => setup_boot_loader_events(app, key),
        SubScreens::SetupUser => setup_user_events(app, key),
        SubScreens::None => match key.code {
            _ => app.current_sub_screen = SubScreens::SetupSwap,
        },
        _ => {}
    }
}

fn setup_user_events(app: &mut App<'_>, key: KeyEvent) {
    let total_list_item = 2;
    match key.code {
        KeyCode::Up => {
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
        KeyCode::Down | KeyCode::Tab => {
            match app.list_selection.selected() {
                Some(x) => {
                    app.list_selection.select(Some((x + 1) % total_list_item));
                }
                None => {
                    app.list_selection.select(Some(0));
                }
            };
        }
        KeyCode::Char(x) => {
            let selection = app.list_selection.selected().unwrap();
            if selection == 0 {
                app.username.push(x);
            } else {
                app.password.push(x);
            }
        }
        KeyCode::Backspace => {
            let selection = app.list_selection.selected().unwrap();
            if selection == 0 {
                app.username.pop();
            } else {
                app.password.pop();
            }
        }
        KeyCode::Enter => {
            let selection = app.list_selection.selected().unwrap();
            if selection == 0 {
                app.list_selection.select(Some(1));
            } else {
                app.current_screen = Screens::StartScreen;
                app.current_sub_screen = SubScreens::None;
                app.list_selection.select(Some(0));
                app.essentials_setup_complete = true;
            }
        }
        KeyCode::Esc => {
            app.current_sub_screen = SubScreens::SetupBootloader;
            app.list_selection.select(Some(0));
        }
        _ => {}
    }
}

fn setup_boot_loader_events(app: &mut App<'_>, key: KeyEvent) {
    let total_list_item = 2;
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
            if selection == 0 {
                app.selected_bootloader = Bootloader::Grub;
            } else {
                app.selected_bootloader = Bootloader::SystemDBoot;
            }
            app.current_sub_screen = SubScreens::SetupUser;
            app.list_selection.select(Some(0));
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.current_sub_screen = SubScreens::SetupExtraPrograms;
            app.list_selection.select(Some(0));
        }
        _ => {}
    }
}

fn setup_extra_packages_events(app: &mut App<'_>, key: KeyEvent) {
    match key.code {
        KeyCode::Enter => {
            app.current_sub_screen = SubScreens::SetupBootloader;
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.current_sub_screen = SubScreens::SetupRootPassword;
            app.list_selection.select(Some(0));
        }
        _ => {}
    }
}

fn setup_root_password_events(app: &mut App<'_>, key: KeyEvent) {
    match key.code {
        KeyCode::Char(x) => {
            app.root_password.push(x);
        }
        KeyCode::Backspace => {
            app.root_password.pop();
        }
        KeyCode::Enter => {
            app.root_password.clear();
            app.current_sub_screen = SubScreens::SetupExtraPrograms;
            app.list_selection.select(Some(0));
        }
        KeyCode::Esc => {
            app.current_sub_screen = SubScreens::SetupHostname;
            app.list_selection.select(Some(0));
        }
        _ => {}
    }
}

fn setup_hostname_events(app: &mut App<'_>, key: KeyEvent) {
    match key.code {
        KeyCode::Char(x) => {
            app.hostname.push(x);
        }
        KeyCode::Backspace => {
            app.hostname.pop();
        }
        KeyCode::Enter => {
            app.hostname.clear();
            app.current_sub_screen = SubScreens::SetupRootPassword;
            app.list_selection.select(Some(0));
        }
        KeyCode::Esc => {
            app.current_sub_screen = SubScreens::SelectLocale;
            app.list_selection.select(Some(0));
        }
        _ => {}
    }
}

fn setup_locale_events(app: &mut App<'_>, key: KeyEvent) {
    let total_list_item = app.locales_list.len();
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
            let (locale, encoding) = app.locales_list[selection].split_once(' ').unwrap();
            app.selected_encoding = locale.to_string();
            app.selected_encoding = encoding.to_string();
            app.current_sub_screen = SubScreens::SetupHostname;
            app.list_selection.select(Some(0));
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.current_sub_screen = SubScreens::SelectTimezone;
            app.list_selection.select(Some(0));
        }
        _ => {}
    }
}

fn setup_timezone_events(app: &mut App<'_>, key: KeyEvent) {
    let total_list_item = app.timezones.len();
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
            app.selected_timezone = app.timezones[selection].to_string();
            app.current_sub_screen = SubScreens::SelectLocale;
            app.list_selection.select(Some(0));
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.current_sub_screen = SubScreens::SetupSwap;
            app.list_selection.select(Some(0));
        }
        _ => {}
    }
}

pub fn setup_swap_events(app: &mut App, key: KeyEvent) {
    let total_list_item = app.swap_sizes_list.len();
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
            app.swap_size = app.swap_sizes_list[selection];
            app.current_sub_screen = SubScreens::SelectTimezone;
            app.list_selection.select(Some(0));
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.current_screen = Screens::StartScreen;
            app.list_selection.select(Some(0));
        }
        _ => {}
    }
}
