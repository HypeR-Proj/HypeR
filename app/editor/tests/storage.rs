// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use crate::storage;
use std::os::unix::fs::PermissionsExt;
use std::{fs, io};
#[test]
fn create_replace_symlink_and_reject_invalid_destination() -> io::Result<()> {
    let path = std::env::temp_dir().join(format!("hyper-editor-test-{}", std::process::id()));
    fs::create_dir(&path)?;
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(path.clone());
    let file = path.join("file");
    storage::save(&file, "abc\n")?;
    assert_eq!(storage::load(&file)?, "abc\n");
    storage::save(&file, "界")?;
    assert_eq!(fs::read(&file)?, "界".as_bytes());
    std::os::unix::fs::symlink("file", path.join("link"))?;
    storage::save(&path.join("link"), "via link")?;
    assert_eq!(
        fs::read_link(path.join("link"))?,
        std::path::Path::new("file")
    );
    assert_eq!(storage::load(&file)?, "via link");
    fs::set_permissions(&file, fs::Permissions::from_mode(0o640))?;
    storage::save(&file, "private")?;
    assert_eq!(fs::metadata(&file)?.permissions().mode() & 0o777, 0o640);
    fs::set_permissions(&file, fs::Permissions::from_mode(0o440))?;
    let error = storage::save(&file, "must not replace").err();
    assert_eq!(
        error.map(|e| e.kind()),
        Some(io::ErrorKind::PermissionDenied)
    );
    assert_eq!(storage::load(&file)?, "private");
    assert!(storage::save(&file.join("child"), "bad").is_err());
    assert!(storage::save(&path, "bad").is_err());
    std::os::unix::fs::symlink("missing", path.join("dangling"))?;
    assert!(storage::save(&path.join("dangling"), "bad").is_err());
    assert!(fs::symlink_metadata(path.join("dangling"))?.is_symlink());
    assert_eq!(fs::read_dir(&path)?.count(), 3);
    Ok(())
}
