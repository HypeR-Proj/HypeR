// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use crate::cli::{Ls, Sort};
use std::fs::{self, Metadata};
use std::io::{self, Write};
#[cfg(target_os = "hyper")]
use std::os::hyper::fs::MetadataExt;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

struct Entry {
    name: String,
    path: PathBuf,
    metadata: io::Result<Metadata>,
}

/// Lists available children and returns true when any entry could not be read.
///
/// A child can disappear or belong to a failed mounted filesystem after its
/// name was enumerated. Preserve that name without inventing attributes; the
/// error belongs to that child, not to the containing directory.
pub fn list(
    path: &Path,
    args: &Ls,
    output: &mut impl Write,
    errors: &mut impl Write,
) -> io::Result<bool> {
    let metadata = fs::symlink_metadata(path)?;
    let mut entries = Vec::new();
    let mut failed = false;
    if metadata.is_dir() && !args.directory {
        for entry in fs::read_dir(path)? {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    diagnostic(errors, path, &error)?;
                    failed = true;
                    continue;
                }
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            if !args.all && name.starts_with('.') {
                continue;
            }
            entries.push(Entry {
                name,
                path: entry.path(),
                metadata: entry.metadata(),
            });
        }
    } else {
        entries.push(Entry {
            name: path.to_string_lossy().into_owned(),
            path: path.to_path_buf(),
            metadata: Ok(metadata),
        });
    }
    Ok(write_entries(entries, args, output, errors)? || failed)
}

fn diagnostic(errors: &mut impl Write, path: &Path, error: &io::Error) -> io::Result<()> {
    writeln!(
        errors,
        "ls: {}: {error}",
        crate::escape_path(&path.to_string_lossy())
    )
}

fn write_entries(
    entries: Vec<Entry>,
    args: &Ls,
    output: &mut impl Write,
    errors: &mut impl Write,
) -> io::Result<bool> {
    let mut failed = false;
    let sort = if args.newest {
        Sort::Time
    } else if args.largest {
        Sort::Size
    } else {
        args.sort
    };
    // Gather fallible keys before sorting. Missing keys stay unknown rather
    // than becoming fabricated zero sizes or epoch timestamps.
    let mut entries: Vec<_> = entries
        .into_iter()
        .map(|entry| {
            let modified = match &entry.metadata {
                Ok(metadata) if matches!(sort, Sort::Time) => match metadata.modified() {
                    Ok(time) => Some(time),
                    Err(error) => {
                        diagnostic(errors, &entry.path, &error)?;
                        failed = true;
                        None
                    }
                },
                Ok(_) => None,
                Err(error) => {
                    diagnostic(errors, &entry.path, error)?;
                    failed = true;
                    None
                }
            };
            Ok((entry, modified))
        })
        .collect::<io::Result<_>>()?;
    entries.sort_by(|(a, a_time), (b, b_time)| match sort {
        Sort::Name => a.name.cmp(&b.name),
        Sort::Size => b
            .metadata
            .as_ref()
            .ok()
            .map(Metadata::len)
            .cmp(&a.metadata.as_ref().ok().map(Metadata::len))
            .then(a.name.cmp(&b.name)),
        Sort::Time => b_time.cmp(a_time).then(a.name.cmp(&b.name)),
    });
    if args.reverse {
        entries.reverse();
    }
    if !args.names_only {
        writeln!(output, "MODE                 SIZE  NAME")?;
    }
    for (entry, _) in entries {
        let name = crate::escape_path(&entry.name);
        let Ok(metadata) = entry.metadata else {
            if args.names_only {
                writeln!(output, "{name}")?;
            } else {
                writeln!(output, "?????????? {:>14}  {name}", "?")?;
            }
            continue;
        };
        let directory = metadata.is_dir();
        let symlink = metadata.file_type().is_symlink();
        let suffix = if directory {
            "/"
        } else if symlink {
            "@"
        } else {
            ""
        };
        if args.names_only {
            writeln!(output, "{name}{suffix}")?;
        } else {
            // Read the link itself: relative and dangling targets must not be
            // resolved against the listing's working directory.
            let target = if symlink {
                match fs::read_link(&entry.path) {
                    Ok(target) => format!(" -> {}", crate::escape_path(&target.to_string_lossy())),
                    Err(error) => {
                        diagnostic(errors, &entry.path, &error)?;
                        failed = true;
                        " -> ?".into()
                    }
                }
            } else {
                String::new()
            };
            let size = if directory {
                "-".into()
            } else if args.bytes {
                metadata.len().to_string()
            } else {
                crate::readable_size(metadata.len())
            };
            writeln!(
                output,
                "{} {:>14}  {name}{suffix}{target}",
                crate::mode_text(metadata.mode(), directory, symlink),
                size
            )?;
        }
    }
    Ok(failed)
}

#[cfg(test)]
#[path = "../tests/listing.rs"]
mod tests;
