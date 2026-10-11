// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Userspace deployment policy for replacing the namespace root. Preparation
//! creates a separate ramfs; only the final syscall changes root or moves the
//! live mounts. A rejected transition leaves the bootstrap namespace usable.

use hyper_os::fs::{Directory, DirectoryRights, MAX_PATH_BYTES};
use hyper_os::{Error, Status, filesystem};
use serde::{Deserialize, Serialize};

const MAX_ENTRIES: usize = 16;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(super) struct Config {
    new_root: String,
    put_old: String,
    #[serde(default)]
    directories: Vec<String>,
    #[serde(default)]
    links: Vec<Link>,
    #[serde(default)]
    mounts: Vec<Mount>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Link {
    path: String,
    target: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Mount {
    source: String,
    target: String,
    #[serde(default)]
    ramfs: bool,
    #[serde(default)]
    optional: bool,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SourceState {
    Missing,
    Directory,
    Mounted,
}

impl Config {
    pub(super) fn put_old(&self) -> &str {
        &self.put_old
    }

    /// Reject ambiguous destinations before the candidate accepts supervision.
    pub(super) fn validate(&self) -> Result<(), &'static str> {
        if !absolute_path(&self.new_root)
            || !absolute_path(&self.put_old)
            || self.directories.len() > MAX_ENTRIES
            || self.links.len() > MAX_ENTRIES
            || self.mounts.len() > MAX_ENTRIES
        {
            return Err("invalid root switch paths or entry count");
        }
        for (index, mount) in self.mounts.iter().enumerate() {
            if !absolute_path(&mount.source)
                || !absolute_path(&mount.target)
                || related(&mount.source, &self.new_root)
                || related(&mount.target, &self.put_old)
                || self.mounts[..index]
                    .iter()
                    .any(|previous| !compatible_mounts(previous, mount))
            {
                return Err("overlapping or invalid root switch mounts");
            }
        }
        for (index, directory) in self.directories.iter().enumerate() {
            if !absolute_path(directory)
                || self.directories[..index].contains(directory)
                || related(directory, &self.put_old)
            {
                return Err("invalid root switch directory");
            }
        }
        for (index, link) in self.links.iter().enumerate() {
            if !absolute_path(&link.path)
                || !link_target(&link.target)
                || self.covered(&link.path)
                || self
                    .directories
                    .iter()
                    .any(|path| related(path, &link.path))
                || self.links[..index]
                    .iter()
                    .any(|previous| related(&previous.path, &link.path))
            {
                return Err("overlapping or invalid root switch links");
            }
        }
        for path in self
            .directories
            .iter()
            .map(String::as_str)
            .chain(self.links.iter().map(|link| link.path.as_str()))
            .chain(self.mounts.iter().map(|mount| mount.target.as_str()))
            .chain(std::iter::once(self.put_old.as_str()))
        {
            if self.new_root.len() + path.len() > MAX_PATH_BYTES {
                return Err("root switch staging path is too long");
            }
        }
        Ok(())
    }

    fn covered(&self, path: &str) -> bool {
        related(path, &self.put_old) || self.mounts.iter().any(|mount| related(path, &mount.target))
    }

    /// Moving a mounted ancestor preserves its existing child mount edges.
    /// Select from observed mountpoints, not merely configured paths: an
    /// optional absent ancestor must not hide a separately mounted child.
    fn selected_mounts(&self, states: &[SourceState]) -> Result<Vec<&Mount>, String> {
        if states.len() != self.mounts.len() {
            return Err("incomplete root switch mount inventory".into());
        }
        let mut present = Vec::new();
        for (mount, state) in self.mounts.iter().zip(states) {
            match (mount.ramfs, state) {
                (_, SourceState::Mounted) | (true, SourceState::Directory) => present.push(mount),
                _ if mount.optional => {}
                _ => return Err(format!("{} is not a mounted filesystem", mount.source)),
            }
        }
        for child in &present {
            if let Some(parent) = present.iter().find(|parent| {
                parent.ramfs && descendant_suffix(&child.source, &parent.source).is_some()
            }) {
                return Err(format!(
                    "unsupported root switch: {} is mounted below RAM subtree {}; RAM subtree transfer cannot contain mounted descendants",
                    child.source, parent.source,
                ));
            }
        }
        Ok(present
            .iter()
            .copied()
            .filter(|mount| {
                !present
                    .iter()
                    .any(|ancestor| descendant_suffix(&mount.source, &ancestor.source).is_some())
            })
            .collect())
    }

    pub(super) fn apply(&self, root: &Directory) -> Result<(), String> {
        self.validate().map_err(String::from)?;
        // Never reuse an existing staging directory. This also prevents a
        // retry from silently consuming another attempt's mounted namespace.
        root.create_directory(&self.new_root, 0o700).map_err(show)?;
        if let Err(error) = filesystem::mount_ramfs(root.as_handle_ref(), &self.new_root) {
            let _ = root.remove_directory(&self.new_root);
            return Err(show(error));
        }
        match self.prepare_and_switch(root) {
            Ok(()) => {
                self.clean_old_mountpoints(root);
                Ok(())
            }
            Err(error) => {
                // Only preparation and the atomic switch can return an error.
                // Post-commit cleanup never reopens the rollback path.
                filesystem::unmount(root.as_handle_ref(), &self.new_root).map_err(|cleanup| {
                    format!("{error}; staging root cleanup failed: {cleanup:?}")
                })?;
                root.remove_directory(&self.new_root).map_err(|cleanup| {
                    format!("{error}; staging directory cleanup failed: {cleanup:?}")
                })?;
                Err(error)
            }
        }
    }

    fn prepare_and_switch(&self, root: &Directory) -> Result<(), String> {
        let states = self
            .mounts
            .iter()
            .map(|mount| source_state(root, &mount.source))
            .collect::<Result<Vec<_>, _>>()?;
        let selected = self.selected_mounts(&states)?;
        for path in self
            .directories
            .iter()
            .map(String::as_str)
            .chain(std::iter::once(self.put_old.as_str()))
            .chain(self.mounts.iter().map(|mount| mount.target.as_str()))
        {
            create_directories(root, &format!("{}{path}", self.new_root))?;
        }
        for link in &self.links {
            let path = format!("{}{}", self.new_root, link.path);
            let (parent, _) = path.rsplit_once('/').ok_or("invalid link parent")?;
            create_directories(root, parent)?;
            root.symlink(&link.target, &path).map_err(show)?;
        }
        // Mounted filesystems move as a unit; a configured RAM subtree is
        // promoted and detached from its old directory in the same transaction.
        let moves = selected
            .iter()
            .map(|mount| (mount.source.as_str(), mount.target.as_str()))
            .collect::<Vec<_>>();
        filesystem::switch_root(root.as_handle_ref(), &self.new_root, &self.put_old, &moves)
            .map_err(show)
    }

    /// A moved mount leaves an empty covered directory in the retained old
    /// root. Remove only that directory, never its contents. Busy snapshots can
    /// briefly pin it; failure is a cleanup warning after a committed switch.
    fn clean_old_mountpoints(&self, root: &Directory) {
        let mut paths = self
            .mounts
            .iter()
            .map(|mount| mount.source.as_str())
            .chain(std::iter::once(self.new_root.as_str()))
            .collect::<Vec<_>>();
        paths.sort_unstable_by_key(|path| std::cmp::Reverse(path.len()));
        for path in paths {
            let path = format!("{}{path}", self.put_old);
            for attempt in 0..4 {
                match root.remove_directory(&path) {
                    Ok(()) | Err(Error::Status(Status::NOT_FOUND)) => break,
                    Err(Error::Status(Status::BUSY)) if attempt < 3 => {
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                    Err(error) => {
                        eprintln!("HypeR init-system: retained old mountpoint {path}: {error:?}");
                        break;
                    }
                }
            }
        }
    }
}

fn source_state(root: &Directory, source: &str) -> Result<SourceState, String> {
    let rights = DirectoryRights::READ.union(DirectoryRights::INSPECT);
    let directory = match root.open_directory(source, rights) {
        Ok(directory) => directory,
        Err(Error::Status(Status::NOT_FOUND)) => return Ok(SourceState::Missing),
        Err(error) => return Err(show(error)),
    };
    let (parent, _) = source.rsplit_once('/').ok_or("invalid mount source")?;
    let parent = root
        .open_directory(if parent.is_empty() { "/" } else { parent }, rights)
        .map_err(show)?;
    // A directory inside an already mounted filesystem has a different mount
    // from the namespace root, but is not itself a transferable mountpoint.
    Ok(
        if directory.info().map_err(show)?.location().mount()
            == parent.info().map_err(show)?.location().mount()
        {
            SourceState::Directory
        } else {
            SourceState::Mounted
        },
    )
}

fn compatible_mounts(first: &Mount, second: &Mount) -> bool {
    if first.source == second.source || first.target == second.target {
        return false;
    }
    if let Some(suffix) = descendant_suffix(&second.source, &first.source) {
        return second.target.strip_prefix(&first.target) == Some(suffix);
    }
    if let Some(suffix) = descendant_suffix(&first.source, &second.source) {
        return first.target.strip_prefix(&second.target) == Some(suffix);
    }
    !related(&first.target, &second.target)
}

fn descendant_suffix<'a>(path: &'a str, ancestor: &str) -> Option<&'a str> {
    path.strip_prefix(ancestor)
        .filter(|suffix| suffix.starts_with('/'))
}

fn create_directories(root: &Directory, path: &str) -> Result<(), String> {
    for (index, _) in path
        .match_indices('/')
        .skip(1)
        .chain(std::iter::once((path.len(), "")))
    {
        let path = &path[..index];
        match root.create_directory(path, 0o755) {
            Ok(()) => {}
            Err(Error::Status(Status::ALREADY_EXISTS)) => {
                // Do not follow a symlink planted at a prepared mountpoint.
                root.open_directory_nofollow(path, DirectoryRights::READ)
                    .map_err(show)?;
            }
            Err(error) => return Err(show(error)),
        }
    }
    Ok(())
}

fn absolute_path(path: &str) -> bool {
    path.starts_with('/') && link_target(path) && path != "/"
}

fn link_target(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= MAX_PATH_BYTES
        && !path.as_bytes().contains(&0)
        && path
            .trim_start_matches('/')
            .split('/')
            .all(|part| !matches!(part, "" | "." | ".."))
        && !path.starts_with("//")
}

fn related(first: &str, second: &str) -> bool {
    first == second
        || first
            .strip_prefix(second)
            .is_some_and(|suffix| suffix.starts_with('/'))
        || second
            .strip_prefix(first)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn show(error: Error) -> String {
    format!("{error:?}")
}

#[cfg(test)]
#[path = "../../tests/root_switch.rs"]
mod tests;
