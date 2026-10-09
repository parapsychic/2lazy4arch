//! Where a config came from, so relative paths in it (scripts, package lists,
//! certificates, compose files) resolve next to it: a folder, or a URL.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{bail, Context, Result};

/// Downloaded files live here (on the ISO's RAM disk).
const DOWNLOADS: &str = "/tmp/2lazy4arch/files";

#[derive(Debug, Clone)]
pub enum Source {
    /// Folder holding the config file
    Dir(PathBuf),
    /// URL of the config's folder, ending in '/'
    Url(String),
}

fn is_url(s: &str) -> bool {
    s.contains("://")
}

/// Reads the config text from a path or an http(s)/ftp/... URL.
pub fn load(location: &str) -> Result<(String, Source)> {
    if is_url(location) {
        let file = download(location)?;
        let text = fs::read_to_string(&file).with_context(|| format!("reading {location}"))?;
        let base = &location[..location.rfind('/').map_or(location.len(), |i| i + 1)];
        Ok((text, Source::Url(base.to_string())))
    } else {
        let path = Path::new(location);
        let text = fs::read_to_string(path).with_context(|| format!("reading {location}"))?;
        let dir = fs::canonicalize(path)?.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
        Ok((text, Source::Dir(dir)))
    }
}

/// curl to a file under DOWNLOADS, named after the URL.
fn download(url: &str) -> Result<PathBuf> {
    let name: String = url.chars().map(|c| if c.is_ascii_alphanumeric() || ".-_".contains(c) { c } else { '_' }).collect();
    let dest = Path::new(DOWNLOADS).join(name);
    fs::create_dir_all(DOWNLOADS)?;
    let status = Command::new("curl").args(["-fsSL", "--retry", "3", "-o"]).arg(&dest).arg(url).status()?;
    if !status.success() {
        bail!("could not download {url} ({status})");
    }
    Ok(dest)
}

impl Source {
    /// The config's folder or URL folder, as base() wrote it.
    pub fn from_base(base: &str) -> Source {
        if is_url(base) {
            Source::Url(base.to_string())
        } else {
            Source::Dir(PathBuf::from(base))
        }
    }

    pub fn base(&self) -> String {
        match self {
            Source::Dir(dir) => dir.display().to_string(),
            Source::Url(url) => url.clone(),
        }
    }

    /// Folder for LAZY_DIR: the config's folder, or where its files are downloaded.
    pub fn dir(&self) -> PathBuf {
        match self {
            Source::Dir(dir) => dir.clone(),
            Source::Url(_) => PathBuf::from(DOWNLOADS),
        }
    }

    /// A local copy of a file named in the config. Absolute paths and URLs are
    /// taken as they are; relative ones are next to the config.
    pub fn fetch(&self, name: &str) -> Result<PathBuf> {
        if is_url(name) {
            return download(name);
        }
        let path = Path::new(name);
        if path.is_absolute() {
            return if path.exists() { Ok(path.to_path_buf()) } else { bail!("{name} doesn't exist") };
        }
        match self {
            Source::Dir(dir) => {
                let local = dir.join(path);
                if local.exists() {
                    Ok(local)
                } else {
                    bail!("{} doesn't exist", local.display())
                }
            }
            Source::Url(base) => download(&format!("{base}{}", name.trim_start_matches("./"))),
        }
    }

    /// Whether `name` can only be fetched as a single file (no folder next to it).
    pub fn is_remote(&self, name: &str) -> bool {
        is_url(name) || (matches!(self, Source::Url(_)) && !Path::new(name).is_absolute())
    }
}
