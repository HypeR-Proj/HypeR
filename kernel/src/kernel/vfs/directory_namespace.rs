// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Namespace transactions prepare ownership before one topology publication.

use super::{DirectoryObject, Error};
use crate::kernel::accounting::ResourceDomain;
use crate::kernel::vfs::instance::{DirectoryAnchor, FilesystemInstance, Location};
use crate::kernel::vfs::mounts::{MAX_ROOT_MOVES, View};
use crate::kernel::vfs::scratch::{ScratchBudget, ScratchVec};
use hyper::fs::NodeKind;
use hyper::mm::FallibleArc;

impl DirectoryObject {
    pub(in crate::kernel::vfs) fn mount_remote(
        &self,
        path: &str,
        device: crate::kernel::vfs::remote::Transport,
        sponsor: &ResourceDomain,
    ) -> Result<(), Error> {
        let filesystem = FilesystemInstance::try_from_remote(device, sponsor)?;
        self.mount_filesystem(path, filesystem, sponsor)
    }

    pub(in crate::kernel::vfs) fn mount_filesystem(
        &self,
        path: &str,
        filesystem: FallibleArc<FilesystemInstance>,
        sponsor: &ResourceDomain,
    ) -> Result<(), Error> {
        let view = self.namespace.mounts.snapshot();
        let location = self.namespace_directory(&view, path, None, sponsor)?;
        self.namespace
            .mounts
            .attach(&view, location, filesystem, sponsor)?;
        Ok(())
    }

    pub(in crate::kernel::vfs) fn unmount(
        &self,
        path: &str,
        sponsor: &ResourceDomain,
    ) -> Result<(), Error> {
        let view = self.namespace.mounts.snapshot();
        let location = self.namespace_directory(&view, path, None, sponsor)?;
        if location == *view.root() || location.node().get() != location.mount().root().get() {
            return Err(Error::InvalidInput);
        }
        let mounted = location.mount().clone();
        drop(location);
        self.namespace.mounts.detach(&view, &mounted, sponsor)?;
        Ok(())
    }

    /// Allocate an empty memory filesystem charged to the caller's domain.
    pub(crate) fn mount_ramfs(&self, path: &str, sponsor: &ResourceDomain) -> Result<(), Error> {
        self.require_namespace_root()?;
        let filesystem = FilesystemInstance::try_empty_ramfs(sponsor)?;
        self.mount_filesystem(path, filesystem, sponsor)
    }

    /// Bind resident data without copying it. Remote transports cannot be
    /// aliased because their current unmount contract retires the whole mount.
    #[cfg(feature = "kernel-self-test")]
    pub(crate) fn bind_ramfs(
        &self,
        source: &str,
        target: &str,
        sponsor: &ResourceDomain,
    ) -> Result<(), Error> {
        self.require_namespace_root()?;
        let view = self.namespace.mounts.snapshot();
        let source = self.namespace_directory(&view, source, None, sponsor)?;
        let target = self.namespace_directory(&view, target, None, sponsor)?;
        self.namespace
            .mounts
            .bind_ramfs(&view, &source, target, sponsor)?;
        Ok(())
    }

    /// Replace the shared namespace root and relocate mounts or RAM subtrees in one
    /// commit. Existing files and fixed directory capabilities retain identity;
    /// only explicitly root-following authorities observe the replacement.
    /// `put_old` and move destinations are relative to the new root. Sources
    /// are resolved in the old namespace. RAM subtree names are detached while
    /// their node identities survive in new mounts. A cwd scoped to an ancestor
    /// of a detached subtree loses relative reachability; a scope rooted within
    /// the subtree remains usable. No filesystem is unmounted or copied.
    pub(crate) fn switch_root(
        &self,
        new_root: &str,
        put_old: &str,
        moves: &[(&str, &str)],
        sponsor: &ResourceDomain,
    ) -> Result<(), Error> {
        self.require_namespace_root()?;
        if moves.len() > MAX_ROOT_MOVES {
            return Err(Error::InvalidInput);
        }
        let view = self.namespace.mounts.snapshot();
        let new_root = self.namespace_directory(&view, new_root, None, sponsor)?;
        let put_old = self.namespace_directory(&view, put_old, Some(&new_root), sponsor)?;
        let mut locations = ScratchVec::new(ScratchBudget::new(sponsor));
        locations.try_reserve_exact(moves.len())?;
        for (source, target) in moves {
            locations.push((
                self.namespace_directory(&view, source, None, sponsor)?,
                self.namespace_directory(&view, target, Some(&new_root), sponsor)?,
            ))?;
        }
        self.namespace
            .mounts
            .switch_root(&view, new_root, put_old, &locations, sponsor)?;
        Ok(())
    }

    fn require_namespace_root(&self) -> Result<(), Error> {
        if self.root.follows_root() && self.current.follows_root() {
            Ok(())
        } else {
            Err(Error::AccessDenied)
        }
    }

    fn namespace_directory(
        &self,
        view: &View,
        path: &str,
        root: Option<&Location>,
        sponsor: &ResourceDomain,
    ) -> Result<Location, Error> {
        let anchor = root.map(|location| DirectoryAnchor::Fixed(location.clone()));
        crate::kernel::vfs::resolve::typed_in_view(
            &self.namespace,
            view,
            anchor.as_ref().unwrap_or(&self.root),
            anchor.as_ref().unwrap_or(&self.current),
            path,
            NodeKind::Directory,
            &ScratchBudget::new(sponsor),
        )
    }
}
