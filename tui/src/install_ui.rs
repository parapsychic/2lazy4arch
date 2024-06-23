use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::Text,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::{
    app::{App, SubScreens},
    ui_utils::show_none_screen,
};

pub fn install_screen_ui(f: &mut Frame<'_>, chunk: Rect, app: &mut App<'_>) {
    match app.current_sub_screen {
        SubScreens::ConfirmInstallation => install_confirm_screen_ui(f, chunk, app),
        SubScreens::StartInstallation => install_start_screen_ui(f, chunk, app),
        _ => show_none_screen(f, chunk, "Install"),
    }
}

pub fn install_confirm_screen_ui(f: &mut Frame<'_>, chunk: Rect, app: &mut App<'_>) {
    let setting_text = format!(
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

    let text = Text::from(setting_text);
    let settings = Paragraph::new(text)
        .wrap(Wrap { trim: true })
        .scroll((1, 1))
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Red)),
        );
    f.render_widget(settings, chunk);
}

pub fn install_start_screen_ui(f: &mut Frame<'_>, chunk: Rect, _: &mut App<'_>) {
    let settings = Paragraph::new("Press Y to start installation? Although unlikely, I am not responsible if this installer does some damages to your system :)");
    f.render_widget(settings, chunk);
}
