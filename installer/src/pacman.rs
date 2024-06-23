use anyhow::{anyhow, Result};
use nix::unistd::Uid;
use shell_iface::{logger::Logger, Shell};

enum PackageManager{
    Pacman,
    Yay
}

pub struct Pacman<'a> {
    shell: Shell<'a>,
    /// specifies whether this is running on the live/installer environment
    /// or the acutal installed machine
    is_non_root: bool,
    program: PackageManager,
}

impl<'a> Pacman<'a> {
    pub fn new<'b>(logger: &'b Logger) -> Pacman<'b> {
        let is_non_root = !Uid::effective().is_root();

        let shell = Shell::new("Pacman", logger);
        Pacman {
            shell,
            is_non_root,
            program: PackageManager::Pacman
        }
    }

    pub fn yay(&mut self) -> &mut Self {
        self.program = PackageManager::Yay;
        return self;
    }

    pub fn pacman(&mut self) -> &mut Self {
        self.program = PackageManager::Pacman;
        return self;
    }

    pub fn update_mirrors(&mut self) -> Result<()> {
        let status = if self.is_non_root {
            self.shell.run_and_wait_with_args(
                "su",
                &format!("-c \"{} -Syyy --noconfirm\"", self.get_program()),
            )?
        } else {
            if let PackageManager::Yay = self.program {
                self.shell.log("ERROR: Called YAY as root.");
                return Err(anyhow!("PACMAN: Called yay as root"));
            }

            self.shell
                .run_and_wait_with_args("pacman", "-Syyy --noconfirm")?
        };

        if !status.success() {
            self.shell
                .log("PACMAN: Could not update pacman. Failed when running pacman -Syyyu.");
            return Err(anyhow!("Could not update pacman lists"));
        }
        Ok(())
    }

    pub fn install(&mut self, packages: Vec<&str>) -> Result<()> {
        let packages = packages.join(" ");

        self.shell.log(&format!("Installing {}.", packages));

        let status = if self.is_non_root {
            self.shell.run_and_wait_with_args(
                "su",
                &format!("-c \"{} -Syu --noconfirm {}\"", self.get_program(), packages),
            )?
        } else {
            if let PackageManager::Yay = self.program {
                self.shell.log("ERROR: Called YAY as root.");
                return Err(anyhow!("PACMAN: Called yay as root"));
            }

            self.shell
                .run_and_wait_with_args("pacman", &format!("-Syu --noconfirm {}", packages))?
        };

        if !status.success() {
            self.shell
                .log(&format!("PACMAN: Could not install {}.", packages));
            return Err(anyhow!("Could not install {}", packages));
        }

        Ok(())
    }

    pub fn uninstall(&mut self, packages: Vec<&str>) -> Result<()> {
        let packages = packages.join(" ");
        self.shell.log(&format!("Uninstalling {}.", packages));

        let status = if self.is_non_root {
            self.shell.run_and_wait_with_args(
                "su",
                &format!("-c \"{} -Rns --noconfirm {}\"", self.get_program(), packages),
            )?
        } else {

            if let PackageManager::Yay = self.program {
                self.shell.log("ERROR: Called YAY as root.");
                return Err(anyhow!("PACMAN: Called yay as root"));
            }
            self.shell
                .run_and_wait_with_args("pacman", &format!("-Rns --noconfirm {}", packages))?
        };

        if !status.success() {
            self.shell
                .log(&format!("Could not uninstall {}.", packages));
            return Err(anyhow!("Could not uninstall {}", packages));
        }

        Ok(())
    }

    /// newer arch isos include reflector by default. this should be used in the live environment
    /// only. Using it in chroot without reflector installed might panic.
    pub fn run_reflector(&mut self, country: &str) -> Result<()> {
        let status = if self.is_non_root {
            self.shell.run_and_wait_with_args(
                "su",
                &format!(
                    "-c \"reflector -c {} --sort rate --save /etc/pacman.d/mirrorlist\"",
                    country
                ),
            )?
        } else {
            self.shell.run_and_wait_with_args(
                "reflector",
                &format!("-c {} --sort rate --save /etc/pacman.d/mirrorlist", country),
            )?
        };

        if !status.success() {
            self.shell
                .log("PACMAN: Reflector failed. Exited with non-zero status.");
            return Err(anyhow!(
                "Could not retrieve new pacman mirrors from reflector."
            ));
        }

        self.update_mirrors()?;

        Ok(())
    }


    fn get_program(&self) -> &str {
        match self.program {
            PackageManager::Pacman => &"pacman",
            PackageManager::Yay => &"yay",
        }
    }
}
