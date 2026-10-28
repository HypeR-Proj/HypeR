// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Interactive changes redraw the retained frame without disturbing sampling.
use crate::cli::{Sort, Top};
use hyper_text_ui::Key;

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    None,
    Redraw,
    Refresh,
    Quit,
}

pub struct View {
    pub sort: Sort,
    pub reverse: bool,
    pub threads: bool,
    pub idle_hide: bool,
    pub normalize: bool,
    pub delay: f64,
    pub help: bool,
    pub offset: usize,
    pub left: usize,
    pub prompt: Option<String>,
    pub message: String,
}
impl View {
    pub fn new(args: &Top) -> Self {
        Self {
            sort: args.sort,
            reverse: args.reverse,
            threads: args.threads,
            idle_hide: args.idle_hide,
            normalize: args.normalize,
            delay: args.delay,
            help: false,
            offset: 0,
            left: 0,
            prompt: None,
            message: String::new(),
        }
    }
    pub fn key(&mut self, key: Key, page: usize) -> Action {
        if key == Key::Control(3) {
            return Action::Quit;
        }
        if let Some(prompt) = self.prompt.as_mut() {
            match key {
                Key::Escape => {
                    self.prompt = None;
                }
                Key::Backspace => {
                    prompt.pop();
                }
                Key::Char(ch) if ch.is_ascii_digit() || ch == '.' => {
                    if prompt.len() < 16 {
                        prompt.push(ch);
                    }
                }
                Key::Enter => {
                    let parsed = hyper_tool_args::parse_interval(prompt);
                    self.prompt = None;
                    match parsed {
                        Ok(delay) => {
                            self.delay = delay;
                            self.message.clear();
                            return Action::Refresh;
                        }
                        Err(error) => self.message = error,
                    }
                }
                _ => return Action::None,
            }
            return Action::Redraw;
        }
        if matches!(key, Key::Char('q' | 'Q')) {
            return Action::Quit;
        }
        if self.help {
            self.help = false;
            return Action::Redraw;
        }
        self.message.clear();
        match key {
            Key::Char('h' | '?') => self.help = true,
            Key::Char('P') => self.sort = Sort::Cpu,
            Key::Char('T') => self.sort = Sort::Time,
            Key::Char('N') => self.sort = Sort::Koid,
            Key::Char('C') => self.sort = Sort::Name,
            Key::Char('R') => self.reverse = !self.reverse,
            Key::Char('H') => {
                self.threads = !self.threads;
                self.offset = 0;
            }
            Key::Char('I') => self.normalize = !self.normalize,
            Key::Char('i') => {
                self.idle_hide = !self.idle_hide;
                self.offset = 0;
            }
            Key::Char('d' | 's') => self.prompt = Some(String::new()),
            Key::Char(' ') | Key::Enter => return Action::Refresh,
            Key::Up => self.offset = self.offset.saturating_sub(1),
            Key::Down => self.offset = self.offset.saturating_add(1),
            Key::PageUp => self.offset = self.offset.saturating_sub(page.max(1)),
            Key::PageDown => self.offset = self.offset.saturating_add(page.max(1)),
            Key::Home => {
                self.offset = 0;
                self.left = 0;
            }
            Key::End => self.offset = usize::MAX,
            Key::Left => self.left = self.left.saturating_sub(8),
            Key::Right => self.left = self.left.saturating_add(8).min(512),
            _ => return Action::None,
        }
        Action::Redraw
    }
}

#[cfg(test)]
#[path = "../tests/view.rs"]
mod tests;
