use std::{
	path::Path,
	fs::{self, File},
    io::Read
};
#[cfg(not(target_family = "windows"))]
use std::os::unix::fs::PermissionsExt;

const S_IXOTH: u32 = 0o1;
#[cfg(not(target_family = "windows"))]
pub fn is_executable(path: &impl AsRef<Path>) -> bool {
    // Linux/Unix uses a file permission bit
    let metadata = fs::metadata(path);
    match metadata {
        Ok(m) => {
            // let S_IXUSR = 0o100;
            // let S_IXGRP = 0o10;
            let mode = m.permissions().mode();
            (mode & (S_IXOTH)) != 0
        }
        Err(_) => false
    }
}

#[cfg(target_family = "windows")]
pub fn is_executable(path: &impl AsRef<Path>) -> bool {
    // Windows executables have certain extensions
    let executable_extns = ["exe", "bat", "com"];
    match path.extension() {
        Some(ext) => {executable_extns.iter().any(
            |extn| ext.eq_ignore_ascii_case(extn))},
        None => false
    }
}

pub fn is_iwad(path: &impl AsRef<Path>) -> bool {
    let iwad = b"IWAD";
    let mut magic: [u8; 4] = [0; 4];
    match File::open(path) {
        Ok(mut f) => {
            let ok = f.read_exact(&mut magic).is_ok();
            if !ok { return false; }
            &magic == iwad
        },
        Err(e) => {
            eprintln!("{:?}", e);
            false
        },
    }
}
