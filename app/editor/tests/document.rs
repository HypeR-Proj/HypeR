// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use crate::{Document, MAX_BYTES};
use hyper_text_ui::Key;
#[test]
fn editing_preserves_utf8_and_crlf() -> Result<(), &'static str> {
    let mut doc = Document::new("a界\r\nb\n".into())?;
    doc.key(Key::End, 20)?;
    doc.key(Key::Backspace, 20)?;
    assert_eq!(doc.text(), "a\r\nb\n");
    doc.key(Key::Delete, 20)?;
    assert_eq!(doc.text(), "ab\n");
    doc.key(Key::Enter, 20)?;
    assert_eq!(doc.text(), "a\r\nb\n");
    doc.key(Key::Home, 20)?;
    doc.key(Key::Backspace, 20)?;
    assert_eq!(doc.text(), "ab\n");
    assert!(doc.dirty);
    Ok(())
}
#[test]
fn vertical_movement_retains_display_column() -> Result<(), &'static str> {
    let mut doc = Document::new("a界b\nx\n123456".into())?;
    doc.key(Key::End, 2)?;
    assert_eq!(doc.column(), 4);
    doc.key(Key::Down, 2)?;
    assert_eq!(doc.column(), 1);
    doc.key(Key::Down, 2)?;
    assert_eq!(doc.column(), 4);
    doc.key(Key::PageUp, 2)?;
    assert_eq!(doc.row(), 0);
    assert_eq!(doc.column(), 4);
    Ok(())
}
#[test]
fn mixed_mutations_keep_line_index_and_cursor_consistent() -> Result<(), &'static str> {
    let mut doc = Document::new("one\r\ntwo\n界".into())?;
    let keys = [
        Key::Enter,
        Key::Char('界'),
        Key::Char('a'),
        Key::Left,
        Key::Right,
        Key::Backspace,
        Key::Delete,
        Key::Home,
        Key::End,
        Key::Up,
        Key::Down,
        Key::Tab,
        Key::PageUp,
        Key::PageDown,
    ];
    let mut state = 5_u32;
    for _ in 0..5000 {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        doc.key(keys[state as usize % keys.len()], 5)?;
        assert!(doc.text().is_char_boundary(doc.cursor));
        let lines: Vec<_> = doc.text().split('\n').collect();
        assert_eq!(lines.len(), doc.lines());
        for (n, line) in lines.iter().enumerate() {
            let expected = if n + 1 < lines.len() {
                line.strip_suffix('\r').unwrap_or(line)
            } else {
                line
            };
            assert_eq!(doc.line(n), expected);
        }
        assert!(doc.row() < doc.lines());
    }
    Ok(())
}
#[test]
fn limit_rejects_edit_without_losing_document() -> Result<(), &'static str> {
    let mut doc = Document::new("x".repeat(MAX_BYTES))?;
    assert!(doc.key(Key::Char('y'), 10).is_err());
    assert_eq!(doc.text().len(), MAX_BYTES);
    assert!(!doc.dirty);
    assert!(Document::new("binary\0".into()).is_err());
    Ok(())
}
#[test]
fn search_wraps() -> Result<(), &'static str> {
    let mut doc = Document::new("abc\ndef abc".into())?;
    assert!(doc.find("abc"));
    assert_eq!(doc.row(), 1);
    assert!(doc.find("abc"));
    assert_eq!(doc.cursor, 0);
    assert!(!doc.find("not here"));
    assert!(!doc.find(""));
    Ok(())
}
