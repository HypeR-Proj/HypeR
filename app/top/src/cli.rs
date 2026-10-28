// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Sort {
    Cpu,
    Name,
    Koid,
    Time,
    Threads,
}

#[derive(Debug, Parser)]
#[command(about = "Monitor Native CPU, memory and processes")]
pub struct Top {
    /// Emit plain snapshots without terminal escape sequences or keyboard input.
    #[arg(short = 'b', long)]
    pub batch: bool,
    /// Exit after this many snapshots.
    #[arg(short = 'n', long)]
    pub iterations: Option<std::num::NonZeroU32>,
    /// Refresh period in seconds (0.1 to 60).
    #[arg(short = 'd', long, default_value = "1", value_parser = hyper_tool_args::parse_interval)]
    pub delay: f64,
    /// Show individual threads, including kernel threads when unfiltered.
    #[arg(short = 'H', long, alias = "threads-show")]
    pub threads: bool,
    /// Hide rows with no CPU activity in the sampled interval.
    #[arg(short = 'i', long)]
    pub idle_hide: bool,
    /// Divide task CPU usage by total host CPU time (100% for the whole machine).
    #[arg(long)]
    pub normalize: bool,
    /// Reverse the selected row order.
    #[arg(short = 'R', long)]
    pub reverse: bool,
    #[command(flatten)]
    pub filter: hyper_tool_args::ProcessFilter,
    /// Order process rows; ties are resolved by KOID.
    #[arg(short = 'o', long, value_enum, default_value = "cpu")]
    pub sort: Sort,
    /// Show at most this many matching process rows (summary remains system-wide).
    #[arg(short = 'l', long)]
    pub limit: Option<std::num::NonZeroUsize>,
}

#[cfg(test)]
#[path = "../tests/cli.rs"]
mod tests;
