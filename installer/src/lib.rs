use std::path::PathBuf;

use anyhow::Result;
use base_installer::BaseInstaller;
use essentials::Essentials;
use filesystem_tasks::Filesystem;
use pacman::Pacman;
use utils::{write_to_file, INSTALL_SUCCESS_FLAG};

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
    selected_reflector_country: &str,
    selected_timezone: &str,
    selected_locale: &str,
    selected_encoding: &str,
    swap_size: usize,
    username: &str,
    password: &str,
    root_password: &str,
    hostname: &str,
) {
    println!("Runnning reflector");
    pacman.run_reflector(&selected_reflector_country).unwrap();

    println!("Setting up filesystem");
    match install_filesystem(filesystem) {
        Ok(_) => {}
        Err(e) => {
            filesystem.try_unmount();
            println!("Installing filesystem failed");
            let _ = write_to_file("log.txt", &e.to_string());
            return;
        }
    }

    println!("Doing a base install");
    match install_base(base_installer) {
        Ok(_) => {}
        Err(e) => {
            filesystem.try_unmount();
            println!("Installing base failed");
            let _ = write_to_file("log.txt", &e.to_string());
            return;
        }
    }

    println!("Setting up the essentials");
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
        selected_reflector_country,
    ) {
        Ok(_) => {}
        Err(e) => {
            filesystem.try_unmount();
            println!("Installing essentials failed");
            let _ = write_to_file("log.txt", &e.to_string());
            return;
        }
    }

    match write_to_file(INSTALL_SUCCESS_FLAG, "true") {
        Ok(_) => {}
        Err(e) => {
            filesystem.try_unmount();
            eprintln!("Setting success flag failed");
            let _ = write_to_file("log.txt", &e.to_string());
            return;
        }
    };

    // Construct the destination path
    let destination_path = PathBuf::from(format!("/mnt/home/{}/installer", username));
    // Get the name of the executable from std::env::args
    let executable_name = match std::env::args().next() {
        Some(x) => x,
        None => {
            eprintln!("Failed to get the executable name from arguments.");
            let _ = write_to_file(
                "log.txt",
                "Failed to get the executable name from arguments.",
            );
            return;
        }
    };

    // Perform the file copy operation
    if let Err(e) = std::fs::copy(executable_name, &destination_path) {
        eprintln!("Failed to copy the installer: {}", e);
        eprintln!(
            "Please copy the file manually to {}",
            destination_path.display()
        );
        let _ = write_to_file(
            "log.txt",
            &format!("Failed to copy the executable. {}", &e.to_string()),
        );
        return;
    }

    println!(
        "Successfully copied the executable to {}. 
        \nPlease run the installer after rebooting to the installed system.
        \nInstaller completed successfully.",
        destination_path.display()
    );
    let _ = write_to_file(
        "log.txt",
        "Installer completed successfully."
    );
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
    selected_timezone: &str,
    selected_locale: &str,
    selected_encoding: &str,
    swap_size: usize,
    username: &str,
    password: &str,
    root_password: &str,
    hostname: &str,
    selected_reflector_country: &str,
) -> Result<()> {
    {
        println!("Entering chroot");
        essentials.chroot()?;

        {
            println!("Initializing swap");
            essentials.initialize_swap(swap_size)?;
        }

        {
            println!("Setting timezones");
            essentials.set_timezones(&selected_timezone)?;
        }

        {
            println!("Setting locale");
            essentials.gen_locale(&selected_locale, &selected_encoding)?;
        }

        {
            println!("Setting hostname");
            essentials.set_hostname(&hostname)?;
        }

        {
            println!("Setting up root");
            essentials.set_password("root", &root_password)?;
        }

        {
            println!("Setting up packages");
            essentials.install_essentials(selected_reflector_country, None)?;
        }

        {
            println!("Setting up bootloader");
            essentials.install_bootloader()?;
            essentials.mkinitcpio()?;
        }

        {
            println!("Setting up user");
            essentials.user_management(&username, &password)?;
        }

        println!("Completed, exiting installer");
    }

    Ok(())
}
