// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Owned 39-bit IOVA address spaces using 4 KiB VMSAv8-64 stage-2 tables.

use super::{DmaBuffer, DmaMemory, Environment, Error, memory};
use alloc::vec::Vec;

const ADDRESS_MASK: u64 = 0x0000_ffff_ffff_f000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DomainId {
    pub(super) controller: u64,
    pub(super) serial: u64,
    pub(super) vmid: u16,
}

#[derive(Clone, Copy, Debug)]
pub enum Permissions {
    Read,
    Write,
    ReadWrite,
}

impl Permissions {
    fn descriptor(self) -> u64 {
        let access = match self {
            Self::Read => 1,
            Self::Write => 2,
            Self::ReadWrite => 3,
        };
        // Page, normal WB, S2AP, Inner Shareable, AF, execute-never.
        3 | (15 << 2) | (access << 6) | (3 << 8) | (1 << 10) | (1 << 54)
    }
}

pub(super) struct Mapping<M> {
    pub iova: u64,
    pub memory: M,
}
struct Table<M> {
    memory: M,
    // Parents precede children in the owner vector. Use physical identity so
    // removing acknowledged owners never invalidates the remaining links.
    parent: Option<(u64, usize)>,
    entries: u16,
}
pub(super) struct Domain<M, B> {
    pub id: DomainId,
    tables: Vec<Table<M>>,
    pub mappings: Vec<Mapping<B>>,
}

impl<M: DmaMemory, B: DmaBuffer> Domain<M, B> {
    pub(super) fn new<E: Environment<Memory = M>>(id: DomainId, bits: u8) -> Result<Self, Error> {
        let mut tables = Vec::new();
        tables.try_reserve_exact(1).map_err(|_| Error::Allocation)?;
        tables.push(Table {
            memory: memory::allocate::<E>(0, bits)?,
            parent: None,
            entries: 0,
        });
        Ok(Self {
            id,
            tables,
            mappings: Vec::new(),
        })
    }

    pub(super) fn root_address(&self) -> u64 {
        self.tables[0].memory.physical()
    }

    fn write_entry(&mut self, table: usize, index: usize, value: u64) -> Result<(), Error> {
        let page = &mut self.tables[table];
        let previous = memory::read(&page.memory, index);
        page.entries = match (previous == 0, value == 0) {
            (true, false) => page.entries.checked_add(1).filter(|count| *count <= 512),
            (false, true) => page.entries.checked_sub(1),
            _ => Some(page.entries),
        }
        .ok_or(Error::Corrupt)?;
        memory::write(&page.memory, index, value);
        Ok(())
    }

    /// Empty intermediate tables can survive allocation failure. They grant no
    /// DMA access and remain owned until a later unmap or domain retirement
    /// detaches them and acknowledges translation invalidation.
    fn leaf<E: Environment<Memory = M>>(
        &mut self,
        iova: u64,
        create: bool,
        bits: u8,
    ) -> Result<(usize, usize), Error> {
        if iova >= (1 << 39) || !iova.is_multiple_of(4096) {
            return Err(Error::Address);
        }
        let mut table = 0;
        for shift in [30, 21] {
            let index = ((iova >> shift) & 511) as usize;
            let entry = memory::read(&self.tables[table].memory, index);
            let next = if entry == 0 {
                if !create {
                    return Err(Error::NotMapped);
                }
                self.tables.try_reserve(1).map_err(|_| Error::Allocation)?;
                let page = memory::allocate::<E>(0, bits)?;
                let physical = page.physical();
                let next = self.tables.len();
                let parent = self.tables[table].memory.physical();
                self.tables.push(Table {
                    memory: page,
                    parent: Some((parent, index)),
                    entries: 0,
                });
                E::synchronize();
                self.write_entry(table, index, physical | 3)?;
                next
            } else {
                self.tables
                    .iter()
                    .position(|page| page.memory.physical() == entry & ADDRESS_MASK)
                    .ok_or(Error::Corrupt)?
            };
            table = next;
        }
        Ok((table, ((iova >> 12) & 511) as usize))
    }

