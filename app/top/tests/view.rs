// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;
use clap::Parser;

#[test]
fn display_controls_do_not_resample_and_unknown_keys_are_ignored()
-> Result<(), Box<dyn std::error::Error>> {
    let mut view = View::new(&Top::try_parse_from(["top"])?);
    assert_eq!(view.key(Key::Char('x'), 10), Action::None);
    for key in ['H', 'I', 'R', 'i', 'T'] {
        assert_eq!(view.key(Key::Char(key), 10), Action::Redraw);
    }
    assert!(view.threads && view.normalize && view.reverse && view.idle_hide);
    assert_eq!(view.sort, Sort::Time);
    assert_eq!(view.key(Key::Char(' '), 10), Action::Refresh);
    assert_eq!(view.key(Key::Enter, 10), Action::Refresh);
    assert_eq!(view.key(Key::Control(3), 10), Action::Quit);
    Ok(())
}

#[test]
fn interval_prompt_validates_cancels_and_updates_without_exiting()
-> Result<(), Box<dyn std::error::Error>> {
    let mut view = View::new(&Top::try_parse_from(["top", "-d", "60"])?);
    view.key(Key::Char('d'), 10);
    view.key(Key::Char('0'), 10);
    assert_eq!(view.key(Key::Enter, 10), Action::Redraw);
    assert_eq!(view.delay, 60.0);
    assert!(!view.message.is_empty());
    view.key(Key::Char('s'), 10);
    for ch in "0.25".chars() {
        view.key(Key::Char(ch), 10);
    }
    assert_eq!(view.key(Key::Enter, 10), Action::Refresh);
    assert_eq!(view.delay, 0.25);
    view.key(Key::Char('d'), 10);
    assert_eq!(view.key(Key::Escape, 10), Action::Redraw);
    assert!(view.prompt.is_none());
    Ok(())
}
