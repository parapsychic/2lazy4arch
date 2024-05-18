use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, Paragraph},
    Frame,
};

use crate::{
    app::{App, SubScreens},
    ui_utils::{centered_rect, show_none_screen},
};

pub fn essentials_ui(f: &mut Frame<'_>, chunk: Rect, app: &mut App<'_>) {
    match app.current_sub_screen {
        SubScreens::SetupSwap => setup_swap_ui(f, chunk, app),
        SubScreens::SelectLocale => setup_locale_ui(f, chunk, app),
        SubScreens::SelectTimezone => setup_timezone_ui(f, chunk, app),
        SubScreens::SetupHostname => setup_hostname_ui(f, chunk, app),
        SubScreens::SetupRootPassword => setup_root_password_ui(f, chunk, app),
        SubScreens::SetupExtraPrograms => setup_extra_programs_ui(f, chunk, app),
        SubScreens::SetupBootloader => setup_bootloader_ui(f, chunk, app),
        SubScreens::SetupUser => setup_user_ui(f, chunk, app),
        SubScreens::SetupSuperUserUtility => setup_superuser_ui(f, chunk, app),
        _ => show_none_screen(f, chunk, "Additional Configuration"),
    }
}

fn setup_superuser_ui(f: &mut Frame<'_>, chunk: Rect, app: &mut App<'_>) {
    let list = List::new(vec!["sudo", "doas"])
        .block(
            Block::default()
                .title("Select the super user utility")
                .borders(Borders::ALL),
        )
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">>")
        .repeat_highlight_symbol(true);

    f.render_stateful_widget(list, chunk, &mut app.list_selection);
}

fn setup_locale_ui(f: &mut Frame<'_>, chunk: Rect, app: &mut App<'_>) {
    let list = List::new(app.locales_list.to_vec())
        .block(
            Block::default()
                .title("Select the locale")
                .borders(Borders::ALL),
        )
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">>")
        .repeat_highlight_symbol(true);

    f.render_stateful_widget(list, chunk, &mut app.list_selection);
}

fn setup_timezone_ui(f: &mut Frame<'_>, chunk: Rect, app: &mut App<'_>) {
    let list = List::new(app.timezones.to_vec())
        .block(
            Block::default()
                .title("Select the timezone")
                .borders(Borders::ALL),
        )
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">>")
        .repeat_highlight_symbol(true);

    f.render_stateful_widget(list, chunk, &mut app.list_selection);
}

fn setup_hostname_ui(f: &mut Frame<'_>, chunk: Rect, app: &mut App<'_>) {
    let hostname_ui = Paragraph::new(Line::from(app.hostname.clone()))
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Left)
        .block(
            Block::default()
                .title("Set a hostname")
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Yellow)),
        );
    f.render_widget(hostname_ui, centered_rect(70, 10, chunk));
}

fn setup_root_password_ui(f: &mut Frame<'_>, chunk: Rect, app: &mut App<'_>) {
    let root_password_ui = Paragraph::new(Line::from("*".repeat(app.root_password.len())))
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Left)
        .block(
            Block::default()
                .title("Set a root password")
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Yellow)),
        );
    f.render_widget(root_password_ui, centered_rect(70, 10, chunk));
}

fn setup_extra_programs_ui(f: &mut Frame<'_>, chunk: Rect, app: &mut App<'_>) {
    let programs_ui = Paragraph::new(Line::from("Still figuring this out..."))
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .title("Setting up extra programs: ")
                .borders(Borders::ALL)
                .style(Style::default().fg(match app.list_selection.selected() {
                    Some(0) => Color::Yellow,
                    _ => Color::Red,
                })),
        );
    f.render_widget(programs_ui, centered_rect(70, 10, chunk));
}

fn setup_bootloader_ui(f: &mut Frame<'_>, chunk: Rect, app: &mut App<'_>) {
    let list = List::new(vec!["Grub", "systemdBoot"])
        .block(
            Block::default()
                .title("Select the bootloader: ")
                .borders(Borders::ALL),
        )
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">>")
        .repeat_highlight_symbol(true);

    f.render_stateful_widget(list, chunk, &mut app.list_selection);
}

fn setup_user_ui(f: &mut Frame<'_>, chunk: Rect, app: &mut App<'_>) {
    let username_ui = Paragraph::new(Line::from(app.username.clone()))
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Left)
        .block(
            Block::default()
                .title("Set username for normal user: ")
                .borders(Borders::ALL)
                .style(Style::default().fg(match app.list_selection.selected() {
                    Some(0) => Color::Yellow,
                    _ => Color::Red,
                })),
        );

    let password_ui = Paragraph::new(Line::from("*".repeat(app.password.len())))
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Left)
        .block(
            Block::default()
                .title("Set password for normal user: ")
                .borders(Borders::ALL)
                .style(Style::default().fg(match app.list_selection.selected() {
                    Some(1) => Color::Yellow,
                    _ => Color::Red,
                })),
        );

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(3)])
        .split(centered_rect(70, 30, chunk));

    f.render_widget(username_ui, layout[0]);
    f.render_widget(password_ui, layout[1]);
}

fn setup_swap_ui(f: &mut Frame, chunk: Rect, app: &mut App) {
    let list = List::new(
        app.swap_sizes_list
            .iter()
            .map(|x| format!("{} GB", x))
            .collect::<Vec<String>>(),
    )
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
