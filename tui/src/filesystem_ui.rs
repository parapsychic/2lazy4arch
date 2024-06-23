use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, Paragraph, Wrap},
    Frame,
};

use crate::{app::{App, SubScreens}, ui_utils::{show_none_screen, centered_rect}};

pub fn filesystem_screen_ui(f: &mut Frame, chunk: Rect, app: &mut App) {
    match app.current_sub_screen {
        SubScreens::Partitioning => partitioning_subscreen_ui(f, chunk, app),
        SubScreens::MountBoot => mount_boot_subscreen_ui(f, chunk, app),
        SubScreens::MountRoot => mount_root_subscreen_ui(f, chunk, app),
        SubScreens::MountHome => mount_home_subscreen_ui(f, chunk, app),
        SubScreens::EraseEFI => erase_efi_ui(f, chunk, app),
        SubScreens::EraseHome => erase_home_ui(f, chunk, app),
        SubScreens::MountExtraPartition => mount_extra_partitions(f, chunk, app),
        SubScreens::MountExtraPartition__Insert => mount_extra_partitions_insert(f, chunk, app),
        SubScreens::ConfirmPartitions => confirm_partitions_ui(f, chunk, app),
        _ => show_none_screen(f, chunk, "Filesystem"),
    }
}

pub fn partitioning_subscreen_ui(f: &mut Frame, chunk: Rect, app: &mut App) {
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

pub fn mount_boot_subscreen_ui(f: &mut Frame, chunk: Rect, app: &mut App) {
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

pub fn mount_root_subscreen_ui(f: &mut Frame, chunk: Rect, app: &mut App) {
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

pub fn mount_home_subscreen_ui(f: &mut Frame, chunk: Rect, app: &mut App) {
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

pub fn erase_efi_ui(f: &mut Frame, chunk: Rect, app: &mut App) {
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

pub fn erase_home_ui(f: &mut Frame, chunk: Rect, app: &mut App) {
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

pub fn mount_extra_partitions(f: &mut Frame, chunk: Rect, app: &mut App) {
    let mut list_with_extra_options = app.filesystem.partitions.iter().map(|x| {
       format!("{} : {}", x.0, x.1) 
    }).collect::<Vec<String>>();
    list_with_extra_options.push("Add new partition".to_string());
    list_with_extra_options.push("Continue".to_string());

    let list = List::new(list_with_extra_options)
        .block(
            Block::default()
                .title("Mount extra partitions")
                .borders(Borders::ALL),
        )
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">>")
        .repeat_highlight_symbol(true);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(chunk);

    let msg = Paragraph::new(Line::from("These partitions will be mount as is."))
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Red)),
        );
    f.render_stateful_widget(list, layout[0], &mut app.list_selection);
    f.render_widget(msg, layout[1]);
}

pub fn mount_extra_partitions_insert(f: &mut Frame, chunk: Rect, app: &mut App) {
    let mount_point = Paragraph::new(Line::from(app.text_controller.clone()))
        .alignment(Alignment::Left)
        .block(
            Block::default()
                .title("Mount point")
                .borders(Borders::ALL)
                .style(Style::default().fg( if app.tab_selection == 0 { Color::Yellow } else { Color::White } )),
        );

    let list = List::new(app.filesystem_partitions_list.to_vec())
        .block(
            Block::default()
                .title("Partition")
                .borders(Borders::ALL)
                .style(Style::default().fg( if app.tab_selection == 1 { Color::Yellow } else { Color::White } ))
        )
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">>")
        .repeat_highlight_symbol(true);

    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Min(1)])
        .split(centered_rect(70, 30, chunk));

    f.render_stateful_widget(list, layout[1], &mut app.list_selection);
    f.render_widget(mount_point, layout[0]);
}

pub fn confirm_partitions_ui(f: &mut Frame, chunk: Rect, app: &mut App) {
    let msg = Paragraph::new(Line::from("Continue with this layout for system disks (extra partitions not shown)?"))
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
