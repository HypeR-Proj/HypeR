// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(about = "Inspect Native ELF shared-library dependencies without executing code")]
#[command(
    after_help = "Uses /system/lib/<arch>-hyper-hyper unless --library-dir is given. Native PT_INTERP identities resolve to the formal loader in /system/lib64; --loader selects another file explicitly. Does not resolve symbols or validate relocations; successful inspection does not guarantee that an image can run."
)]
pub struct Ldd {
    /// Show which object requires each library; repeated objects are not expanded again.
    #[arg(long, conflicts_with = "direct")]
    pub tree: bool,
    /// Inspect only the input's immediate dependencies and interpreter.
    #[arg(long)]
    pub direct: bool,
    /// Include architecture, ELF ABI and SONAME metadata.
    #[arg(short, long)]
    pub verbose: bool,
    /// Inspect an alternate library directory, without changing runtime loader policy.
    #[arg(long, value_name = "DIR")]
    pub library_dir: Option<PathBuf>,
    /// Check an explicitly selected executable loader, including for static programs.
    #[arg(long, value_name = "FILE")]
    pub loader: Option<PathBuf>,
    /// Executables or shared objects to inspect.
    #[arg(required = true, value_name = "FILE")]
    pub files: Vec<PathBuf>,
}
