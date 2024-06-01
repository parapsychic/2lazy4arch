use crate::app::{App, Screens, SubScreens};
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use installer::utils::is_valid_mount_point;
use std::rc::Rc;

pub fn filesystem_screen_events(app: &mut App, key: KeyEvent) {
    match app.current_sub_screen {
        SubScreens::Partitioning => partitioning_events(app, key),
        SubScreens::MountBoot => mount_boot_events(app, key),
        SubScreens::MountHome => mount_home_events(app, key),
        SubScreens::MountRoot => mount_root_events(app, key),
        SubScreens::MountExtraPartition => mount_extra_partitions(app, key),
        SubScreens::MountExtraPartition__Insert => insert_extra_partitions(app, key),
        SubScreens::ConfirmPartitions => confirm_partitions_events(app, key),
        SubScreens::EraseEFI => erase_efi_events(app, key),
        SubScreens::EraseHome => erase_home_events(app, key),
        SubScreens::None => match key.code {
            _ => app.current_sub_screen = SubScreens::Partitioning,
        },
        _ => app.current_sub_screen = SubScreens::Partitioning,
    }
}

fn partitioning_events(app: &mut App, key: KeyEvent) {
    let total_list_item = app.filesystem_drives_list.len();
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            match app.list_selection.selected() {
                Some(x) => {
                    let index: usize;
                    if x == 0 {
                        index = total_list_item - 1;
                    } else {
                        index = x - 1;
                    }
                    app.list_selection.select(Some(index));
                }
                None => {
                    app.list_selection.select(Some(0));
                }
            };
        }
        KeyCode::Down | KeyCode::Char('j') => {
            match app.list_selection.selected() {
                Some(x) => {
                    app.list_selection.select(Some((x + 1) % total_list_item));
                }
                None => {
                    app.list_selection.select(Some(0));
                }
            };
        }
        KeyCode::Enter => {
            let selection = app.list_selection.selected().unwrap();
            if selection == total_list_item - 1 {
                let list = app
                    .filesystem
                    .lsblk()
                    .unwrap()
                    .iter()
                    .map(|x| {
                        x.children
                            .as_ref()
                            .unwrap()
                            .iter()
                            .map(|x| {
                                let mount_points = x
                                    .mountpoints
                                    .clone()
                                    .unwrap_or_default()
                                    .iter()
                                    .map(|x| x.clone().unwrap_or_default())
                                    .collect::<Vec<String>>()
                                    .join(", ");
                                format!(
                                    "{} {} {}",
                                    x.name,
                                    x.size.clone().unwrap_or_default(),
                                    mount_points
                                )
                            })
                            .collect::<Vec<String>>()
                    })
                    .collect::<Vec<Vec<String>>>()
                    .into_iter()
                    .flatten()
                    .collect::<Vec<String>>();

                app.filesystem_partitions_list = Rc::new(list);
                app.current_sub_screen = SubScreens::MountBoot;
                app.list_selection.select(Some(0));
            } else {
                let disk = format!("/dev/{}", &app.filesystem_drives_list.clone()[selection]);
                let _ = app.filesystem.partition_disks(&disk);
                app.redraw_next_frame = true;
            }
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.current_screen = Screens::StartScreen;
            app.list_selection.select(Some(0));
        }
        _ => {}
    }
}

fn mount_boot_events(app: &mut App, key: KeyEvent) {
    let total_list_item = app.filesystem_partitions_list.len();
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            match app.list_selection.selected() {
                Some(x) => {
                    let index: usize;
                    if x == 0 {
                        index = total_list_item - 1;
                    } else {
                        index = x - 1;
                    }
                    app.list_selection.select(Some(index));
                }
                None => {
                    app.list_selection.select(Some(0));
                }
            };
        }
        KeyCode::Down | KeyCode::Char('j') => {
            match app.list_selection.selected() {
                Some(x) => {
                    app.list_selection.select(Some((x + 1) % total_list_item));
                }
                None => {
                    app.list_selection.select(Some(0));
                }
            };
        }
        KeyCode::Enter => {
            let selected_index = app.list_selection.selected().unwrap();
            let selected = &app.filesystem_partitions_list.clone()[selected_index];
            let selected = selected.split(' ').collect::<Vec<&str>>();

            let disk = format!("/dev/{}", selected.first().unwrap());

            match app.filesystem.set_boot(&disk) {
                Ok(_) => {}
                Err(x) => {
                    app.error_console = x.to_string();
                    return;
                }
            }
            app.list_selection.select(Some(0));
            app.current_sub_screen = SubScreens::MountRoot;
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.current_screen = Screens::Filesystem;
            app.current_sub_screen = SubScreens::Partitioning;
            app.list_selection.select(Some(0));
        }
        _ => {}
    }
}

