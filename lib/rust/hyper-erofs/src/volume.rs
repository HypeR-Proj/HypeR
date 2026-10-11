// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use alloc::string::String;

use crate::{Device, Error, directory, inode::Inode, reader::Reader, superblock::Superblock};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    File,
    Directory,
    Symlink,
}

#[derive(Debug)]
pub struct Entry {
    pub name: String,
    pub kind: Kind,
    pub mode: u32,
    pub size: u64,
    pub modified: (i64, u32),
}

/// Immutable media semantics with a bounded metadata block and last-path cache.
/// File data is read directly into the caller's range; whole images and files
/// are never buffered by this parser.
pub struct Volume<D> {
    reader: Reader<D>,
    superblock: Superblock,
    root: Inode,
    last: Option<(String, Inode)>,
}

impl<D: Device> Volume<D> {
    /// Return the original device after the service has stopped using the mount.
    pub fn into_device(self) -> D {
        self.reader.device
    }

    pub fn mount(device: D) -> Result<Self, Error> {
        let mut reader = Reader::new(device);
        let superblock = Superblock::load(&mut reader)?;
        let root = Inode::load(&mut reader, &superblock, superblock.root)?;
        if root.kind != Kind::Directory {
            return Err(Error::Corrupt);
        }
        // Validate the root's first directory block before exposing the mount.
        directory::find(&mut reader, &superblock, root, None, 0)?;
        Ok(Self {
            reader,
            superblock,
            root,
            last: None,
        })
    }

    /// Resolve canonical relative paths without following symbolic links.
    /// Namespace-level symlink traversal belongs to the VFS caller.
    fn lookup(&mut self, path: &str) -> Result<Inode, Error> {
        if path.len() > 4096 || path.as_bytes().contains(&0) {
            return Err(Error::InvalidInput);
        }
        if path.is_empty() {
            return Ok(self.root);
        }
        if let Some((cached, inode)) = &self.last
            && cached == path
        {
            return Ok(*inode);
        }
        let mut inode = self.root;
        for part in path.split('/') {
            if part.is_empty() || part == "." || part == ".." || part.len() > 255 {
                return Err(Error::InvalidInput);
            }
            let child = directory::find(
                &mut self.reader,
                &self.superblock,
                inode,
                Some(part.as_bytes()),
                0,
            )?
            .ok_or(Error::Missing)?;
            inode = Inode::load(&mut self.reader, &self.superblock, child.nid)?;
            if child.kind.is_some_and(|kind| kind != inode.kind) {
                return Err(Error::Corrupt);
            }
        }
        self.last = Some((copy_string(path)?, inode));
        Ok(inode)
    }

    pub fn stat(&mut self, path: &str) -> Result<Entry, Error> {
        let inode = self.lookup(path)?;
        entry(inode, path.rsplit('/').next().ok_or(Error::InvalidInput)?)
    }

    pub fn entry(&mut self, path: &str, index: usize) -> Result<Option<Entry>, Error> {
        let directory = self.lookup(path)?;
        let Some(child) =
            directory::find(&mut self.reader, &self.superblock, directory, None, index)?
        else {
            return Ok(None);
        };
        let inode = Inode::load(&mut self.reader, &self.superblock, child.nid)?;
        if child.kind.is_some_and(|kind| kind != inode.kind) {
            return Err(Error::Corrupt);
        }
        let name =
            core::str::from_utf8(&child.name[..child.length]).map_err(|_| Error::Unsupported)?;
        entry(inode, name).map(Some)
    }

    pub fn read(&mut self, path: &str, offset: u64, output: &mut [u8]) -> Result<usize, Error> {
        let inode = self.lookup(path)?;
        if inode.kind != Kind::File {
            return Err(Error::InvalidInput);
        }
        inode.read(&mut self.reader, &self.superblock, offset, output)
    }

    /// Return the stored link bytes verbatim; do not join or normalize a target.
    pub fn readlink(&mut self, path: &str, output: &mut [u8]) -> Result<usize, Error> {
        let inode = self.lookup(path)?;
        if inode.kind != Kind::Symlink {
            return Err(Error::InvalidInput);
        }
        if inode.size == 0 || inode.size > 4096 {
            return Err(Error::Corrupt);
        }
        if inode.size > output.len() as u64 {
            return Err(Error::InvalidInput);
        }
        let count = inode.read(&mut self.reader, &self.superblock, 0, output)?;
        if output[..count].contains(&0) {
            return Err(Error::Corrupt);
        }
        Ok(count)
    }
}

fn entry(inode: Inode, name: &str) -> Result<Entry, Error> {
    Ok(Entry {
        name: copy_string(name)?,
        kind: inode.kind,
        mode: inode.mode,
        size: inode.size,
        modified: inode.modified,
    })
}

fn copy_string(value: &str) -> Result<String, Error> {
    let mut result = String::new();
    result
        .try_reserve_exact(value.len())
        .map_err(|_| Error::Allocation)?;
    result.push_str(value);
    Ok(result)
}