    /// Validate and allocate all intermediate tables before publishing any leaf.
    /// Empty tables may remain after failure; they grant no access. One retained
    /// owner covers the complete mapping, avoiding one reference per RAM page.
    pub(super) fn map<E: Environment<Memory = M>>(
        &mut self,
        iova: u64,
        buffer: B,
        permissions: Permissions,
        bits: u8,
    ) -> Result<(), Error> {
        let length = buffer.length();
        let end = iova.checked_add(length).ok_or(Error::Address)?;
        if length == 0
            || !length.is_multiple_of(4096)
            || !iova.is_multiple_of(4096)
            || end > 1 << 39
        {
            return Err(Error::Address);
        }
        let position = self.mappings.partition_point(|map| map.iova < iova);
        if position > 0
            && self.mappings[position - 1].iova + self.mappings[position - 1].memory.length() > iova
            || self
                .mappings
                .get(position)
                .is_some_and(|map| map.iova < end)
        {
            return Err(Error::AlreadyMapped);
        }
        self.mappings
            .try_reserve(1)
            .map_err(|_| Error::Allocation)?;
        for offset in (0..length).step_by(4096) {
            let physical = buffer.physical_page(offset);
            if !physical.is_multiple_of(4096)
                || physical
                    .checked_add(4096)
                    .is_none_or(|end| end > 1u64 << bits)
            {
                return Err(Error::Address);
            }
            self.leaf::<E>(iova + offset, true, bits)?;
        }
        // No fallible allocation or metadata growth follows this ownership cut.
        self.mappings.insert(
            position,
            Mapping {
                iova,
                memory: buffer,
            },
        );
        for offset in (0..length).step_by(4096) {
            let (table, index) = self.leaf::<E>(iova + offset, false, bits)?;
            let entry =
                self.mappings[position].memory.physical_page(offset) | permissions.descriptor();
            E::synchronize();
            self.write_entry(table, index, entry)?;
        }
        Ok(())
    }

    pub(super) fn invalidate<E: Environment<Memory = M>>(
        &mut self,
        iova: u64,
        bits: u8,
    ) -> Result<usize, Error> {
        let mapping = self
            .mappings
            .binary_search_by_key(&iova, |map| map.iova)
            .map_err(|_| Error::NotMapped)?;
        for offset in (0..self.mappings[mapping].memory.length()).step_by(4096) {
            let (table, index) = match self.leaf::<E>(iova + offset, false, bits) {
                Ok(leaf) => leaf,
                // The test-only page revocation can empty a still-owned
                // mapping. Another unmap may already have pruned its subtree.
                #[cfg(all(feature = "kernel-test", feature = "io-dma-revoke-test"))]
                Err(Error::NotMapped) => continue,
                Err(error) => return Err(error),
            };
            self.write_entry(table, index, 0)?;
        }
        Ok(mapping)
    }

    /// Withdraw empty children bottom-up without allocating or freeing owners.
    /// The controller excludes map/unmap until its domain-wide TLBI completes;
    /// command failure keeps detached tables pinned alongside payload leases.
    pub(super) fn detach_empty_tables(&mut self) -> Result<(), Error> {
        for child in (1..self.tables.len()).rev() {
            if self.tables[child].entries != 0 {
                continue;
            }
            let Some((physical, slot)) = self.tables[child].parent else {
                continue;
            };
            let parent = self.tables[..child]
                .iter()
                .position(|page| page.memory.physical() == physical)
                .ok_or(Error::Corrupt)?;
            if memory::read(&self.tables[parent].memory, slot)
                != (self.tables[child].memory.physical() | 3)
            {
                return Err(Error::Corrupt);
            }
            self.write_entry(parent, slot, 0)?;
            self.tables[child].parent = None;
        }
        Ok(())
    }

    /// Called only after domain-wide invalidation and `CMD_SYNC` acknowledged
    /// every detached walk. The root remains owned while the domain is alive.
    pub(super) fn finish_unmap(&mut self, mapping: usize) -> B {
        let root = self.root_address();
        self.tables
            .retain(|page| page.parent.is_some() || page.memory.physical() == root);
        let buffer = self.mappings.remove(mapping).memory;
        compact(&mut self.tables);
        compact(&mut self.mappings);
        buffer
    }

    #[cfg(all(feature = "kernel-test", feature = "io-dma-revoke-test"))]
    pub(super) fn revoke_test_page<E: Environment<Memory = M>>(
        &mut self,
        iova: u64,
        bits: u8,
    ) -> Result<B, Error>
    where
        B: Clone,
    {
        if !iova.is_multiple_of(4096) {
            return Err(Error::Address);
        }
        let mapping = self
            .mappings
            .iter()
            .find(|mapping| iova >= mapping.iova && iova - mapping.iova < mapping.memory.length())
            .ok_or(Error::NotMapped)?;
        // Keep the production mapping owner AND an independent test owner.
        // Neither CPU access nor backing lifetime changes with this missing PTE.
        let retained = mapping.memory.clone();
        let (table, index) = self.leaf::<E>(iova, false, bits)?;
        if memory::read(&self.tables[table].memory, index) == 0 {
            return Err(Error::NotMapped);
        }
        self.write_entry(table, index, 0)?;
        Ok(retained)
    }
}

/// Bound spare metadata in long-lived domains without making completed
/// hardware retirement depend on allocation. Geometric shrink avoids copying
/// the remaining owners for every small buffer in a fragmented grant.
fn compact<T>(owners: &mut Vec<T>) {
    if owners.len() >= owners.capacity() / 2 {
        return;
    }
    let mut replacement = Vec::new();
    if replacement.try_reserve_exact(owners.len()).is_ok() {
        replacement.append(owners);
        *owners = replacement;
    }
}
