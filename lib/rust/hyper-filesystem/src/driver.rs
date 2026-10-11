// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bounded format hints used to select a trusted filesystem worker executable.
//! `NativeBlock` already identifies a logical volume; this does not scan partitions.

/// Whole-sector prefix covering the FAT boot sector and EROFS superblock magic.
pub const PROBE_BYTES: usize = 3 * 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
    Fat,
    Erofs,
}

impl Format {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Fat => "FAT",
            Self::Erofs => "EROFS",
        }
    }

    pub const fn worker_name(self) -> &'static str {
        match self {
            Self::Fat => "fs-fat",
            Self::Erofs => "fs-erofs",
        }
    }
}

/// Candidate recognition only. The selected worker must validate full geometry
/// and allocation metadata before publishing a mount. Recognition does not
/// imply that the corresponding worker is installed. Ambiguous hints are rejected.
pub fn identify(prefix: &[u8]) -> Option<Format> {
    // EROFS reserves the first KiB and stores its little-endian magic at the
    // start of the superblock. No EROFS implementation is linked into this probe.
    let erofs = prefix.get(1024..1028) == Some(&0xe0f5_e1e2u32.to_le_bytes());
    match (fat_geometry(prefix), erofs) {
        (true, false) => Some(Format::Fat),
        (false, true) => Some(Format::Erofs),
        _ => None,
    }
}

fn fat_geometry(prefix: &[u8]) -> bool {
    let Some(boot) = prefix.get(..512) else {
        return false;
    };
    if boot[510..512] != [0x55, 0xaa] {
        return false;
    }
    let word = |offset| u16::from_le_bytes([boot[offset], boot[offset + 1]]);
    let dword = |offset| {
        u32::from_le_bytes([
            boot[offset],
            boot[offset + 1],
            boot[offset + 2],
            boot[offset + 3],
        ])
    };
    // The informational FAT type string is optional and can be stale. Probe
    // plausible BPB geometry, leaving FAT type and full validation to the driver.
    let sector_bytes = word(11);
    let clusters = boot[13];
    let reserved = word(14);
    let fats = boot[16];
    let total = if word(19) != 0 {
        u32::from(word(19))
    } else {
        dword(32)
    };
    let fat_sectors = if word(22) != 0 {
        u32::from(word(22))
    } else {
        dword(36)
    };
    matches!(sector_bytes, 512 | 1024 | 2048 | 4096)
        && clusters.is_power_of_two()
        && clusters <= 128
        && reserved != 0
        && (1..=2).contains(&fats)
        && fat_sectors != 0
        && u64::from(total) > u64::from(reserved) + u64::from(fats) * u64::from(fat_sectors)
}
