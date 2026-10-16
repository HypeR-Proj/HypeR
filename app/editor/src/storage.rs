// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Save a complete recovery copy before touching the destination. Filesystems
//! supporting replace-by-rename publish it atomically. FAT currently rejects
//! replacement, so it uses an in-place write with the synced copy retained on
//! failure. This is not a power-loss transaction or a concurrent-writer lock.
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub fn load(path: &Path) -> io::Result<String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(super::MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > super::MAX_BYTES {
        return Err(io::Error::other("file exceeds 4 MiB"));
    }
    String::from_utf8(bytes).map_err(|_| io::Error::other("file is not UTF-8 text"))
}

pub fn save(path: &Path, text: &str) -> io::Result<()> {
    let target = match fs::canonicalize(path) {
        Ok(path) => path,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            // Do not replace a dangling symlink with a regular file.
            match fs::symlink_metadata(path) {
                Ok(_) => return Err(io::Error::other("destination is a dangling link")),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            path.to_path_buf()
        }
        Err(error) => return Err(error),
    };
    let metadata = match fs::metadata(&target) {
        Ok(metadata) => Some(metadata),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    if let Some(metadata) = &metadata {
        if !metadata.is_file() {
            return Err(io::Error::other("destination is not a regular file"));
        }
        if metadata.permissions().readonly() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "destination is read-only",
            ));
        }
    }
    let (temporary, mut file) = temporary(&target)?;
    if let Some(metadata) = &metadata
        && let Err(error) = file.set_permissions(metadata.permissions())
    {
        drop(file);
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    let prepared = file
        .write_all(text.as_bytes())
        .and_then(|()| file.sync_all());
    if let Err(error) = prepared {
        drop(file);
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    drop(file);
    let result = match fs::rename(&temporary, &target) {
        Ok(()) => return Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            // Keep the complete synced copy until the replacement is durable.
            (|| {
                let mut destination = OpenOptions::new().write(true).open(&target)?;
                destination.write_all(text.as_bytes())?;
                destination.set_len(text.len() as u64)?;
                destination.sync_all()
            })()
        }
        Err(error) => Err(error),
    };
    match result {
        Ok(()) => {
            let _ = fs::remove_file(temporary);
            Ok(())
        }
        Err(error) => Err(io::Error::new(
            error.kind(),
            format!("{error}; recovery copy: {}", temporary.display()),
        )),
    }
}
fn temporary(target: &Path) -> io::Result<(PathBuf, fs::File)> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    for _ in 0..32 {
        let path = parent.join(format!(
            ".editor-save-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::other("cannot create an editor recovery file"))
}
