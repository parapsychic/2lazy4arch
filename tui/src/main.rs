use std::{io, process};

use installer::filesystem_tasks::Filesystem;
use shell_iface::{logger::Logger, Shell};

fn main() {
    let mut buffer = String::new();
    let stdin = io::stdin();

    let logger = Logger::new(true);

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

        if buffer.trim() == "q" {
            break;
        }

        let index: usize = buffer.trim().parse().unwrap();

        let disk = format!("/dev/{}", disks[index]);
        filesystem.partition_disks(&disk).unwrap();
    }

    // Setting /boot partition
    loop {
        println!("Current disk layout:\n {}\n", filesystem.lsblk().unwrap());
        println!("Select a partition to be /boot");
        buffer.clear();
        stdin.read_line(&mut buffer).unwrap();
        print!("/boot: ");
        match filesystem.set_boot(&buffer.clone()) {
            Ok(_) => {
                break;
            }
            Err(e) => {
                eprintln!("{}", e);
                println!("Retry? (Y/n) \n(n to exit installer)");
                buffer.clear();
                stdin.read_line(&mut buffer).unwrap();
                if buffer == "n" {
                    process::exit(-1);
                }
            }
        }
    }

    // Setting / partition
    loop {
        println!("Current disk layout:\n {}\n", filesystem.lsblk().unwrap());
        println!("Select a partition to be /boot");
        buffer.clear();
        stdin.read_line(&mut buffer).unwrap();
        print!("/boot: ");
        match filesystem.set_boot(&buffer.clone()) {
            Ok(_) => {
                break;
            }
            Err(e) => {
                eprintln!("{}", e);
                println!("Retry? (Y/n) \n(n to exit installer)");
                buffer.clear();
                stdin.read_line(&mut buffer).unwrap();
                if buffer == "n" {
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
        if buffer.to_lowercase() == "n" {
            break;
        }

        println!("Current disk layout:\n {}\n", filesystem.lsblk().unwrap());
        println!("Select a partition to be /home");
        buffer.clear();
        stdin.read_line(&mut buffer).unwrap();
        print!("/boot: ");
        match filesystem.set_boot(&buffer.clone()) {
            Ok(_) => {
                break;
            }
            Err(e) => {
                eprintln!("{}", e);
                println!("Retry? (Y/n) \n(n to exit installer)");
                buffer.clear();
                stdin.read_line(&mut buffer).unwrap();
                if buffer == "n" {
                    process::exit(-1);
                }
            }
        }
    }

    // Format partitions
    {
        println!("Do you want to format /boot? (y/N)");
        println!("If you have any other OS on the same EFI partition, press n");
        buffer.clear();
        stdin.read_line(&mut buffer).unwrap();

        let mut erase_efi = false;
        if buffer.to_lowercase() == "y" {
            erase_efi = true;
        }
        filesystem.format_partitions(erase_efi).unwrap();
    }


}
