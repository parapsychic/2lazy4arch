use crate::app::{App,Screens, SubScreens};
use crossterm::event::KeyEvent;
use crossterm::event::KeyCode;

pub fn pacman_screen_events(app: &mut App, key: KeyEvent) {
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
