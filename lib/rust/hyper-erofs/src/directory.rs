// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use crate::{
    Device, Error, Kind,
    inode::Inode,
    reader::{MAX_BLOCK, Reader, u16_at, u64_at},
    superblock::Superblock,
};

const DIRENT_BYTES: usize = 12;
const MAX_DIRECTORY_BYTES: u64 = 16 * 1024 * 1024;

pub(crate) struct Child {
    pub nid: u64,
    pub name: [u8; 255],
    pub length: usize,
    pub kind: Option<Kind>,
}

/// Scans bounded directory blocks, validating the full block before returning
/// any entry. Names are sorted, unique, and cannot escape a path component.
pub(crate) fn find<D: Device>(
    reader: &mut Reader<D>,
    sb: &Superblock,
    inode: Inode,
    name: Option<&[u8]>,
    mut index: usize,
) -> Result<Option<Child>, Error> {
    if inode.kind != Kind::Directory {
        return Err(Error::InvalidInput);
    }
    if inode.size == 0 {
        return Err(Error::Corrupt);
    }
    if inode.size > MAX_DIRECTORY_BYTES {
        return Err(Error::ResourceLimit);
    }
    let mut buffer = [0; MAX_BLOCK];
    let mut offset = 0;
    let mut previous_block_last = [0; 255];
    let mut previous_length = 0;
    while offset < inode.size {
        let length = inode.read(reader, sb, offset, &mut buffer[..sb.block_size])?;
        let block = &buffer[..length];
        let count = validate(block)?;
        let first = child(block, 0, count)?;
        if previous_length != 0 && first.0 <= &previous_block_last[..previous_length] {
            return Err(Error::Corrupt);
        }
        let last = child(block, count - 1, count)?.0;
        previous_block_last[..last.len()].copy_from_slice(last);
        previous_length = last.len();
        for position in 0..count {
            let (entry_name, nid, kind) = child(block, position, count)?;
            if matches!(entry_name, b"." | b"..") {
                continue;
            }
            let selected = if let Some(name) = name {
                name == entry_name
            } else {
                index == 0
            };
            if selected {
                let mut child = Child {
                    nid,
                    name: [0; 255],
                    length: entry_name.len(),
                    kind,
                };
                child.name[..entry_name.len()].copy_from_slice(entry_name);
                return Ok(Some(child));
            }
            if name.is_none() {
                index -= 1;
            }
        }
        offset += length as u64;
    }
    Ok(None)
}

fn validate(block: &[u8]) -> Result<usize, Error> {
    let table = usize::from(u16_at(block, 8)?);
    if table == 0 || table >= block.len() || table % DIRENT_BYTES != 0 {
        return Err(Error::Corrupt);
    }
    let count = table / DIRENT_BYTES;
    let mut previous: &[u8] = &[];
    for index in 0..count {
        let (name, _, _) = child(block, index, count)?;
        if name <= previous {
            return Err(Error::Corrupt);
        }
        previous = name;
    }
    Ok(count)
}

fn child(block: &[u8], index: usize, count: usize) -> Result<(&[u8], u64, Option<Kind>), Error> {
    let record = index.checked_mul(DIRENT_BYTES).ok_or(Error::Corrupt)?;
    let start = usize::from(u16_at(block, record + 8)?);
    let end = if index + 1 == count {
        block.len()
    } else {
        usize::from(u16_at(block, record + DIRENT_BYTES + 8)?)
    };
    if start < count * DIRENT_BYTES || start >= end {
        return Err(Error::Corrupt);
    }
    let mut name = block.get(start..end).ok_or(Error::Corrupt)?;
    if index + 1 == count {
        let length = name
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(name.len());
        // Only the first padding byte terminates the final name; the on-disk
        // format does not require subsequent unused bytes to be zero.
        name = &name[..length];
    }
    if name.is_empty()
        || name.len() > 255
        || name.contains(&0)
        || name.contains(&b'/')
        || block.get(record + 11) != Some(&0)
    {
        return Err(Error::Corrupt);
    }
    let kind = match block.get(record + 10).copied().ok_or(Error::Corrupt)? {
        0 => None,
        1 => Some(Kind::File),
        2 => Some(Kind::Directory),
        7 => Some(Kind::Symlink),
        3..=6 => return Err(Error::Unsupported),
        _ => return Err(Error::Corrupt),
    };
    Ok((name, u64_at(block, record)?, kind))
}
