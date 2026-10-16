// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use std::fmt::Write;
use unicode_width::UnicodeWidthChar;

fn cell_width(ch: char, column: usize) -> usize {
    match ch {
        '\t' => 8 - column % 8,
        '\0'..='\x1f' | '\x7f' => 2,
        '\u{80}'..='\u{9f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' => 1,
        _ => ch.width().unwrap_or(0),
    }
}
fn push_glyph(output: &mut String, ch: char, width: usize) {
    match ch {
        '\t' => output.extend(std::iter::repeat_n(' ', width)),
        '\0'..='\x1f' => {
            output.push('^');
            output.push(char::from(ch as u8 + 64));
        }
        '\x7f' => output.push_str("^?"),
        '\u{80}'..='\u{9f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' => output.push('?'),
        _ => output.push(ch),
    }
}

pub fn cells(text: &str) -> usize {
    text.chars()
        .fold(0, |column, ch| column + cell_width(ch, column))
}

pub fn byte_at(text: &str, target: usize) -> usize {
    let mut column = 0;
    for (offset, ch) in text.char_indices() {
        let width = cell_width(ch, column);
        if column + width > target {
            return offset;
        }
        column += width;
    }
    text.len()
}

/// Render untrusted text as cells, never as terminal instructions. Tabs use
/// original columns; clipping through a wide character leaves blank cells.
pub fn visible(text: &str, offset: usize, width: usize) -> String {
    let mut result = String::new();
    let mut column = 0;
    for ch in text.chars() {
        let count = cell_width(ch, column);
        if column >= offset.saturating_add(width) && count != 0 {
            break;
        }
        if count == 0 {
            if column > offset && !result.is_empty() {
                push_glyph(&mut result, ch, count);
            }
        } else if column >= offset && column + count <= offset.saturating_add(width) {
            push_glyph(&mut result, ch, count);
        } else {
            let left = column.max(offset);
            let right = (column + count).min(offset.saturating_add(width));
            if right > left {
                result.extend(std::iter::repeat_n(' ', right - left));
            }
        }
        column += count;
    }
    result
}

/// One buffered frame; reserve the physical last column to avoid autowrap.
pub struct Screen {
    bytes: String,
    width: usize,
}
impl Screen {
    pub fn new(columns: usize) -> Self {
        Self {
            bytes: "\x1b[?25l\x1b[H".into(),
            width: columns.saturating_sub(1),
        }
    }
    pub fn row(&mut self, row: usize, text: &str, offset: usize, inverted: bool) {
        let _ = write!(self.bytes, "\x1b[{};1H\x1b[0m\x1b[K", row + 1);
        if inverted {
            self.bytes.push_str("\x1b[7m");
        }
        let text = visible(text, offset, self.width);
        self.bytes.push_str(&text);
        if inverted {
            // Pad by display cells so wide and combining characters align too.
            self.bytes.extend(std::iter::repeat_n(
                ' ',
                self.width.saturating_sub(cells(&text)),
            ));
        }
        self.bytes.push_str("\x1b[0m");
    }
    pub fn cursor(&mut self, row: usize, column: usize) {
        let _ = write!(
            self.bytes,
            "\x1b[{};{}H\x1b[?25h",
            row + 1,
            column.min(self.width) + 1
        );
    }
    pub fn bytes(&self) -> &[u8] {
        self.bytes.as_bytes()
    }
}
