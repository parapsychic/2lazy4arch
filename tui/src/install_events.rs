use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{App, Screens, SubScreens};

pub fn install_screen_events(app: &mut App, key: KeyEvent) {
    if let SubScreens::ConfirmInstallation = app.current_sub_screen {
        match key.code {
            KeyCode::Char('y') => {
                app.current_sub_screen = SubScreens::StartInstallation;
            }
            _ => {
                app.current_screen = Screens::StartScreen;
                app.list_selection.select(Some(0));
            }
        }
    }
}

pub fn start_install_screen_events(app: &mut App) {
    app.start_installation = true;
}