fn mount_root_events(app: &mut App, key: KeyEvent) {
    let total_list_item = app.filesystem_partitions_list.len();
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            match app.list_selection.selected() {
                Some(x) => {
                    let index: usize;
                    if x == 0 {
                        index = total_list_item - 1;
                    } else {
                        index = x - 1;
                    }
                    app.list_selection.select(Some(index));
                }
                None => {
                    app.list_selection.select(Some(0));
                }
            };
        }
        KeyCode::Down | KeyCode::Char('j') => {
            match app.list_selection.selected() {
                Some(x) => {
                    app.list_selection.select(Some((x + 1) % total_list_item));
                }
                None => {
                    app.list_selection.select(Some(0));
                }
            };
        }
        KeyCode::Enter => {
            let selected_index = app.list_selection.selected().unwrap();
            let selected = &app.filesystem_partitions_list.clone()[selected_index];
            let selected = selected.split(' ').collect::<Vec<&str>>();

            let disk = format!("/dev/{}", selected.first().unwrap());

            match app.filesystem.set_root(&disk) {
                Ok(_) => {}
                Err(x) => {
                    app.error_console = x.to_string();
                    return;
                }
            }

            app.list_selection.select(Some(0));
            app.current_sub_screen = SubScreens::MountHome;
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.current_screen = Screens::Filesystem;
            app.list_selection.select(Some(0));
            app.current_sub_screen = SubScreens::MountBoot;
            app.list_selection.select(Some(0));
        }
        _ => {}
    }
}

fn mount_home_events(app: &mut App, key: KeyEvent) {
    // adding + 1 for "No separate home partition"
    let total_list_item = app.filesystem_partitions_list.len() + 1;
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            match app.list_selection.selected() {
                Some(x) => {
                    let index: usize;
                    if x == 0 {
                        index = total_list_item - 1;
                    } else {
                        index = x - 1;
                    }
                    app.list_selection.select(Some(index));
                }
                None => {
                    app.list_selection.select(Some(0));
                }
            };
        }
        KeyCode::Down | KeyCode::Char('j') => {
            match app.list_selection.selected() {
                Some(x) => {
                    app.list_selection.select(Some((x + 1) % total_list_item));
                }
                None => {
                    app.list_selection.select(Some(0));
                }
            };
        }
        KeyCode::Enter => {
            let selected_index = app.list_selection.selected().unwrap();
            if selected_index == app.filesystem_partitions_list.len() {
                app.filesystem.set_home(None).unwrap();
                app.current_sub_screen = SubScreens::EraseEFI;
            } else {
                let selected = &app.filesystem_partitions_list.clone()[selected_index];
                let selected = selected.split(' ').collect::<Vec<&str>>();

                let disk = format!("/dev/{}", selected.first().unwrap());

                match app.filesystem.set_home(Some(&disk)) {
                    Ok(_) => {}
                    Err(x) => {
                        app.error_console = x.to_string();
                        return;
                    }
                }
                app.current_sub_screen = SubScreens::EraseEFI;
            }
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.current_screen = Screens::Filesystem;
            app.current_sub_screen = SubScreens::MountRoot;
            app.list_selection.select(Some(0));
        }
        _ => {}
    }
}

fn erase_efi_events(app: &mut App, key: KeyEvent) {
    let next_screen = match app.filesystem.get_home() {
        Some(_) => SubScreens::EraseHome,
        None => SubScreens::ConfirmPartitions,
    };

    match key.code {
        KeyCode::Char('y') => {
            app.filesystem.format_boot = true;
            app.current_screen = Screens::Filesystem;
            app.current_sub_screen = next_screen;
        }
        KeyCode::Char('n') => {
            app.filesystem.format_boot = false;
            app.current_screen = Screens::Filesystem;
            app.current_sub_screen = next_screen;
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.current_screen = Screens::Filesystem;
            app.current_sub_screen = SubScreens::MountHome;
            app.list_selection.select(Some(0));
        }
        _ => {}
    }
}

fn erase_home_events(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('y') => {
            app.filesystem.format_home = true;
            app.current_screen = Screens::Filesystem;
            app.current_sub_screen = SubScreens::ConfirmPartitions;
        }
        KeyCode::Char('n') => {
            app.filesystem.format_home = false;
            app.current_screen = Screens::Filesystem;
            app.current_sub_screen = SubScreens::ConfirmPartitions;
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.current_screen = Screens::Filesystem;
            app.current_sub_screen = SubScreens::MountHome;
            app.list_selection.select(Some(0));
        }
        _ => {}
    }
}

