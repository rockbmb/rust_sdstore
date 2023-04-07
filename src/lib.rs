use std::{io, path::{PathBuf}};

pub mod config;
pub mod util;

pub fn create_file_for_udsock(path: &str) -> io::Result<PathBuf> {
    let pwd = std::env::current_dir()?;
    Ok(pwd.join(path))
}