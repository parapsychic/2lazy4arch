use ratatui::{
    layout::Rect,
    Frame, widgets::{List, Block, Borders}, style::{Style, Modifier},
};

use crate::{app::{App, SubScreens}, ui_utils::show_none_screen};

pub fn essentials_ui(f: &mut Frame<'_>, chunk: Rect, app: &mut App<'_>) {
    match app.current_sub_screen{
        SubScreens::SetupSwap => setup_swap_ui(f, chunk, app),
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

fn setup_swap_ui(f: &mut Frame, chunk: Rect, app: &mut App) {
    let list = List::new(app.swap_sizes.iter().map(|x| format!("{} GB", x)).collect::<Vec<String>>())
        .block(
            Block::default()
                .title("Set the size of the swap file")
                .borders(Borders::ALL),
        )
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">>")
        .repeat_highlight_symbol(true);

    f.render_stateful_widget(list, chunk, &mut app.list_selection);
}
