// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Key {
    Char(char),
    Control(u8),
    Enter,
    Tab,
    Backspace,
    Delete,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
}

/// Incremental VT key decoder. Unknown sequences are consumed, not inserted as
/// text. Bracketed paste suppresses command controls, including save and quit.
#[derive(Default)]
pub struct Decoder {
    bytes: VecDeque<u8>,
    paste: bool,
    paste_cr: bool,
}
impl Decoder {
    pub fn feed(&mut self, bytes: &[u8]) {
        self.bytes.extend(bytes);
    }
    pub fn incomplete(&self) -> bool {
        !self.bytes.is_empty()
    }
    pub fn expire(&mut self) -> Option<Key> {
        let escape = self.bytes.len() == 1 && self.bytes.front() == Some(&27) && !self.paste;
        self.bytes.clear();
        escape.then_some(Key::Escape)
    }
    pub fn pop(&mut self) -> Option<Key> {
        loop {
            let byte = *self.bytes.front()?;
            // A pasted CRLF is one line break, even across input messages.
            // Outside paste mode, consecutive Enter keys remain independent.
            let after_cr = std::mem::take(&mut self.paste_cr);
            if self.paste && after_cr && byte == b'\n' {
                self.bytes.pop_front();
                continue;
            }
            if byte == 27 {
                let bytes = self.bytes.make_contiguous();
                if bytes.len() == 1 {
                    return None;
                }
                if matches!(bytes[1], b'[' | b'O') {
                    let Some(end) = bytes[2..]
                        .iter()
                        .position(|b| (0x40..=0x7e).contains(b))
                        .map(|n| n + 2)
                    else {
                        if bytes.len() >= 32 {
                            self.bytes.clear();
                        }
                        return None;
                    };
                    let sequence = &bytes[1..=end];
                    let key = match sequence {
                        b"[200~" => {
                            self.paste = true;
                            None
                        }
                        b"[201~" => {
                            self.paste = false;
                            None
                        }
                        _ if self.paste => None,
                        b"[A" | b"OA" => Some(Key::Up),
                        b"[B" | b"OB" => Some(Key::Down),
                        b"[C" | b"OC" => Some(Key::Right),
                        b"[D" | b"OD" => Some(Key::Left),
                        b"[H" | b"OH" | b"[1~" | b"[7~" => Some(Key::Home),
                        b"[F" | b"OF" | b"[4~" | b"[8~" => Some(Key::End),
                        b"[3~" => Some(Key::Delete),
                        b"[5~" => Some(Key::PageUp),
                        b"[6~" => Some(Key::PageDown),
                        _ => None,
                    };
                    self.bytes.drain(..=end);
                    if key.is_some() {
                        return key;
                    }
                    continue;
                }
                self.bytes.pop_front();
                if !self.paste {
                    return Some(Key::Escape);
                }
                continue;
            }
            if byte < 128 {
                self.bytes.pop_front();
                self.paste_cr = self.paste && byte == b'\r';
                let key = match byte {
                    b'\r' | b'\n' => Some(Key::Enter),
                    b'\t' => Some(Key::Tab),
                    32..=126 => Some(Key::Char(char::from(byte))),
                    _ if self.paste => None,
                    8 | 127 => Some(Key::Backspace),
                    0..=31 => Some(Key::Control(byte)),
                    _ => None,
                };
                if key.is_some() {
                    return key;
                }
                continue;
            }
            let length = match byte {
                0xc2..=0xdf => 2,
                0xe0..=0xef => 3,
                0xf0..=0xf4 => 4,
                _ => 1,
            };
            if self.bytes.len() < length {
                return None;
            }
            let bytes = self.bytes.make_contiguous();
            match std::str::from_utf8(&bytes[..length])
                .ok()
                .and_then(|s| s.chars().next())
            {
                Some(ch) => {
                    self.bytes.drain(..length);
                    return Some(Key::Char(ch));
                }
                None => {
                    self.bytes.pop_front();
                }
            }
        }
    }
}
