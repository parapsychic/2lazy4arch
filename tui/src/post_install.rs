use std::{
    fs,
    io::{self, Write},
    process::ExitCode,
};

use installer::{post_install::PostInstall, utils::AUR_QUEUE};
use nix::unistd::Uid;
use shell_iface::logger::Logger;

/// Part 2: runs as your user on the installed system. Sets up yay, installs the AUR
/// packages picked in part 1 and your own package lists. Plain CLI on purpose: sudo,
/// makepkg and yay all want the terminal for prompts.
pub fn run_post_install() -> ExitCode {
    if Uid::effective().is_root() {
        eprintln!("Run this as your normal user, not root. It uses sudo when it needs to.");
        return ExitCode::FAILURE;
    }

    let queued: Vec<String> = fs::read_to_string(AUR_QUEUE)
        .unwrap_or_default()
        .split_whitespace()
        .map(String::from)
        .collect();

    println!("\x1b[1;36m2lazy4arch, part 2\x1b[0m");
    if !queued.is_empty() {
        println!("Queued from part 1 (AUR): {}", queued.join(" "));
    }
    println!("Package lists are text files with one package name per line (see examples/ in the repo).");
    println!("Names that don't exist are skipped and listed at the end.");
    println!("Keep an eye out for sudo password prompts.\n");

    let packages = read_list("pacman package list (enter to skip): ");
    let mut aur = queued;
    aur.extend(read_list("AUR package list (enter to skip): "));

    let logger = Logger::new(true);
    let mut post_install = PostInstall::new(&logger);
    let mut ok = true;
    let mut report = |what: &str, result: anyhow::Result<()>| {
        if let Err(e) = result {
            ok = false;
            eprintln!("\x1b[1;31mInstalling {what} failed:\x1b[0m {e:#}");
        }
    };

    report("packages", post_install.install_packages(&as_strs(&packages)));
    report("AUR packages", post_install.install_aur(&as_strs(&aur)));
    if std::env::args().any(|a| a == "parapsychic-mode") {
        report("the ParaPsychic rice", post_install.misc_options());
    }

    if !post_install.skipped().is_empty() {
        println!(
            "\n\x1b[1;33mThese weren't found and were skipped:\x1b[0m {}\nCheck the names on https://archlinux.org/packages or https://aur.archlinux.org.",
            post_install.skipped().join(" ")
        );
    }
    if ok {
        println!("\n\x1b[1;32mInstallation has finished. Enjoy!\x1b[0m");
        ExitCode::SUCCESS
    } else {
        println!("\nSome steps failed, see above and shell_log.txt. Running 2lazy4arch again skips what's already installed.");
        ExitCode::FAILURE
    }
}

fn as_strs(v: &[String]) -> Vec<&str> {
    v.iter().map(String::as_str).collect()
}

/// Asks for a package list file until it's readable or skipped. Ignores blank lines and # comments.
fn read_list(prompt: &str) -> Vec<String> {
    loop {
        print!("{prompt}");
        let _ = io::stdout().flush();
        let mut path = String::new();
        if io::stdin().read_line(&mut path).unwrap_or(0) == 0 {
            return vec![];
        }
        let path = path.trim();
        if path.is_empty() {
            return vec![];
        }
        match fs::read_to_string(path) {
            Ok(content) => {
                return content
                    .lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty() && !l.starts_with('#'))
                    .map(String::from)
                    .collect()
            }
            Err(e) => println!("Can't read {path}: {e}"),
        }
    }
}
