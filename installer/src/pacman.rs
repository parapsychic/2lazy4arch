use anyhow::{anyhow, Result};
use shell_iface::{logger::Logger, Shell};

pub struct Pacman<'a> {
    shell: Shell<'a>,
}

impl<'a> Pacman<'a> {
    pub fn new<'b>(logger: &'b Logger) -> Pacman<'b> {
        let shell = Shell::new("Pacman", logger);
        Pacman { shell }
    }

    pub fn update_mirrors(&mut self) -> Result<()> {
        let status = self
            .shell
            .run_and_wait_with_args("pacman", "-Syyy --noconfirm")?;
        if !status.success() {
            self.shell
                .log("PACMAN: Could not update pacman. Failed when running pacman -Syyyu.");
            return Err(anyhow!("Could not update pacman lists"));
        }
        Ok(())
    }

    pub fn install(&mut self, packages: Vec<&str>) -> Result<()> {
        let packages = packages.join(" ");

        self.shell
            .log(&format!("Installing {}.", packages));

        let status = self
            .shell
            .run_and_wait_with_args("pacman", &format!("-Syu --noconfirm {}", packages))?;
        if !status.success() {
            self.shell
                .log(&format!("PACMAN: Could not install {}.", packages));
            return Err(anyhow!("Could not install {}", packages));
        }

        Ok(())
    }

    pub fn uninstall(&mut self, packages: Vec<&str>) -> Result<()> {
        let packages = packages.join(" ");
        self.shell
            .log(&format!("Uninstalling {}.", packages));

        let status = self
            .shell
            .run_and_wait_with_args("pacman", &format!("-Rns --noconfirm {}", packages))?;
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
        let status = self.shell.run_and_wait_with_args(
            "reflector",
            &format!("-c {} --sort rate --save /etc/pacman.d/mirrorlist", country),
        )?;

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
}
