// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Temporary kernel-owned bytes for the restricted bootstrap ELF parser.

use alloc::borrow::Cow;
use alloc::vec::Vec;

use crate::kernel::accounting::{ResourceAmount, ResourceDomain, ResourceError, ResourceKind};

use super::instance::Error;

/// A coherent parser input retained only for the kernel loading transaction.
///
/// Mutable or remote contents are copied under the canonical content gate and
/// charged to the requesting process. Immutable boot archive bytes can be
/// borrowed. Neither representation is published as a userspace VMO.
pub(crate) struct ExecutableSnapshot {
    bytes: Cow<'static, [u8]>,
    _charge: Option<crate::kernel::accounting::CommittedCharge>,
}

impl ExecutableSnapshot {
    pub(super) fn read(
        length: u64,
        sponsor: &ResourceDomain,
        read: impl FnOnce(&mut [u8]) -> Result<usize, Error>,
    ) -> Result<Self, Error> {
        let (mut bytes, charge) = parser_buffer(length, sponsor)?;
        if read(&mut bytes)? != bytes.len() {
            return Err(Error::InvalidBackendResult);
        }
        Ok(Self::owned(bytes, charge))
    }

    pub(super) fn borrowed(bytes: &'static [u8]) -> Self {
        Self {
            bytes: Cow::Borrowed(bytes),
            _charge: None,
        }
    }

    pub(super) fn owned(
        bytes: Vec<u8>,
        charge: crate::kernel::accounting::CommittedCharge,
    ) -> Self {
        Self {
            bytes: Cow::Owned(bytes),
            _charge: Some(charge),
        }
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

fn parser_buffer(
    length: u64,
    sponsor: &ResourceDomain,
) -> Result<(Vec<u8>, crate::kernel::accounting::CommittedCharge), Error> {
    let size = usize::try_from(length).map_err(|_| Error::InvalidSize)?;
    let charge = crate::kernel::mm::reclaim::retry_prepare_with(
        || {
            sponsor
                .reserve(ResourceAmount::ZERO.with(ResourceKind::KernelMemoryBytes, length))
                .map(|reservation| reservation.commit())
        },
        |error| match error {
            ResourceError::LimitExceeded { domain, .. } => {
                Some(crate::kernel::mm::reclaim::Target::Domain(*domain))
            }
            _ => None,
        },
    )
    .map_err(Error::Resource)?;
    let mut bytes = Vec::new();
    crate::kernel::mm::reclaim::reserve_exact(&mut bytes, size).map_err(|_| Error::Allocation)?;
    bytes.resize(size, 0);
    Ok((bytes, charge))
}
