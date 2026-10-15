// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Basic fullscreen editor for the formal system terminal.

#[cfg(target_os = "hyper")]
extern crate hyper_clap_shared as _;
#[cfg(target_os = "hyper")]
extern crate hyper_text_ui_shared as hyper_text_ui;
use clap::Parser;
use hyper_editor::{Document, storage};
use hyper_text_ui::{Error, Key, Screen, Session};
use std::path::PathBuf;
use std::process::ExitCode;

struct Editor {
    document: Document,
    path: Option<PathBuf>,
    top: usize,
    left: usize,
    message: String,
    prompt: Option<(Prompt, String)>,
    confirm_quit: bool,
}
#[derive(Clone, Copy)]
enum Prompt {
    Save,
    Search,
}
impl Editor {
    fn save(&mut self) {
        let Some(path) = self.path.clone() else {
            self.prompt = Some((Prompt::Save, String::new()));
            return;
        };
        self.save_as(path);
    }
    fn save_as(&mut self, path: PathBuf) {
        match storage::save(&path, self.document.text()) {
            Ok(()) => {
                self.path = Some(path);
                self.document.dirty = false;
                self.message = format!("Saved {} bytes", self.document.text().len());
            }
            Err(error) => self.message = format!("Save failed: {error}"),
        }
    }
    fn key(&mut self, key: Key, page: usize) -> bool {
        if let Some((kind, mut value)) = self.prompt.take() {
            match key {
                Key::Escape | Key::Control(3) => {
                    self.message = "Cancelled".into();
                    return false;
                }
                Key::Enter => {
                    if !value.is_empty() {
                        match kind {
                            Prompt::Save => {
                                self.save_as(value.into());
                            }
                            Prompt::Search => {
                                self.message = if self.document.find(&value) {
                                    "Match"
                                } else {
                                    "Not found"
                                }
                                .into()
                            }
                        }
                    }
                    return false;
                }
                Key::Char(ch) if value.len() + ch.len_utf8() <= 1024 => value.push(ch),
                Key::Backspace => {
                    value.pop();
                }
                _ => {}
            }
            self.prompt = Some((kind, value));
            return false;
        }
        if key == Key::Control(17) {
            if !self.document.dirty || self.confirm_quit {
                return true;
            }
            self.confirm_quit = true;
            self.message = "Unsaved changes. Ctrl-Q again discards; Ctrl-S saves.".into();
            return false;
        }
        self.confirm_quit = false;
        self.message.clear();
        match key {
            Key::Control(19) => self.save(),
            Key::Control(15) => {
                self.prompt = Some((
                    Prompt::Save,
                    self.path
                        .as_ref()
                        .map_or(String::new(), |p| p.to_string_lossy().into_owned()),
                ))
            }
            Key::Control(6) => self.prompt = Some((Prompt::Search, String::new())),
            _ => {
                if let Err(error) = self.document.key(key, page) {
                    self.message = error.into();
                }
            }
        }
        false
    }
    fn render(&mut self, session: &Session) -> Result<(), Error> {
        let height = session.rows - 3;
        let row = self.document.row();
        let column = self.document.column();
        if row < self.top {
            self.top = row;
        }
        if row >= self.top + height {
            self.top = row - height + 1;
        }
        let width = session.columns - 1;
        if column < self.left {
            self.left = column;
        }
        if column >= self.left + width {
            self.left = column - width + 1;
        }
        let mut screen = Screen::new(session.columns);
        screen.row(
            0,
            &format!(
                "editor | {}{} | {}:{}",
                self.path
                    .as_ref()
                    .map_or("[new]".into(), |p| p.display().to_string()),
                if self.document.dirty { " *" } else { "" },
                row + 1,
                column + 1
            ),
            0,
            true,
        );
        for n in 0..height {
            screen.row(n + 1, self.document.line(self.top + n), self.left, false);
        }
        let status = self.prompt.as_ref().map(|(kind, value)| {
            format!(
                "{}{}",
                match kind {
                    Prompt::Save => "Save as: ",
                    Prompt::Search => "Find: ",
                },
                value
            )
        });
        screen.row(
            session.rows - 2,
            status.as_deref().unwrap_or(&self.message),
            0,
            false,
        );
        screen.row(
            session.rows - 1,
            "^S Save  ^O Save as  ^F Find  ^Q Quit   PgUp/PgDn",
            0,
            true,
        );
        if let Some(status) = status {
            screen.cursor(session.rows - 2, hyper_text_ui::cells(&status));
        } else {
            screen.cursor(row - self.top + 1, column - self.left);
        }
        session.write(screen.bytes())
    }
}
fn run() -> Result<(), Error> {
    let path = hyper_editor::Args::parse().file;
    let text = match path.as_deref().map(storage::load).transpose() {
        Ok(text) => text.unwrap_or_default(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.into()),
    };
    let mut editor = Editor {
        document: Document::new(text)?,
        path,
        top: 0,
        left: 0,
        message: String::new(),
        prompt: None,
        confirm_quit: false,
    };
    let mut startup = hyper_rt::process::startup()?;
    let mut session = Session::open(&mut startup)?;
    editor.render(&session)?;
    loop {
        if let Some(key) = session.poll()? {
            if editor.key(key, session.rows - 3) {
                return Ok(());
            }
            // Drain already delivered keys before issuing another full frame.
            while let Some(key) = session.buffered_key() {
                if editor.key(key, session.rows - 3) {
                    return Ok(());
                }
            }
            editor.render(&session)?;
        } else {
            if session.resized {
                session.resized = false;
                editor.render(&session)?;
            }
            session.wait(None)?;
        }
    }
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("editor: {error}");
            ExitCode::FAILURE
        }
    }
}
