// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use hyper_text_ui::{Decoder, Key, byte_at, cells, visible};
#[test]
fn fragmented_keys_and_utf8() {
    let mut decoder = Decoder::default();
    for byte in b"\x1b[6" {
        decoder.feed(&[*byte]);
        assert_eq!(decoder.pop(), None);
    }
    decoder.feed(b"~");
    assert_eq!(decoder.pop(), Some(Key::PageDown));
    let wide = "界".as_bytes();
    decoder.feed(&wide[..2]);
    assert_eq!(decoder.pop(), None);
    decoder.feed(&wide[2..]);
    assert_eq!(decoder.pop(), Some(Key::Char('界')));
    decoder.feed(b"\x1b");
    assert_eq!(decoder.pop(), None);
    assert_eq!(decoder.expire(), Some(Key::Escape));
    decoder.feed(b"\xffx\x1b[99~y");
    assert_eq!(decoder.pop(), Some(Key::Char('x')));
    assert_eq!(decoder.pop(), Some(Key::Char('y')));
}
#[test]
fn paste_cannot_save_or_quit() {
    let mut decoder = Decoder::default();
    decoder.feed(b"\x1b[200~a\x11\x13\n\tb\x1b[201~\x13");
    let mut keys = Vec::new();
    while let Some(key) = decoder.pop() {
        keys.push(key);
    }
    assert_eq!(
        keys,
        vec![
            Key::Char('a'),
            Key::Enter,
            Key::Tab,
            Key::Char('b'),
            Key::Control(19)
        ]
    );
}
#[test]
fn pasted_crlf_is_one_newline_across_messages() {
    let mut decoder = Decoder::default();
    let mut keys = Vec::new();
    for byte in b"\x1b[200~a\r\nb\rc\n\x1b[201~\r\n" {
        decoder.feed(&[*byte]);
        while let Some(key) = decoder.pop() {
            keys.push(key);
        }
    }
    assert_eq!(
        keys,
        vec![
            Key::Char('a'),
            Key::Enter,
            Key::Char('b'),
            Key::Enter,
            Key::Char('c'),
            Key::Enter,
            Key::Enter,
            Key::Enter
        ]
    );
}
#[test]
fn control_bytes_and_wide_clipping_are_safe() {
    assert_eq!(visible("\x1b[31m\x07x\u{009b}", 0, 40), "^[[31m^Gx?");
    assert_eq!(visible("a界b", 2, 2), " b");
    assert_eq!(visible("a\tb", 6, 3), "  b");
    assert_eq!(cells("界\te\u{301}"), 9);
    assert_eq!(byte_at("界a", 1), 0);
    assert_eq!(byte_at("界a", 2), 3);
    assert_eq!(visible("界a", 0, 1), " ");
}
