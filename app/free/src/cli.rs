// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    about = "Display physical memory usage",
    after_help = "used = total - free - reclaimable; available = free + reclaimable (estimate).\nOwners partition managed allocations; kernel combines direct pages, heap and page tables\nminus reclaimable slab. buffered/cached combines file cache and reclaimable slab;\nits pinned/loading payloads remain in used. Reclaimable includes shrinker-managed\nindex/header backing; live pins may retain a minimum metadata set.\nguest includes VMO backing loaned to guests.\nused = reserved + kernel + user + guest + cache non-reclaimable + other.\nUse --bytes to reconcile exact totals without display rounding.\nRamfs capacity is logical payload capacity within kernel, not additional physical RAM.\nAn unavailable census is shown as —; it does not mean zero."
)]
pub struct Free {
    /// Report exact bytes instead of human-readable units.
    #[arg(short = 'b', long)]
    pub bytes: bool,
    /// Report whole KiB.
    #[arg(short = 'k', long, conflicts_with_all = ["bytes", "mebi", "gibi"])]
    pub kibi: bool,
    /// Report whole MiB.
    #[arg(short = 'm', long, conflicts_with_all = ["bytes", "gibi"])]
    pub mebi: bool,
    /// Report whole GiB.
    #[arg(short = 'g', long, conflicts_with = "bytes")]
    pub gibi: bool,
    /// Number of snapshots; the first is immediate.
    #[arg(short = 'c', long, default_value = "1")]
    pub count: std::num::NonZeroU32,
    /// Seconds between snapshots (0.1 to 60); use -c to request multiple snapshots.
    #[arg(short = 's', long, default_value = "1", value_parser = hyper_tool_args::parse_interval)]
    pub seconds: f64,
}

impl Free {
    pub fn quantity(&self, bytes: u64) -> String {
        let unit = if self.kibi {
            Some((1024, "KiB"))
        } else if self.mebi {
            Some((1024 * 1024, "MiB"))
        } else if self.gibi {
            Some((1024 * 1024 * 1024, "GiB"))
        } else {
            None
        };
        unit.map_or_else(
            || crate::format_bytes(bytes, self.bytes),
            |(divisor, label)| format!("{} {label}", bytes / divisor),
        )
    }
}

#[cfg(test)]
#[path = "../tests/cli.rs"]
mod tests;
