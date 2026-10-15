// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Small UTF-8 document model, independent of terminal and filesystem I/O.

#[cfg(target_os = "hyper")]
extern crate hyper_text_ui_shared as hyper_text_ui;
use hyper_text_ui::{Key, cells};
use std::ops::Range;
pub mod storage;
pub const MAX_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_LINES: usize = 131_072;

pub struct Document {
    text: String,
    starts: Vec<usize>,
    pub cursor: usize,
    pub dirty: bool,
    eol: &'static str,
    goal: Option<usize>,
}
impl Document {
    pub fn new(text: String) -> Result<Self, &'static str> {
        if text.len() > MAX_BYTES || text.contains('\0') {
            return Err("expected UTF-8 text without NUL, at most 4 MiB");
        }
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        if starts.len() > MAX_LINES {
            return Err("too many lines (limit 131072)");
        }
        let eol = if text
            .find('\n')
            .is_some_and(|n| n > 0 && text.as_bytes()[n - 1] == b'\r')
        {
            "\r\n"
        } else {
            "\n"
        };
        Ok(Self {
            text,
            starts,
            cursor: 0,
            dirty: false,
            eol,
            goal: None,
        })
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn lines(&self) -> usize {
        self.starts.len()
    }
    pub fn row(&self) -> usize {
        self.starts
            .partition_point(|p| *p <= self.cursor)
            .saturating_sub(1)
    }
    fn range(&self, row: usize) -> Range<usize> {
        let start = self.starts[row];
        let mut end = self.starts.get(row + 1).map_or(self.text.len(), |n| n - 1);
        if end > start
            && self.starts.get(row + 1).is_some()
            && self.text.as_bytes()[end - 1] == b'\r'
        {
            end -= 1;
        }
        start..end
    }
    pub fn line(&self, row: usize) -> &str {
        self.starts
            .get(row)
            .map_or("", |_| &self.text[self.range(row)])
    }
    pub fn column(&self) -> usize {
        cells(&self.text[self.starts[self.row()]..self.cursor])
    }
    fn replace(&mut self, range: Range<usize>, value: &str) -> Result<(), &'static str> {
        if self.text.len() - range.len() + value.len() > MAX_BYTES {
            return Err("document limit: 4 MiB");
        }
        let before = self.starts.partition_point(|p| *p <= range.start);
        let after = self.starts.partition_point(|p| *p <= range.end);
        let added = value.bytes().filter(|b| *b == b'\n').count();
        if self.starts.len() - (after - before) + added > MAX_LINES {
            return Err("line count limit reached");
        }
        self.starts.drain(before..after);
        for offset in &mut self.starts[before..] {
            *offset = *offset - range.len() + value.len();
        }
        self.starts.splice(
            before..before,
            value.match_indices('\n').map(|(n, _)| range.start + n + 1),
        );
        self.cursor = range.start + value.len();
        self.text.replace_range(range, value);
        self.dirty = true;
        self.goal = None;
        Ok(())
    }
    fn vertical(&mut self, target: usize) {
        let goal = self.goal.unwrap_or_else(|| self.column());
        let row = target.min(self.lines() - 1);
        self.cursor = self.starts[row] + hyper_text_ui::byte_at(self.line(row), goal);
        self.goal = Some(goal);
    }
    pub fn key(&mut self, key: Key, page: usize) -> Result<(), &'static str> {
        let row = self.row();
        let range = self.range(row);
        match key {
            Key::Char(ch) => {
                let mut bytes = [0; 4];
                self.replace(self.cursor..self.cursor, ch.encode_utf8(&mut bytes))?;
            }
            Key::Tab => self.replace(self.cursor..self.cursor, "\t")?,
            Key::Enter => self.replace(self.cursor..self.cursor, self.eol)?,
            Key::Backspace if self.cursor > 0 => {
                let start = if self.cursor == range.start {
                    self.range(row - 1).end
                } else {
                    self.text[..self.cursor]
                        .char_indices()
                        .next_back()
                        .map_or(0, |(n, _)| n)
                };
                self.replace(start..self.cursor, "")?;
            }
            Key::Delete if self.cursor < self.text.len() => {
                let end = if self.cursor == range.end {
                    self.starts[row + 1]
                } else {
                    self.cursor
                        + self.text[self.cursor..]
                            .chars()
                            .next()
                            .map_or(0, char::len_utf8)
                };
                self.replace(self.cursor..end, "")?;
            }
            Key::Up => self.vertical(row.saturating_sub(1)),
            Key::Down => self.vertical(row + 1),
            Key::PageUp => self.vertical(row.saturating_sub(page)),
            Key::PageDown => self.vertical(row.saturating_add(page)),
            Key::Home | Key::Control(1) => {
                self.cursor = range.start;
                self.goal = None;
            }
            Key::End | Key::Control(5) => {
                self.cursor = range.end;
                self.goal = None;
            }
            Key::Left if self.cursor > 0 => {
                self.cursor = if self.cursor == range.start {
                    self.range(row - 1).end
                } else {
                    self.text[..self.cursor]
                        .char_indices()
                        .next_back()
                        .map_or(0, |(n, _)| n)
                };
                self.goal = None;
            }
            Key::Right if self.cursor < self.text.len() => {
                self.cursor = if self.cursor == range.end {
                    self.starts[row + 1]
                } else {
                    self.cursor
                        + self.text[self.cursor..]
                            .chars()
                            .next()
                            .map_or(0, char::len_utf8)
                };
                self.goal = None;
            }
            _ => {}
        }
        Ok(())
    }
    pub fn find(&mut self, query: &str) -> bool {
        if query.is_empty() {
            return false;
        }
        let start = self.text[self.cursor..]
            .chars()
            .next()
            .map_or(self.cursor, |c| self.cursor + c.len_utf8());
        let found = self.text[start..]
            .find(query)
            .map(|n| start + n)
            .or_else(|| self.text[..start].find(query));
        if let Some(at) = found {
            self.cursor = at;
            self.goal = None;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
#[path = "../tests/document.rs"]
mod document_tests;

#[cfg(test)]
#[path = "../tests/storage.rs"]
mod storage_tests;

/// Command-line arguments for the formal text tool.
#[derive(clap::Parser)]
#[command(
    name = "editor",
    about = "Edit a UTF-8 text file",
    after_help = "UTF-8 text, up to 4 MiB. Ctrl-S save, Ctrl-O save as, Ctrl-F find, Ctrl-Q quit.\nArrows, Home/End and Page Up/Page Down navigate."
)]
pub struct Args {
    /// File to open (omit for an unnamed buffer).
    pub file: Option<std::path::PathBuf>,
}
