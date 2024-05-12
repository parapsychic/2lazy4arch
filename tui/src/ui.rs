use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, Screens, SubScreens};

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
            Screens::PostInstall => Span::styled("PostInstall", Style::default().fg(Color::Yellow)),
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
            _ => Span::styled("Select some screen", Style::default().fg(Color::Red)),
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
        Screens::Essentials => todo!(),
        Screens::PostInstall => todo!(),
        Screens::Exiting => start_screen_ui(f, chunk, app),
        Screens::Installing => todo!(),
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
            "{}Setup Essentials",
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

fn filesystem_screen_ui(f: &mut Frame, chunk: Rect, app: &mut App) {
    match app.current_sub_screen {
        SubScreens::Partitioning => partitioning_subscreen_ui(f, chunk, app),
        SubScreens::MountBoot => mount_boot_subscreen_ui(f, chunk, app),
        SubScreens::MountRoot => mount_root_subscreen_ui(f, chunk, app),
        SubScreens::MountHome => mount_home_subscreen_ui(f, chunk, app),
        SubScreens::EraseEFI => erase_efi_ui(f, chunk, app),
        SubScreens::EraseHome => erase_home_ui(f, chunk, app),
        SubScreens::ConfirmPartitions => confirm_partitions_ui(f, chunk, app),
        _ => show_none_screen(f, chunk, "Filesystem"),
    }
}

fn show_none_screen(f: &mut Frame, chunk: Rect, msg: &str) {
    let msg = Paragraph::new(Line::from(msg))
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Red)),
        );
    f.render_widget(msg, centered_rect(70, 30, chunk));
}

fn partitioning_subscreen_ui(f: &mut Frame, chunk: Rect, app: &mut App) {
    let list = List::new(app.filesystem_drives_list.to_vec())
        .block(
            Block::default()
                .title("Select a disk to partition")
                .borders(Borders::ALL),
        )
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">>")
        .repeat_highlight_symbol(true);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(chunk);

    let msg =
        Paragraph::new(Line::from("[WARN]: All partitioning changes are made using another program cfdisk and are immediate and permanent."))
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL).style(Style::default().fg(Color::Red)));
    f.render_stateful_widget(list, layout[0], &mut app.list_selection);
    f.render_widget(msg, layout[1]);
}

fn mount_boot_subscreen_ui(f: &mut Frame, chunk: Rect, app: &mut App) {
    let list = List::new(app.filesystem_partitions_list.to_vec())
        .block(
            Block::default()
                .title("Select a device to mount /boot")
                .borders(Borders::ALL),
        )
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">>")
        .repeat_highlight_symbol(true);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(chunk);

    let msg =
        Paragraph::new(Line::from("The EFI Partition goes here. If you have another OS on your system, press no when asked to format /boot"))
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL).style(Style::default().fg(Color::Red)));
    f.render_stateful_widget(list, layout[0], &mut app.list_selection);
    f.render_widget(msg, layout[1]);
}

fn mount_root_subscreen_ui(f: &mut Frame, chunk: Rect, app: &mut App) {
    let list = List::new(app.filesystem_partitions_list.to_vec())
        .block(
            Block::default()
                .title("Select a device to mount /root")
                .borders(Borders::ALL),
        )
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">>")
        .repeat_highlight_symbol(true);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(chunk);

    let msg = Paragraph::new(Line::from(
        "The root partition goes here. This partition will be erased.",
    ))
    .style(Style::default().fg(Color::Yellow))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Red)),
    );
    f.render_stateful_widget(list, layout[0], &mut app.list_selection);
    f.render_widget(msg, layout[1]);
}

fn mount_home_subscreen_ui(f: &mut Frame, chunk: Rect, app: &mut App) {
    let mut list_with_extra_options = app.filesystem_partitions_list.to_vec();
    list_with_extra_options.push("No Separate Home Partition".to_string());
    let list = List::new(list_with_extra_options)
        .block(
            Block::default()
                .title("Select a device to mount /home")
                .borders(Borders::ALL),
        )
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">>")
        .repeat_highlight_symbol(true);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(chunk);

    let msg = Paragraph::new(Line::from("The home partition goes here..."))
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Red)),
        );
    f.render_stateful_widget(list, layout[0], &mut app.list_selection);
    f.render_widget(msg, layout[1]);
}

fn erase_efi_ui(f: &mut Frame, chunk: Rect, app: &mut App) {
    let msg = Paragraph::new(Line::from(format!(
        "Do you want to format the EFI partition {}",
        app.filesystem.get_boot().unwrap()
    )))
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

fn erase_home_ui(f: &mut Frame, chunk: Rect, app: &mut App) {
    let msg = Paragraph::new(Line::from(format!(
        "Do you want to format the home partition {}",
        app.filesystem.get_home().unwrap_or_default()
    )))
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

fn confirm_partitions_ui(f: &mut Frame, chunk: Rect, app: &mut App) {
    let msg = Paragraph::new(Line::from("Are you happy with this layout?"))
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center)
        .block(Block::default());

    let boot_partition = Paragraph::new(Line::from(format!(
        "/boot: {} | erase: {}",
        app.filesystem.get_boot().unwrap(),
        app.filesystem.format_boot
    )))
    .style(Style::default().fg(Color::Green))
    .alignment(Alignment::Center)
    .wrap(Wrap::default())
    .block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Green)),
    );

    let root_partition = Paragraph::new(Line::from(format!(
        "/root: {} | erase: true",
        app.filesystem.get_root().unwrap()
    )))
    .style(Style::default().fg(Color::Green))
    .alignment(Alignment::Center)
    .wrap(Wrap::default())
    .block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Green)),
    );

    let home_partition_msg = match app.filesystem.get_home() {
        Some(x) => format!("/home: {} | erase: {}", x, app.filesystem.format_home),
        None => String::from("No separate home partition selected"),
    };

    let home_partition = Paragraph::new(Line::from(home_partition_msg))
        .style(Style::default().fg(Color::Green))
        .alignment(Alignment::Center)
        .wrap(Wrap::default())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Green)),
        );

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

    let prompt_box = centered_rect(70, 50, chunk);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Min(12),
            Constraint::Min(3),
            Constraint::Min(1),
        ])
        .split(prompt_box);

    let partition_column_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Min(3),
            Constraint::Min(3),
            Constraint::Min(1),
        ])
        .split(layout[1]);

    let action_btn_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(centered_rect(75, 75, layout[2]));

    f.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Yellow)),
        prompt_box,
    );
    f.render_widget(msg, layout[0]);
    f.render_widget(
        boot_partition,
        centered_rect(75, 75, partition_column_layout[0]),
    );
    f.render_widget(
        root_partition,
        centered_rect(75, 75, partition_column_layout[1]),
    );
    f.render_widget(
        home_partition,
        centered_rect(75, 75, partition_column_layout[2]),
    );
    f.render_widget(yes_msg, action_btn_layout[0]);
    f.render_widget(no_msg, action_btn_layout[1]);
}

fn pacman_setup_ui(f: &mut Frame, chunk: Rect, app: &mut App) {
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

/// helper function to create a centered rect using up certain percentage of the available rect `r`
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    // Cut the given rectangle into three vertical pieces
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    // Then cut the middle vertical piece into three width-wise pieces
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1] // Return the middle chunk
}
