use std::{
    fs::{self, OpenOptions},
    io::{Write, self, BufReader, BufRead},
};

use anyhow::{anyhow, Result};

pub const DWM_SCRIPT_URL : &str = "";
pub const RICE_SCRIPT_URL : &str = "";

/// Opens a file, writes the content.
/// Creates the file if the file does not exist.
pub fn write_to_file(path: &str, content: &str) -> Result<()> {
    let mut fstab = match OpenOptions::new().write(true).create(true).open(path) {
        Ok(x) => x,
        Err(e) => {
            return Err(anyhow!(e));
        }
    };

    if let Err(e) = fstab.write(content.as_bytes()) {
        return Err(anyhow!(e));
    }

    Ok(())
}

/// Opens a file, appends the content.
/// Creates the file if the file does not exist.
/// Adds a newline before appending just to be sure.
pub fn append_to_file(path: &str, content: &str) -> Result<()> {
    let mut fstab = match OpenOptions::new().append(true).create(true).open(path) {
        Ok(x) => x,
        Err(e) => {
            return Err(anyhow!(e));
        }
    };

    if let Err(e) = fstab.write(format!("\n{}", content).as_bytes()) {
        return Err(anyhow!(e));
    }

    Ok(())
}

/// Glorified sed function to insert lines at specified line number.
/// This can in theory do the same stuff as append and write.
pub fn sed(file_path: &str, line_number: usize, line_content: &str) -> io::Result<()> {
    let file = fs::File::open(file_path)?;
    let reader = BufReader::new(file);

    let mut lines = reader.lines().collect::<io::Result<Vec<String>>>()?;

    if line_number <= lines.len() {
        lines.insert(line_number - 1, line_content.to_string());
    } else {
        lines.push(line_content.to_string());
    }

    let mut new_file = fs::File::create(file_path)?;

    for line in lines {
        writeln!(new_file, "{}", line)?;
    }

    Ok(())
}

/// Checks whether the processor is Intel or AMD.
/// Returns None if none of them.
pub fn get_processor_make() -> Option<String> {
    let processor_info = match fs::read_to_string("/proc/cpuinfo") {
        Ok(x) => x,
        Err(_) => return None,
    };

    if processor_info.contains("AuthenticAMD") {
        return Some(String::from("amd"));
    } 
    if processor_info.contains("GenuineIntel") {
        return Some(String::from("intel"));
    }
   return None;
}

/// Get UUID of root
/// This might fail if:
/// - the fstab is not generated
/// - the fstab is not generated with UUIDs using genfstab
/// - the root's UUID is not in fstab
pub fn get_uuid_root() -> Result<String> {
    let fstab = match fs::read_to_string("/etc/fstab") {
        Ok(x) => x.lines().filter(|x|{
            let line = x.trim();
            return !(line.starts_with("#") || line.is_empty())
        }).collect::<Vec<&str>>().iter().map(|l| {
            l.to_string()
        }).collect::<Vec<String>>(),
        Err(_) => {
            return Err(anyhow!("Could not open fstab"));
        },
    };
    
    for line in fstab {
        let row = line.split("\t").map(|x| {
            x.trim()
        }).collect::<Vec<&str>>();
        
        if row.len() > 2 {
            //println!("{:#?}", row);
            if row[1] == "/"{
                match row[0].split("=").last() {
                    Some(x) => return Ok(x.to_string()),
                    None => {},
                };
            }
        }
    }
    
    Err(anyhow!("Could not find UUID"))
}

/// Check if a string is a valid mountpoint name
pub fn is_valid_mount_point(name: &str) -> bool {
    // Allowed characters in mount point names
    let allowed_chars = |c: char| {
        c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.'
    };

    // Check if the string is empty or starts with a hyphen
    if name.is_empty() || name.starts_with('-') {
        return false;
    }

    // Check each character in the string
    for c in name.chars() {
        if !allowed_chars(c) {
            return false;
        }
    }

    true
}

