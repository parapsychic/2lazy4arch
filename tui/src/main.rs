use std::{io, process};

use installer::{filesystem_tasks::Filesystem, pacman::Pacman, base_installer::BaseInstaller};
use shell_iface::{logger::Logger, Shell};

// This file will be rewritten to be a TUI.
// Currently, I'm testing using the main.
fn main() {
    let mut buffer = String::new();
    let stdin = io::stdin();

    let logger = Logger::new(true);

    /* LIVE PRESETUP */
    let mut pacman = Pacman::new(&logger);
    // run reflector
    {
        println!("Enter your country for pacman mirrorlist: ");
        buffer.clear();
        stdin.read_line(&mut buffer).unwrap();
        pacman.run_reflector(&buffer.clone()).unwrap();
    }



    /* FILESYSTEM TASKS */
    /* Creating partitions
     * Formatting partitions
     * Mounting partitions */

    let mut filesystem = Filesystem::new(&logger);

    // Creating partitions with cfdisk
    loop {
        println!("Current disk layout:\n {}\n", filesystem.lsblk().unwrap());

        let disks = filesystem.get_disks().unwrap();
        let disks = disks.trim().split('\n').collect::<Vec<&str>>();

        println!("Select a disk to partition (0 indexed) or q to quit");
        println!("{:#?}", disks);
        buffer.clear();
        stdin.read_line(&mut buffer).unwrap();

        if buffer.trim().to_lowercase() == "q" {
            break;
        }

        let index: usize = buffer.trim().parse().unwrap();

        let disk = format!("/dev/{}", disks[index]);
        filesystem.partition_disks(&disk).unwrap();
    }

    loop {
        // Setting /boot partition
        loop {
            println!("Current disk layout:\n {}\n", filesystem.lsblk().unwrap());
            println!("Select a partition to be /boot");
            print!("/boot: ");
            buffer.clear();
            stdin.read_line(&mut buffer).unwrap();
            match filesystem.set_boot(&buffer.clone()) {
                Ok(_) => {
                    break;
                }
                Err(e) => {
                    eprintln!("{}", e);
                    println!("Retry? (Y/n) \n(n to exit installer)");
                    buffer.clear();
                    stdin.read_line(&mut buffer).unwrap();
                    if buffer.trim().to_lowercase() == "n" {
                        process::exit(-1);
                    }
                }
            }
        }

        // Setting / partition
        loop {
            println!("Current disk layout:\n {}\n", filesystem.lsblk().unwrap());
            println!("Select a partition to be /root");
            print!("/root: ");
            buffer.clear();
            stdin.read_line(&mut buffer).unwrap();
            match filesystem.set_root(&buffer.clone()) {
                Ok(_) => {
                    break;
                }
                Err(e) => {
                    eprintln!("{}", e);
                    println!("Retry? (Y/n) \n(n to exit installer)");
                    buffer.clear();
                    stdin.read_line(&mut buffer).unwrap();
                    if buffer.trim().to_lowercase() == "n" {
                        process::exit(-1);
                    }
                }
            }
        }

        // Setting /home partition
        loop {
            println!("Do you want a separate /home partition? (Y/n)");
            buffer.clear();
            stdin.read_line(&mut buffer).unwrap();
            if buffer.trim().to_lowercase() == "n" {
                filesystem.set_home(None).unwrap();
                break;
            }

            println!("Current disk layout:\n {}\n", filesystem.lsblk().unwrap());
            println!("Select a partition to be /home");
            print!("/home: ");
            buffer.clear();
            stdin.read_line(&mut buffer).unwrap();
            match filesystem.set_home(Some(&buffer.clone())) {
                Ok(_) => {
                    break;
                }
                Err(e) => {
                    eprintln!("{}", e);
                    println!("Retry? (Y/n) \n(n to exit installer)");
                    buffer.clear();
                    stdin.read_line(&mut buffer).unwrap();
                    if buffer.trim().to_lowercase() == "n" {
                        process::exit(-1);
                    }
                }
            }
        }
        
        // prompt user to confirm
        println!(
            "/boot: {}\n/root: {}\n/home: {}",
            filesystem
                .get_boot()
                .unwrap_or_else(|| String::from("None")),
            filesystem
                .get_root()
                .unwrap_or_else(|| String::from("None")),
            filesystem
                .get_home()
                .unwrap_or_else(|| String::from("None"))
        );
        println!("Are you happy with this layout? (y/N)");
        buffer.clear();
        stdin.read_line(&mut buffer).unwrap();

        if buffer.trim().to_lowercase() == "y" {
            break;
        }
    }

    // Format partitions
    {
        println!("Do you want to format /boot? (y/N)");
        println!("If you have any other OS on the same EFI partition, press n");
        buffer.clear();
        stdin.read_line(&mut buffer).unwrap();

        let mut erase_efi = false;
        if buffer.trim().to_lowercase() == "y" {
            erase_efi = true;
        }
        println!("Formatting partitions: ");
        filesystem.format_partitions(erase_efi).unwrap();
    }

    {
        println!("Mounting partitions...");
        filesystem.mount_partitions().unwrap();
    }

    
    /* BASE INSTALLATION */
    let mut base_installer = BaseInstaller::new(&logger); 

    // Install base packages
    {
        base_installer.base_packages_install().unwrap();
    }

}
