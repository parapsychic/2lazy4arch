use anyhow::{Result, Ok};
use enums::StorageSize;
use filesystem_tasks::Filesystem;
use shell_iface::{Shell, logger::Logger};

pub mod filesystem_tasks;
pub mod pacman;
pub mod enums;

pub fn base_install(shell: &mut Shell) -> Result<()>{
    
    Ok(()) 
}


pub fn install(mut shell: &mut Shell, logger: &Logger) -> Result<()>{
     Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install() {
    }
}
