// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::{Discipline, EventFlags, Record, Settings, output::column_after_byte};

impl Discipline {
    /// Always inspect input for flow controls even when ordinary queues are
    /// full. Loss is explicit and cannot indefinitely hide a subsequent START.
    pub fn input(&mut self, bytes: &[u8], now_ns: u64) {
        for &original in bytes {
            if self.literal {
                self.literal = false;
                self.accept(original, true, now_ns);
                continue;
            }
            let original = if self.settings.input.strip_high_bit {
                original & 0x7f
            } else {
                original
            };
            let original = if self.settings.input.ascii_lowercase {
                original.to_ascii_lowercase()
            } else {
                original
            };
            if self.settings.input.receive_flow_control {
                if Some(original) == self.settings.controls.stop {
                    self.received_stop = true;
                    continue;
                }
                if Some(original) == self.settings.controls.start {
                    if self.received_stop {
                        self.events.0 |= EventFlags::OUTPUT_RESUMED;
                    }
                    self.received_stop = false;
                    continue;
                }
                if self.settings.input.resume_on_any {
                    if self.received_stop {
                        self.events.0 |= EventFlags::OUTPUT_RESUMED;
                    }
                    self.received_stop = false;
                }
            }
            let byte = match original {
                b'\r' if self.settings.input.ignore_cr => continue,
                b'\r' if self.settings.input.cr_to_lf => b'\n',
                b'\n' if self.settings.input.lf_to_cr => b'\r',
                byte => byte,
            };
            if self.settings.canonical && self.control(byte) {
                continue;
            }
            self.accept(byte, false, now_ns);
        }
        self.update_flow();
    }

    fn control(&mut self, byte: u8) -> bool {
        let controls = self.settings.controls;
        if Some(byte) == controls.literal_next {
            self.literal = true;
            return true;
        }
        if Some(byte) == controls.erase {
            self.erase();
            return true;
        }
        if Some(byte) == controls.kill {
            if self.settings.echo_kill_line {
                self.edit.clear();
                self.echo_byte(b'\n');
            } else {
                while !self.edit.is_empty() {
                    self.erase();
                }
            }
            self.discard_line = false;
            return true;
        }
        if Some(byte) == controls.word_erase {
            while self.edit.last().is_some_and(u8::is_ascii_whitespace) {
                self.erase();
            }
            while self.edit.last().is_some_and(|b| !b.is_ascii_whitespace()) {
                self.erase();
            }
            return true;
        }
        if Some(byte) == controls.reprint {
            self.echo_byte(b'\n');
            // Copy one byte at a time to avoid allocating from the input path.
            for index in 0..self.edit.len() {
                self.echo_byte(self.edit[index]);
            }
            return true;
        }
        if Some(byte) == controls.eof {
            if !self.discard_line {
                self.commit_line(true);
            } else {
                self.discard_line = false;
                self.edit.clear();
            }
            return true;
        }
        false
    }

    fn accept(&mut self, byte: u8, quoted: bool, now_ns: u64) {
        if self.settings.canonical {
            let delimiter =
                !quoted && (byte == b'\n' || Some(byte) == self.settings.controls.end_of_line);
            if self.discard_line {
                if delimiter {
                    self.discard_line = false;
                }
                return;
            }
            if self.edit.len() == self.limits.line_bytes
                || self.buffered_input() == self.limits.input_bytes
                || self.records.len() == self.limits.records
            {
                self.events.0 |= EventFlags::INPUT_LOSS;
                self.edit.clear();
                self.discard_line = !delimiter;
                return;
            }
            if self.edit.is_empty() {
                self.edit_start_column = self.column;
            }
            self.edit.push(byte);
            self.echo_byte(byte);
            if delimiter {
                self.commit_line(false);
            }
        } else {
            if self.input.len() == self.limits.input_bytes {
                self.events.0 |= EventFlags::INPUT_LOSS;
                return;
            }
            if !self.records.back().is_some_and(|r| r.open) {
                if self.records.len() == self.limits.records {
                    self.events.0 |= EventFlags::INPUT_LOSS;
                    return;
                }
                self.records.push_back(Record {
                    remaining: 0,
                    open: true,
                });
            }
            self.input.push_back(byte);
            if let Some(record) = self.records.back_mut() {
                record.remaining += 1;
            }
            self.echo_byte(byte);
        }
        self.last_input_ns = now_ns;
        self.input_generation = self.input_generation.wrapping_add(1);
    }

    pub(super) fn commit_line(&mut self, allow_empty: bool) {
        if self.edit.is_empty() && !allow_empty {
            return;
        }
        if self.records.len() == self.limits.records {
            self.events.0 |= EventFlags::INPUT_LOSS;
            self.edit.clear();
            return;
        }
        let remaining = self.edit.len();
        self.input.extend(self.edit.drain(..));
        self.records.push_back(Record {
            remaining,
            open: false,
        });
    }

    fn erase(&mut self) {
        if self.edit.is_empty() {
            return;
        }
        let end = self.edit.len();
        let mut start = end - 1;
        if self.settings.utf8 {
            while start > 0 && self.edit[start] & 0xc0 == 0x80 {
                start -= 1;
            }
            if core::str::from_utf8(&self.edit[start..]).is_err() {
                start = end - 1;
            }
        }
        let width = visual_columns(&self.edit[..end], self.edit_start_column, self.settings)
            .saturating_sub(visual_columns(
                &self.edit[..start],
                self.edit_start_column,
                self.settings,
            ));
        self.edit.truncate(start);
        if self.settings.echo && self.settings.visual_erase {
            for _ in 0..width.max(1) {
                self.echo_raw(b"\x08 \x08");
            }
            self.column = self.column.saturating_sub(width);
        }
    }

    fn echo_byte(&mut self, byte: u8) {
        if !self.settings.echo && !(self.settings.echo_newline && byte == b'\n') {
            return;
        }
        if self.settings.echo_controls
            && (byte < 32 || byte == 127)
            && !matches!(byte, b'\n' | b'\t')
        {
            self.echo_raw(&[b'^', if byte == 127 { b'?' } else { byte + 64 }]);
            self.column = self.column.saturating_add(2);
        } else {
            if self.write_with_limit(&[byte], self.echo_limit()) == 0 {
                self.events.0 |= EventFlags::ECHO_LOSS;
            }
        }
    }

    fn echo_raw(&mut self, bytes: &[u8]) {
        if bytes.len() > self.echo_limit().saturating_sub(self.output.len()) {
            self.events.0 |= EventFlags::ECHO_LOSS;
        } else {
            self.output.extend(bytes);
        }
    }
}

fn visual_columns(bytes: &[u8], initial: usize, settings: Settings) -> usize {
    let mut column = initial;
    for &byte in bytes {
        if settings.echo_controls && (byte < 32 || byte == 127) && !matches!(byte, b'\n' | b'\t') {
            column = column.saturating_add(2);
            continue;
        }
        if byte == b'\n' && settings.output.lf_to_crlf {
            column = 0;
        }
        let byte = if byte == b'\r' && settings.output.cr_to_lf {
            b'\n'
        } else {
            byte
        };
        column = column_after_byte(column, byte, settings);
    }
    column
}
