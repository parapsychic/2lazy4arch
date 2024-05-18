use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, Paragraph},
    Frame,
};

use crate::{
    app::{App, Screens},
    essentials_ui::essentials_ui,
    filesystem_ui::filesystem_screen_ui,
    pacman_ui::pacman_setup_ui, install_ui::install_screen_ui,
};

pub fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(f.size());

    let title_block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default());

    let title = Paragraph::new(Text::styled(
        "2Lazy4Arch: Install Arch Fast",
        Style::default().fg(Color::Green),
    ))
    .block(title_block);

    let current_navigation_text = vec![
        // The first half of the text
        match app.current_screen {
            Screens::StartScreen => Span::styled("Start Screen", Style::default().fg(Color::Green)),
            Screens::Filesystem => Span::styled("Filesystem", Style::default().fg(Color::Yellow)),
            Screens::Exiting => Span::styled("Exiting", Style::default().fg(Color::LightRed)),
            Screens::Pacman => Span::styled("Pacman", Style::default().fg(Color::Yellow)),
            Screens::Essentials => Span::styled("Essentials", Style::default().fg(Color::Yellow)),
            Screens::Installing => Span::styled("Installing", Style::default().fg(Color::Green)),
        }
        .to_owned(),
        // A white divider bar to separate the two sections
        Span::styled(" | ", Style::default().fg(Color::White)),
        // The final section of the text, with hints on what the user is editing
        Span::styled("Not Editing Anything", Style::default().fg(Color::DarkGray)),
    ];

    let mode_footer = Paragraph::new(Line::from(current_navigation_text))
        .block(Block::default().borders(Borders::ALL));

    let current_keys_hint = {
        match app.current_screen {
            Screens::StartScreen => Span::styled(
                "(esc) to quit / (enter) to select",
                Style::default().fg(Color::Red),
            ),
            Screens::Exiting => Span::styled(
                "(y) to quit / (any key) to cancel",
                Style::default().fg(Color::Red),
            ),
            Screens::Installing => Span::styled(
                "(y) to confirm / (any key) to cancel",
                Style::default().fg(Color::Red),
            ),

            _ => Span::styled(
                "(esc) to quit / (enter) to select / (up/down) to change selection",
                Style::default().fg(Color::Red),
            ),
        }
    };

    let key_notes_footer =
        Paragraph::new(Line::from(current_keys_hint)).block(Block::default().borders(Borders::ALL));

    let footer_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[2]);

    let main_area = match app.error_console.is_empty() {
        true => Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(4)])
            .split(chunks[1]),
        false => Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(4), Constraint::Max(3)])
            .split(chunks[1]),
    };
    screen_switcher(f, main_area[0], app);

    if !app.error_console.is_empty() {
        let errors = Paragraph::new(Line::from(format!("ERROR: {}", app.error_console)))
            .block(Block::default().borders(Borders::ALL));

        f.render_widget(errors, main_area[1]);
    }
    f.render_widget(mode_footer, footer_chunks[0]);
    f.render_widget(key_notes_footer, footer_chunks[1]);
    f.render_widget(title, chunks[0]);
}

fn screen_switcher(f: &mut Frame, chunk: Rect, app: &mut App) {
    match app.current_screen {
        Screens::StartScreen => start_screen_ui(f, chunk, app),
        Screens::Filesystem => filesystem_screen_ui(f, chunk, app),
        Screens::Pacman => pacman_setup_ui(f, chunk, app),
        Screens::Essentials => essentials_ui(f, chunk, app),
        Screens::Exiting => start_screen_ui(f, chunk, app),
        Screens::Installing => install_screen_ui(f, chunk, app),
    }
}

fn start_screen_ui(f: &mut Frame, chunk: Rect, app: &mut App) {
    let items = [
        &format!(
            "{}Setup Filesystem",
            match app.filesystem_setup_complete {
                true => "✔ ",
                false => "",
            }
        ),
        &format!(
            "{}Setup Pacman Mirrors",
            match app.pacman_setup_complete {
                true => "✔ ",
                false => "",
            }
        ),
        &format!(
            "{}Setup Additional Configurations",
            match app.essentials_setup_complete {
                true => "✔ ",
                false => "",
            }
        ),
        "Install",
        "Exit",
    ];
    let list = List::new(items)
        .block(Block::default().title("Main Menu").borders(Borders::ALL))
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">> ")
        .repeat_highlight_symbol(true);

    f.render_stateful_widget(list, chunk, &mut app.list_selection);
}
