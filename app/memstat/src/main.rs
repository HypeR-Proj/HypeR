// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

#[cfg(target_os = "hyper")]
extern crate hyper_clap_shared as _;

use clap::Parser;
use hyper_memstat::{capture, snapshot::Snapshot};
use hyper_os::{inspect::MemoryInspector, startup};
use std::io::Write;
use std::path::PathBuf;

/// Inspect allocation traffic, retained pages and cache metadata.
#[derive(Parser)]
#[command(
    name = "memstat",
    after_help = "Capture a baseline: memstat --save /tmp/before.json\nAfter the workload: memstat --diff /tmp/before.json --classes\nCompare only within the same boot. Reading diagnostics never forces reclamation."
)]
struct Args {
    /// Save the full sample as JSON (create a new file; never overwrite).
    #[arg(long, value_name = "PATH")]
    save: Option<PathBuf>,
    /// Compare against a saved sample from this boot; show changed metrics.
    #[arg(long, value_name = "PATH", conflicts_with = "json")]
    diff: Option<PathBuf>,
    /// Include per-pool/size-class metrics in the text report.
    #[arg(long)]
    classes: bool,
    /// Print the full machine-readable sample.
    #[arg(long)]
    json: bool,
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let mut startup = hyper_rt::process::startup()?;
    hyper_os::require_core_abi()?;
    let inspector = MemoryInspector::from_handle(startup.take(startup::MEMORY_INSPECTOR)?);
    // Sample before reading/allocating the baseline or creating its destination.
    let snapshot = capture::capture(&inspector)?;
    let baseline = args
        .diff
        .as_ref()
        .map(|path| Snapshot::load(std::fs::File::open(path)?))
        .transpose()?;
    if let Some(baseline) = &baseline {
        snapshot.validate_baseline(baseline)?;
    }
    if let Some(path) = &args.save {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        serde_json::to_writer_pretty(&mut file, &snapshot)?;
        file.write_all(b"\n")?;
    }
    let mut out = std::io::stdout().lock();
    if args.json {
        serde_json::to_writer_pretty(&mut out, &snapshot)?;
        writeln!(out)?;
    } else {
        snapshot.print(baseline.as_ref(), args.classes, &mut out)?;
    }
    out.flush()?;
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("memstat: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
