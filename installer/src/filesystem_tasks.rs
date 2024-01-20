use anyhow::Result;
use shell_iface::Shell;


use crate::enums::StorageSize;

pub fn initialize_swap(shell: &Shell ,size: usize,  unit: Option<StorageSize>) -> Result<()> {
    
    let unit_char =  match unit {
        Some(StorageSize::Megabyte) => 'M',
        Some(StorageSize::Gigabyte) => 'G',
        Some(StorageSize::Kilobyte) => 'K',
        Some(StorageSize::Terabyte) => 'T',
        Some(StorageSize::Byte) => ' ',
        None => {
            println!("No unit set, defaulting to Megabytes. \n [This shouldn't have happenend. The UI has sent some wrong info");
            'M'
        },
    };
    
    println!("Size: {}", size);
    println!("Creating Swap Partition");
    shell.run("dd", "if=/dev/zero of=/swapfile bs=1M count=8192 status=progress")?;
    Ok(())
}
