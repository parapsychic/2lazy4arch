use anyhow::Result;
use base_installer::BaseInstaller;
use essentials::Essentials;
use filesystem_tasks::Filesystem;
use pacman::Pacman;

pub mod base_installer;
pub mod essentials;
pub mod filesystem_tasks;
pub mod pacman;
pub mod partition_table;
pub mod post_install;
pub mod utils;

pub fn install(
    filesystem: &mut Filesystem,
    base_installer: &mut BaseInstaller,
    essentials: &mut Essentials,
    pacman: &mut Pacman,
    selected_reflector_country: String,
    selected_timezone: String,
    selected_locale: String,
    selected_encoding: String,
    swap_size: usize,
    username: String,
    password: String,
    root_password: String,
    hostname: String,
) {
    pacman.run_reflector(&selected_reflector_country).unwrap();

    match install_filesystem(filesystem) {
        Ok(_) => {}
        Err(_) => {
            filesystem.try_unmount();
        }
    }

    match install_base(base_installer) {
        Ok(_) => {}
        Err(_) => {
            filesystem.try_unmount();
        }
    }

    match install_essentials(
        essentials,
        selected_timezone,
        selected_locale,
        selected_encoding,
        swap_size,
        username,
        password,
        root_password,
        hostname,
    ) {
        Ok(_) => {}
        Err(_) => {
            filesystem.try_unmount();
        }
    }
}

fn install_filesystem(filesystem: &mut Filesystem) -> Result<()> {
    // Format partitions
    {
        filesystem.format_partitions()?;
    }

    {
        filesystem.mount_partitions()?;
    }

    Ok(())
}

fn install_base(base_installer: &mut BaseInstaller) -> Result<()> {
    // Install base packages
    {
        base_installer.base_packages_install()?;
    }

    // generate fstab
    {
        base_installer.genfstab()?;
    }

    Ok(())
}

fn install_essentials(
    essentials: &mut Essentials,
    selected_timezone: String,
    selected_locale: String,
    selected_encoding: String,
    swap_size: usize,
    username: String,
    password: String,
    root_password: String,
    hostname: String,
) -> Result<()> {
    {
        essentials.initialize_swap(swap_size)?;
    }

    {
        essentials.set_timezones(&selected_timezone)?;
    }

    {
        essentials.gen_locale(&selected_locale, &selected_encoding)?;
    }

    {
        essentials.set_hostname(&hostname)?;
    }

    {
        essentials.set_password("root", &root_password)?;
    }

    {
        essentials.install_essentials(None)?;
    }

    {
        essentials.install_bootloader()?;
    }

    {
        essentials.user_management(&username, &password)?;
    }

    Ok(())
}
