// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bounded user input snapshots for privileged namespace transactions.

use crate::kernel::capability::{HandleValue, Rights};
use crate::kernel::mm::user_space::{UserAddress, UserSlice};
use crate::kernel::process::Process;
use crate::kernel::vfs::service::copy_path;
use crate::kernel::vfs::{DirectoryObject, VfsServiceError};
use hyper::abi::native as abi;

const NAMESPACE_RIGHTS: Rights = Rights::MANAGE_NAMESPACE
    .union(Rights::WRITE)
    .union(Rights::EXECUTE);
const MAX_MOVES: usize = abi::HYPER_NATIVE_FILESYSTEM_SWITCH_ROOT_MAX_MOVES as usize;

pub(super) fn mount_ramfs(
    process: &Process,
    root: HandleValue,
    path: UserSlice,
) -> Result<(), VfsServiceError> {
    let directory = process.resolve_handle::<DirectoryObject>(root, NAMESPACE_RIGHTS)?;
    let path = copy_path(process, path)?;
    directory
        .object()
        .mount_ramfs(&path, &process.resource_domain())
        .map_err(Into::into)
}

pub(super) fn switch_root(
    process: &Process,
    root: HandleValue,
    request: UserSlice,
) -> Result<(), VfsServiceError> {
    let directory = process.resolve_handle::<DirectoryObject>(root, NAMESPACE_RIGHTS)?;
    let mut header = [0u8; 48];
    process.copy_from_user(request, &mut header)?;
    let words = decode_words::<6>(&header)?;
    let count = usize::try_from(words[5]).map_err(|_| VfsServiceError::InvalidInput)?;
    if count > MAX_MOVES || (count == 0 && words[4] != 0) {
        return Err(VfsServiceError::InvalidInput);
    }
    let new_root = copy_path(process, user_slice(words[0], words[1])?)?;
    let put_old = copy_path(process, user_slice(words[2], words[3])?)?;
    let mut wire_moves = [0u8; MAX_MOVES * 32];
    let bytes = count.checked_mul(32).ok_or(VfsServiceError::InvalidInput)?;
    if count != 0 {
        process.copy_from_user(
            user_slice(words[4], bytes as u64)?,
            &mut wire_moves[..bytes],
        )?;
    }
    let mut owned = core::array::from_fn::<_, MAX_MOVES, _>(|_| None);
    for (slot, record) in owned.iter_mut().zip(wire_moves[..bytes].chunks_exact(32)) {
        let fields = decode_words::<4>(record)?;
        *slot = Some((
            copy_path(process, user_slice(fields[0], fields[1])?)?,
            copy_path(process, user_slice(fields[2], fields[3])?)?,
        ));
    }
    let mut moves = [("", ""); MAX_MOVES];
    for (output, value) in moves.iter_mut().zip(owned.iter()).take(count) {
        let (source, target) = value.as_ref().ok_or(VfsServiceError::InvalidInput)?;
        *output = (source.as_str(), target.as_str());
    }
    // All fallible user reads are complete before the VFS prepares or publishes
    // topology. Concurrent userspace writes cannot replace paths after validation.
    directory
        .object()
        .switch_root(
            &new_root,
            &put_old,
            &moves[..count],
            &process.resource_domain(),
        )
        .map_err(Into::into)
}

fn user_slice(address: u64, length: u64) -> Result<UserSlice, VfsServiceError> {
    UserSlice::new(UserAddress::new(address), length).map_err(|_| VfsServiceError::InvalidInput)
}

fn decode_words<const N: usize>(bytes: &[u8]) -> Result<[u64; N], VfsServiceError> {
    if bytes.len() != N * 8 {
        return Err(VfsServiceError::InvalidInput);
    }
    let mut words = [0; N];
    for (word, bytes) in words.iter_mut().zip(bytes.chunks_exact(8)) {
        *word = u64::from_le_bytes(
            bytes
                .try_into()
                .map_err(|_| VfsServiceError::InvalidInput)?,
        );
    }
    Ok(words)
}
