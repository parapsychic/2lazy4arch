use anyhow::{Result, Ok};
use enums::StorageSize;
use filesystem_tasks::initialize_swap;
use shell_iface::Shell;

pub mod filesystem_tasks;
pub mod enums;

pub fn presetup(shell: &Shell) -> Result<()>{
    shell.run("lsblk", "");
    Ok(()) 
}

pub fn install(shell: &Shell) -> Result<()>{
    initialize_swap(shell, 8, Some(StorageSize::Gigabyte))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install() {
    }
}
