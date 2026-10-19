// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-scoped physical-memory summary.

// Select the shared parser implementation for Native delivery.
#[cfg(target_os = "hyper")]
extern crate hyper_clap_shared as _;

use clap::Parser;
use hyper_os::inspect::MemoryInspector;
use hyper_os::startup;
use std::io::Write;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = hyper_free::cli::Free::parse();
    let mut startup = hyper_rt::process::startup()?;
    hyper_os::require_core_abi()?;
    let inspector = MemoryInspector::from_handle(startup.take(startup::MEMORY_INSPECTOR)?);
    let mut output = std::io::stdout().lock();
    for sample in 0..args.count.get() {
        if sample != 0 {
            std::thread::sleep(std::time::Duration::from_secs_f64(args.seconds));
            writeln!(output)?;
        }
        snapshot(&args, inspector.read()?, &mut output)?;
        output.flush()?;
    }
    Ok(())
}

fn snapshot(
    args: &hyper_free::cli::Free,
    observation: hyper_os::inspect::MemoryObservation,
    output: &mut impl Write,
) -> std::io::Result<()> {
    let quantity = |bytes| args.quantity(bytes);
    let summary = hyper_free::report::Summary::from_observation(&observation);
    let estimate = |bytes: Option<u64>| bytes.map_or_else(|| String::from("—"), quantity);
    writeln!(
        output,
        "          {:>11}  {:>11}  {:>11}  {:>11}  {:>11}",
        "total", "used", "free", "reclaimable", "available"
    )?;
    writeln!(
        output,
        "Mem:      {:>11}  {:>11}  {:>11}  {:>11}  {:>11}",
        quantity(observation.total_bytes),
        estimate(summary.used),
        quantity(observation.free_bytes),
        estimate(summary.reclaimable),
        estimate(summary.available),
    )?;
    writeln!(
        output,
        "Reserved: {}  Managed: {}  Allocated: {}",
        quantity(summary.reserved),
        quantity(observation.managed_bytes),
        quantity(summary.allocated),
    )?;
    writeln!(
        output,
        "Owners:   kernel={} user={} guest={} buffered/cached={} other={}",
        estimate(summary.kernel),
        quantity(summary.user),
        quantity(summary.guest),
        estimate(summary.buffered_cached),
        quantity(summary.other),
    )?;
    writeln!(
        output,
        "Cache:    file-reclaimable={} slab-reclaimable={}",
        estimate(observation.file_cache_reclaimable_bytes),
        estimate(observation.slab_reclaimable_bytes),
    )?;
    writeln!(
        output,
        "          non-reclaimable={} (included in used and buffered/cached)",
        estimate(summary.cache_non_reclaimable),
    )?;
    writeln!(
        output,
        "Ramfs:    capacity={} (logical; included in kernel)",
        quantity(observation.ramfs_capacity_bytes),
    )?;
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("free: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
