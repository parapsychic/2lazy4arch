use shell_iface::{Shell, logger::Logger};
use anyhow::{Result, anyhow};

pub struct Pacman<'a>{
    shell: Shell<'a>,
    logger: &'a Logger
}

impl<'a> Pacman<'a> {
    pub fn new<'b>(logger: &'b Logger) -> Pacman<'b>{
        let shell = Shell::new("PACMAN", logger);
        Pacman {
            shell,
            logger
        }
    }

    pub fn update_mirrors(&mut self) -> Result<()>{
        let status = self.shell.run_and_wait_with_args("pacman", "-Syyy")?;
        if !status.success(){
            self.logger.debug("PACMAN: Could not update pacman. Failed when running pacman -Syyyu.");
            return Err(anyhow!("Could not update pacman lists"));
        }
        Ok(())
    }

    pub fn install(&mut self, packages: &str) -> Result<()>{
        let status = self.shell.run_and_wait_with_args("pacman", &format!("-Syu {}", packages))?;
        if !status.success(){
            self.logger.debug(&format!("PACMAN: Could not install {}.", packages));
            return Err(anyhow!("Could not install {}", packages));
        }

        Ok(())
    }

    /// newer arch isos include reflector by default. this should be used in the live environment
    /// only. Using it in chroot without reflector installed might panic.
    pub fn run_reflector(&mut self, country: &str) -> Result<()>{
        let status = self.shell.run_and_wait_with_args("reflector", &format!("-c {} --sort rate --save /etc/pacman.d/mirrorlist", country))?;

        if !status.success(){
            self.logger.debug("PACMAN: Reflector failed. Exited with non-zero status.");
            return Err(anyhow!("Could not retrieve new pacman mirrors from reflector."));
        }
        
        self.update_mirrors()?;

        Ok(())
    }
}
