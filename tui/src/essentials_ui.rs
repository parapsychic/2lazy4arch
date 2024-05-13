use ratatui::{
    layout::Rect,
    Frame,
};

use crate::{app::{App, SubScreens}, ui_utils::show_none_screen};

pub fn essentials_ui(f: &mut Frame<'_>, chunk: Rect, app: &mut App<'_>) {
    match app.current_sub_screen{
        SubScreens::SetupSwap => {},
        SubScreens::SelectLocale => {},
        SubScreens::SelectTimezone => {},
        SubScreens::SetupHostname => {},
        SubScreens::SetupRootPassword => {},
        SubScreens::SetupExtraPrograms => {},
        SubScreens::SetupBootloader => {},
        SubScreens::SetupUser => {},
        _ => show_none_screen(f, chunk, "Additional Configuration"),
    }
}
