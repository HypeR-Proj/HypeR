// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use crate::{Content, MAX_BYTES, MAX_LINES};
#[test]
fn fragmented_input_preserves_bytes_and_lines() -> Result<(), &'static str> {
    let mut content = Content::default();
    assert_eq!(content.lines(), 0);
    for byte in "one\r\n界\nlast".as_bytes() {
        content.append(&[*byte])?;
    }
    assert_eq!(content.lines(), 3);
    assert_eq!(content.line(0), "one");
    assert_eq!(content.line(1), "界");
    assert_eq!(content.line(2), "last");
    content.append(b"\n")?;
    assert_eq!(content.lines(), 3);
    assert_eq!(content.find(2, "one"), Some(0));
    Ok(())
}
#[test]
fn binary_data_is_retained_for_safe_rendering() -> Result<(), &'static str> {
    let mut content = Content::default();
    content.append(b"\xff\x1b[2J\0")?;
    assert_eq!(hyper_text_ui::visible(&content.line(0), 0, 40), "�^[[2J^@");
    Ok(())
}
#[test]
fn bounds_fail_without_losing_previous_data() -> Result<(), &'static str> {
    let mut content = Content::default();
    content.append(&vec![b'x'; MAX_BYTES])?;
    assert!(content.append(b"y").is_err());
    assert_eq!(content.bytes(), MAX_BYTES);
    let mut lines = Content::default();
    lines.append(&vec![b'\n'; MAX_LINES - 1])?;
    assert!(lines.append(b"\n").is_err());
    assert_eq!(lines.lines(), MAX_LINES - 1);
    Ok(())
}

#[test]
fn standalone_carriage_return_remains_visible() -> Result<(), &'static str> {
    let mut content = Content::default();
    content.append(b"first\r\nlast\r")?;
    assert_eq!(content.line(0), "first");
    assert_eq!(hyper_text_ui::visible(&content.line(1), 0, 40), "last^M");
    Ok(())
}
