use ratatui::{
    layout::{Constraint, Direction, Layout, Rect, Alignment},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, Paragraph, Wrap},
    Frame,
};

use crate::{
    app::{App, Screens, SubScreens},
    essentials_ui::essentials_ui,
    filesystem_ui::filesystem_screen_ui,
    install_ui::install_screen_ui,
    pacman_ui::pacman_setup_ui, ui_utils::centered_rect,
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

        match app.current_sub_screen {
    SubScreens::None => 
        Span::styled("", Style::default().fg(Color::DarkGray)),
    SubScreens::Partitioning => Span::styled("Partitioning ", Style::default().fg(Color::DarkGray)),
    SubScreens::MountBoot => Span::styled("Mount Boot ", Style::default().fg(Color::DarkGray)),
    SubScreens::MountHome => Span::styled("Mount Home ", Style::default().fg(Color::DarkGray)),
    SubScreens::MountRoot => Span::styled("Mount Root ", Style::default().fg(Color::DarkGray)),
    SubScreens::EraseEFI => Span::styled("Erase EFI ", Style::default().fg(Color::DarkGray)),
    SubScreens::EraseHome => Span::styled("Erase Home ", Style::default().fg(Color::DarkGray)),
    SubScreens::MountExtraPartition => Span::styled("Mount Extra Partitions ", Style::default().fg(Color::DarkGray)),
    SubScreens::MountExtraPartition__Insert => Span::styled("Add New Partition", Style::default().fg(Color::DarkGray)),
    SubScreens::ConfirmPartitions => Span::styled("Confirm Partitions ", Style::default().fg(Color::DarkGray)),
    SubScreens::SetupSwap => Span::styled("Setup Swap ", Style::default().fg(Color::DarkGray)),
    SubScreens::SelectTimezone => Span::styled("Select Timezone ", Style::default().fg(Color::DarkGray)),
    SubScreens::SelectLocale => Span::styled("Select Locale ", Style::default().fg(Color::DarkGray)),
    SubScreens::SetupHostname => Span::styled("Setup Hostname ", Style::default().fg(Color::DarkGray)),
    SubScreens::SetupRootPassword => Span::styled("Setup Root Password ", Style::default().fg(Color::DarkGray)),
    SubScreens::SetupBootloader => Span::styled("Setup Bootloader ", Style::default().fg(Color::DarkGray)),
    SubScreens::SetupSuperUserUtility => Span::styled("Setup SuperUser Utility ", Style::default().fg(Color::DarkGray)),
    SubScreens::SetupUser => Span::styled("Setup User ", Style::default().fg(Color::DarkGray)),
    SubScreens::ConfirmInstallation => Span::styled("Confirm  Installation ", Style::default().fg(Color::DarkGray)),
    SubScreens::StartInstallation => Span::styled("Start Installation ", Style::default().fg(Color::DarkGray)),
}
        .to_owned(),
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
            Screens::Filesystem => match app.current_sub_screen {
                SubScreens::MountExtraPartition => Span::styled(
                    "(esc) to go back / (enter) to select / (delete) or (x) to delete / (up/down) to change selection",
                    Style::default().fg(Color::Red),
                ),

                _ => Span::styled(
                    "(esc) to go back / (enter) to select / (up/down) to change selection",
                    Style::default().fg(Color::Red),
                ),
            },
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
        Screens::Exiting => exit_screen_ui(f, chunk, app),
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

fn exit_screen_ui(f: &mut Frame, chunk: Rect, _: &mut App) {
    let msg = Paragraph::new(Line::from(
        "Do you want to exit the installer?",
    ))
    .style(Style::default().fg(Color::Yellow))
    .alignment(Alignment::Center)
    .block(Block::default());

    let yes_msg = Paragraph::new(Line::from("[Y] Yes"))
        .style(Style::default().fg(Color::Green))
        .alignment(Alignment::Center)
        .wrap(Wrap::default())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Red)),
        );

    let no_msg = Paragraph::new(Line::from("[N] No"))
        .style(Style::default().fg(Color::Green))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Red)),
        );

    let prompt_box = centered_rect(70, 30, chunk);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(3),
            Constraint::Min(1),
        ])
        .split(prompt_box);

    let action_btn_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(centered_rect(75, 75, layout[1]));

    f.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Yellow)),
        prompt_box,
    );
    f.render_widget(msg, layout[0]);
    f.render_widget(yes_msg, centered_rect(75, 75, action_btn_layout[0]));
    f.render_widget(no_msg, centered_rect(75, 75, action_btn_layout[1]));
}
