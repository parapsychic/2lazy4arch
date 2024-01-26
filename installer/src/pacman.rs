use shell_iface::{Shell, logger::Logger};
use anyhow::{Result, anyhow};

struct Pacman<'a>{
    shell: &'a mut Shell<'a>,
    logger: &'a Logger
}

impl<'a> Pacman<'a> {
    fn update_mirrors(&mut self) -> Result<()>{
        let status = self.shell.run_and_wait_with_args("pacman", "-Syyyu")?;
        if !status.success(){
            self.logger.debug("PACMAN: Could not update pacman. Failed when running pacman -Syyyu.");
            return Err(anyhow!("Could not update pacman lists"));
        }
        Ok(())
    }

    fn install(&mut self, packages: &str) -> Result<()>{
        let status = self.shell.run_and_wait_with_args("pacman", &format!("-Syu {}", packages))?;
        if !status.success(){
            self.logger.debug(&format!("PACMAN: Could not install {}.", packages));
            return Err(anyhow!("Could not install {}", packages));
        }

        Ok(())
    }

    fn run_reflector(&mut self, country: &str) -> Result<()>{
        self.install("reflector")?;

        let status = self.shell.run_and_wait_with_args("reflector", &format!("-c {} --sort rate --save /etc/pacman.d/mirrorlist", country))?;
        if !status.success(){
            self.logger.debug("PACMAN: Reflector failed. Exited with non-zero status.");
            return Err(anyhow!("Could not retrieve new pacman mirrors from reflector."));
        }
        
        self.update_mirrors()?;

        Ok(())
    }
}
