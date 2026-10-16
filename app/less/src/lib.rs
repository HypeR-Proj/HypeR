// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bounded incremental pager history. Input bytes are never terminal commands.

#[cfg(target_os = "hyper")]
extern crate hyper_text_ui_shared as hyper_text_ui;
use std::borrow::Cow;
pub const MAX_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_LINES: usize = 262_144;
#[derive(Default)]
pub struct Content {
    bytes: Vec<u8>,
    starts: Vec<usize>,
}
impl Content {
    pub fn append(&mut self, bytes: &[u8]) -> Result<(), &'static str> {
        if self.bytes.len() + bytes.len() > MAX_BYTES {
            return Err("input exceeds 16 MiB history limit");
        }
        let lines = bytes.iter().filter(|b| **b == b'\n').count();
        if self.starts.len() + lines + usize::from(self.starts.is_empty()) > MAX_LINES {
            return Err("input exceeds 262144 line history limit");
        }
        if self.starts.is_empty() {
            self.starts.push(0);
        }
        let base = self.bytes.len();
        self.starts.extend(
            bytes
                .iter()
                .enumerate()
                .filter(|(_, b)| **b == b'\n')
                .map(|(i, _)| base + i + 1),
        );
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    pub fn lines(&self) -> usize {
        self.starts
            .len()
            .saturating_sub(usize::from(self.starts.last() == Some(&self.bytes.len())))
    }
    pub fn bytes(&self) -> usize {
        self.bytes.len()
    }
    pub fn line(&self, row: usize) -> Cow<'_, str> {
        let Some(&start) = self.starts.get(row) else {
            return Cow::Borrowed("");
        };
        let mut end = self.starts.get(row + 1).map_or(self.bytes.len(), |n| n - 1);
        if self.starts.get(row + 1).is_some() && end > start && self.bytes[end - 1] == b'\r' {
            end -= 1;
        }
        String::from_utf8_lossy(&self.bytes[start..end])
    }
    pub fn find(&self, from: usize, query: &str) -> Option<usize> {
        if query.is_empty() {
            return None;
        }
        (from..self.lines())
            .chain(0..from.min(self.lines()))
            .find(|row| self.line(*row).contains(query))
    }
}

#[cfg(test)]
#[path = "../tests/content.rs"]
mod content_tests;

/// Command-line arguments for the formal text tool.
#[derive(clap::Parser)]
#[command(
    name = "less",
    about = "Page through a file or piped input",
    after_help = "Page Up/b and Page Down/Space flip pages; arrows scroll; g/G first/last; / search; n next; q quit.\nHistory: 16 MiB / 262144 lines. Long lines scroll horizontally.\nRedirected stdout copies input without formatting."
)]
pub struct Args {
    /// File to open (omit or use - for stdin).
    pub file: Option<std::path::PathBuf>,
}
