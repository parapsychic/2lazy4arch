use std::io;

use chrono::Local;
use installer::{
    post_install::{DesktopEnvironment, PostInstall},
    utils::{append_to_file, write_to_file},
};
use shell_iface::logger::Logger;

pub fn run_post_install() {
    println!("Before starting, make sure you read through this:
       1. For installing extra packages from pacman, make a text file and enter the path when prompted (eg. packages.txt)
       2. For installing aur packages, do the same with another file (aur_packages.txt).
       3. If you add the wrong name for any package, the installation will fail mid-way and leave a whole lot of mess. Make sure the package name is correct.
       4. [IMPORTANT] If you are not ME, do not run the ParaPsychic-specific scripts. If you do not know what this is, then you should be good to go.
       5. The entire process will not involve a TUI like the install section because I'm having trouble figuring out doing some stuff completely in the background/unattended.
       6. Keep an eye out on the installation process.
       7. I'm assuming you are running right after the installation. If not, this might mess up your system.

       Example packages files are included in the repo.
       As always, I'm not responsible for the damages :D
       Aight, lets start.
       ");

    let mut buffer = String::new();
    let stdin = io::stdin();
    let logger = Logger::new(true);
    let mut post_install = PostInstall::new(&logger);

    buffer.clear();
    while buffer.trim() != "yes" {
        println!("Type yes to continue...");
        buffer.clear();
        stdin.read_line(&mut buffer).unwrap();
    }

    let _ = write_to_file("log.txt", &format!("Starting post install: {}", Local::now()));
    let mut packages_file = String::new();
    let mut aur_packages_file = String::new();
    {
        println!("Enter path to packages file: (Installer might fail if wrong path is entered.): ");
        packages_file.clear();
        stdin.read_line(&mut packages_file).unwrap();

        println!(
            "Enter path to AUR packages file: (Installer might fail if wrong path is entered.): "
        );
        aur_packages_file.clear();
        stdin.read_line(&mut aur_packages_file).unwrap();
    }

    match post_install.install_additionals(&packages_file, &aur_packages_file) {
        Ok(_) => {}
        Err(e) => {
            let _ = append_to_file("log.txt", &e.to_string());
            println!("Installing packages has failed. Please check the log file");
        }
    }

    println!("Pick a Desktop Environment / Window Manager");
    println!("No display manager will be installed. Please install and configure on your own.");
    println!("Or launch it from the tty like a true chad");

    println!("1. Gnome");
    println!("2. KDE Plasma");
    println!("3. Hyprland");

    buffer.clear();
    stdin.read_line(&mut buffer).unwrap();
    let index: usize = buffer.trim().parse().unwrap_or(1);
    let de: DesktopEnvironment;

    match index {
        1 => de = DesktopEnvironment::Gnome,
        2 => de = DesktopEnvironment::KDE,
        3 => de = DesktopEnvironment::Hyprland,
        _ => de = DesktopEnvironment::Gnome,
    }

    match post_install.install_desktop(de) {
        Ok(_) => {}
        Err(e) => {
            let _ = append_to_file("log.txt", &e.to_string());
            println!("Installing desktop has failed. Please check the log file");
        }
    }

    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args.contains(&String::from("parapsychic-mode")) {
        match post_install.misc_options() {
            Ok(_) => {}
            Err(e) => {
                let _ = append_to_file("log.txt", &e.to_string());
                println!("Installing misc options has failed. Please check the log file");
            }
        }
    }

    println!("Installation has finished. Enjoy!")
}