fn confirm_partitions_events(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('y') => {
            app.current_screen = Screens::Filesystem;
            app.list_selection.select(Some(0));
            app.current_sub_screen = SubScreens::MountExtraPartition;
        }
        KeyCode::Char('n') | KeyCode::Char('q') => {
            app.filesystem.clear_mounts();
            app.current_screen = Screens::Filesystem;
            app.list_selection.select(Some(0));
            app.current_sub_screen = SubScreens::MountBoot;
        }
        _ => {}
    }
}

fn mount_extra_partitions(app: &mut App, key: KeyEvent) {
    // adding + 1 for "No separate home partition"
    let total_list_item = app.filesystem.partitions.iter().len() + 2;
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            match app.list_selection.selected() {
                Some(x) => {
                    let index: usize;
                    if x == 0 {
                        index = total_list_item - 1;
                    } else {
                        index = x - 1;
                    }
                    app.list_selection.select(Some(index));
                }
                None => {
                    app.list_selection.select(Some(0));
                }
            };
        }
        KeyCode::Down | KeyCode::Char('j') => {
            match app.list_selection.selected() {
                Some(x) => {
                    app.list_selection.select(Some((x + 1) % total_list_item));
                }
                None => {
                    app.list_selection.select(Some(0));
                }
            };
        }
        KeyCode::Enter => {
            let selected_index = app.list_selection.selected().unwrap();

            // Add new partition
            if selected_index == app.filesystem.partitions.iter().len() {
                app.list_selection.select(Some(0));
                app.current_sub_screen = SubScreens::MountExtraPartition__Insert;
            }
            // Continue after completion
            else if selected_index == app.filesystem.partitions.iter().len() + 1 {
                app.filesystem_setup_complete = true;
                app.tab_selection = 0;
                app.list_selection.select(Some(0));
                app.current_screen = Screens::StartScreen;
                app.current_sub_screen = SubScreens::None;
            }
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.current_screen = Screens::Filesystem;
            app.current_sub_screen = SubScreens::EraseHome;
            app.list_selection.select(Some(0));
        }
        _ => {}
    }
}

fn insert_extra_partitions(app: &mut App, key: KeyEvent) {
    let total_list_item = app.filesystem_partitions_list.len();
    match key.code {
        KeyCode::Up | KeyCode::Char('k') if app.tab_selection == 1 => {
            match app.list_selection.selected() {
                Some(x) => {
                    let index: usize;
                    if x == 0 {
                        index = total_list_item - 1;
                    } else {
                        index = x - 1;
                    }
                    app.list_selection.select(Some(index));
                }
                None => {
                    app.list_selection.select(Some(0));
                }
            };
        }
        KeyCode::Down | KeyCode::Char('j') if app.tab_selection == 1 => {
            match app.list_selection.selected() {
                Some(x) => {
                    app.list_selection.select(Some((x + 1) % total_list_item));
                }
                None => {
                    app.list_selection.select(Some(0));
                }
            };
        }
        KeyCode::Char(x) if app.tab_selection == 0 => {
            app.text_controller.push(x);
        }
        KeyCode::Backspace if app.tab_selection == 0 => {
            app.text_controller.pop();
        }
        KeyCode::Tab => {
            app.tab_selection = (app.tab_selection + 1) % 2;
        }
        KeyCode::Enter => {
            if app.tab_selection == 0 {
                app.tab_selection = 1;
            } else {
                if !is_valid_mount_point(&app.text_controller)
                {
                    app.error_console = "Mount point name contains invalid characters".to_string();
                }

                // if user tries to type eg, /WindowsShared
                // then we have to internally mount it at /mnt/WindowsShared
                // so, it makes sense to remove the /
                if app.text_controller.starts_with("/"){
                    app.text_controller.remove(0);
                }

                let selected_index = app.list_selection.selected().unwrap();
                let selected = &app.filesystem_partitions_list.clone()[selected_index];
                let selected = selected.split(' ').collect::<Vec<&str>>();
                match app.filesystem.set_mount_points(
                    &format!("/dev/{}",selected.first().unwrap()),
                    &app.text_controller.clone(),
                ) {
                    Ok(_) => {}
                    Err(e) => {
                        app.error_console = e.to_string();
                    }
                };
                app.text_controller.clear();
                app.tab_selection = 0;
                app.list_selection.select(Some(0));
                app.current_sub_screen = SubScreens::MountExtraPartition;
            }
        }
        KeyCode::Esc => {
            app.current_sub_screen = SubScreens::MountExtraPartition;
            app.list_selection.select(Some(0));
        }
        _ => {}
    }
}
