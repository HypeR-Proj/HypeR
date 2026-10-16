// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

// Select the shared parser implementation for Native delivery.
#[cfg(target_os = "hyper")]
extern crate hyper_clap_shared as _;

use clap::Parser;
use hyper_ls::cli::Ls;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

fn run() -> io::Result<bool> {
    let mut args = Ls::parse();
    if args.paths.is_empty() {
        args.paths.push(PathBuf::from("."));
    }
    let mut output = io::stdout().lock();
    let mut errors = io::stderr().lock();
    let mut failed = false;
    for (index, path) in args.paths.iter().enumerate() {
        if args.paths.len() > 1 {
            if index != 0 {
                writeln!(output)?;
            }
            writeln!(output, "{}:", path.display())?;
        }
        match hyper_ls::listing::list(path, &args, &mut output, &mut errors) {
            Ok(partial_failure) => failed |= partial_failure,
            Err(error) => {
                if error.kind() == io::ErrorKind::BrokenPipe {
                    return Err(error);
                }
                writeln!(errors, "ls: {}: {error}", path.display())?;
                failed = true;
            }
        }
    }
    output.flush()?;
    Ok(failed)
}

fn main() -> ExitCode {
    match run() {
        Ok(false) => ExitCode::SUCCESS,
        Ok(true) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("ls: {error}");
            ExitCode::FAILURE
        }
    }
}
