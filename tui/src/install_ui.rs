use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::{
    app::App,
    ui_utils::centered_rect,
};

pub fn install_screen_ui(f: &mut Frame<'_>, chunk: Rect, app: &mut App<'_>) {
    let setting_text = String::new();
    format!(
        "
Filesystem:
/boot: {} | erase: {}
/root: {} | erase: true
/home: {} | erase: {}

---
Pacman:
Mirror country: {}

---
Misc Settings:
bootloader: {}
superuser utility: {}
swap space: {} GB
locale: {} {}
timezone: {}

hostname: {}
username: {}

[Y] to install
        ",
        app.filesystem.get_boot().unwrap(),
        app.filesystem.format_boot,
        app.filesystem.get_root().unwrap(),
        match app.filesystem.get_home() {
            Some(x) => x,
            None => "No home partition selected".to_string(),
        },
        match app.filesystem.get_home() {
            Some(_) => app.filesystem.format_home.to_string(),
            None => "no".to_string(),
        },
        app.selected_reflector_country,
        match app.essentials.bootloader {
            installer::essentials::Bootloader::Grub => "Grub",
            installer::essentials::Bootloader::SystemDBoot => "systemd boot",
        },
        match app.essentials.super_user_utility {
            installer::essentials::SuperUserUtility::Sudo => "sudo",
            installer::essentials::SuperUserUtility::Doas => "doas",
        },
        app.swap_size,
        app.selected_locale,
        app.selected_encoding,
        app.selected_timezone,
        app.hostname,
        app.username,
    );

    let settings = Paragraph::new(Line::from(setting_text))
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Red)),
        );
    f.render_widget(settings, centered_rect(70, 30, chunk));
}
