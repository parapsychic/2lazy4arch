use crossterm::event::{KeyEvent, KeyCode};

use crate::app::{SubScreens, App, Screens};

pub fn essentials_events(app: &mut App, key: KeyEvent) {
    match app.current_sub_screen {
        SubScreens::SetupSwap => setup_swap_events(app, key),
        SubScreens::None => match key.code {
            _ => app.current_sub_screen = SubScreens::SetupSwap,
        },
        _ => {}
    }
}


pub fn setup_swap_events(app: &mut App, key: KeyEvent) {
    let total_list_item = app.swap_sizes.len();
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
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.current_screen = Screens::StartScreen;
            app.list_selection.select(Some(0));
        }
        _ => {}
    }
}
