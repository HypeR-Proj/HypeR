// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;
use clap::Parser;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Directory(PathBuf);

impl Directory {
    fn new() -> io::Result<Self> {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("hyper-ls-test-{}-{id}", std::process::id()));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }

    fn entry(&self, name: &str) -> Entry {
        let path = self.0.join(name);
        Entry {
            name: name.into(),
            metadata: fs::symlink_metadata(&path),
            path,
        }
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn one_unavailable_child_preserves_the_rest_of_every_sorted_listing() -> io::Result<()> {
    let directory = Directory::new()?;
    fs::create_dir(directory.0.join("bin"))?;
    fs::create_dir(directory.0.join("data"))?;
    fs::write(directory.0.join("healthy"), b"available")?;
    // ReadDir has already supplied this name. Its mounted backend can fail
    // independently before the per-entry metadata lookup.
    for sort in ["name", "size", "time"] {
        let args = Ls::parse_from(["ls", "--sort", sort]);
        let mut unavailable = directory.entry("data");
        unavailable.metadata = Err(io::Error::other("filesystem backend unavailable"));
        let mut output = Vec::new();
        let mut errors = Vec::new();
        assert!(write_entries(
            vec![
                directory.entry("healthy"),
                unavailable,
                directory.entry("bin"),
            ],
            &args,
            &mut output,
            &mut errors,
        )?);
        let output = String::from_utf8_lossy(&output);
        assert!(output.lines().any(|line| line.ends_with("  bin/")));
        assert!(output.lines().any(|line| line.ends_with("  healthy")));
        assert!(
            output
                .lines()
                .any(|line| line == "??????????              ?  data")
        );
        let errors = String::from_utf8_lossy(&errors);
        assert_eq!(errors.lines().count(), 1);
        assert!(errors.contains("/data: filesystem backend unavailable"));
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn lost_symlink_target_does_not_hide_subsequent_entries() -> io::Result<()> {
    let directory = Directory::new()?;
    std::os::unix::fs::symlink("missing-target", directory.0.join("alias"))?;
    fs::write(directory.0.join("healthy"), b"available")?;
    let alias = directory.entry("alias");
    // The metadata lookup succeeded, but the link vanishes before read_link.
    fs::remove_file(&alias.path)?;
    let mut output = Vec::new();
    let mut errors = Vec::new();
    assert!(write_entries(
        vec![alias, directory.entry("healthy")],
        &Ls::parse_from(["ls"]),
        &mut output,
        &mut errors,
    )?);
    let output = String::from_utf8_lossy(&output);
    assert!(output.contains("alias@ -> ?"));
    assert!(output.lines().any(|line| line.ends_with("  healthy")));
    assert!(String::from_utf8_lossy(&errors).contains("/alias:"));
    Ok(())
}

#[test]
fn failed_directory_operand_remains_an_error() -> io::Result<()> {
    let directory = Directory::new()?;
    let mut output = Vec::new();
    let mut errors = Vec::new();
    let result = list(
        &directory.0.join("missing"),
        &Ls::parse_from(["ls"]),
        &mut output,
        &mut errors,
    );
    assert!(matches!(result, Err(error) if error.kind() == io::ErrorKind::NotFound));
    assert!(output.is_empty());
    assert!(errors.is_empty());
    Ok(())
}

#[test]
fn output_failure_is_not_treated_as_a_bad_directory_entry() -> io::Result<()> {
    struct ClosedPipe;
    impl Write for ClosedPipe {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let directory = Directory::new()?;
    fs::write(directory.0.join("healthy"), b"available")?;
    let result = list(
        &directory.0,
        &Ls::parse_from(["ls"]),
        &mut ClosedPipe,
        &mut Vec::new(),
    );
    assert!(matches!(result, Err(error) if error.kind() == io::ErrorKind::BrokenPipe));
    Ok(())
}
