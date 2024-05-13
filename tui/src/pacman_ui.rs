use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Borders, List},
    Frame,
};

use crate::app::App;

pub fn pacman_setup_ui(f: &mut Frame, chunk: Rect, app: &mut App) {
    let list = List::new(app.reflector_countries.to_vec())
        .block(
            Block::default()
                .title("Select a country to set the mirrorlist to ")
                .borders(Borders::ALL),
        )
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">>")
        .repeat_highlight_symbol(true);

    f.render_stateful_widget(list, chunk, &mut app.list_selection);
}
