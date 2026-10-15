// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Fullscreen pager with independent keyboard and piped standard input.

#[cfg(target_os = "hyper")]
extern crate hyper_clap_shared as _;
#[cfg(target_os = "hyper")]
extern crate hyper_text_ui_shared as hyper_text_ui;
use clap::Parser;
use hyper_less::Content;
use hyper_os::handle::{ByteChannelObject, OwnedHandle};
use hyper_os::stream::{ProcessInput, ReadResult};
use hyper_os::wait::{ObjectSignals, WaitItem};
use hyper_text_ui::{Error, Key, Screen, Session};
use std::fs::File;
use std::io::{self, IsTerminal, Read, Write};
use std::process::ExitCode;

enum Input {
    File(File),
    Stream { input: ProcessInput, pending: bool },
    Pipe(&'static OwnedHandle<ByteChannelObject>),
}
impl Input {
    fn read(&mut self, bytes: &mut [u8]) -> Result<Option<usize>, Error> {
        match self {
            Self::File(file) => Ok(Some(file.read(bytes)?)),
            Self::Pipe(handle) => match handle.as_byte_channel().try_receive(bytes) {
                Ok(0) => Ok(None), // Empty messages carry no data; only peer closure is EOF.
                Ok(count) => Ok(Some(count)),
                Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => Ok(None),
                Err(hyper_os::Error::Status(hyper_os::Status::PEER_CLOSED)) => Ok(Some(0)),
                Err(error) => Err(error.into()),
            },
            Self::Stream { input, pending } => {
                if !*pending {
                    input.start_read(
                        bytes.len().min(hyper_os::stream::MAX_DATA),
                        hyper_os::DEADLINE_INFINITE,
                    )?;
                    *pending = true;
                }
                match input.try_complete(bytes)? {
                    None => Ok(None),
                    Some(result) => {
                        *pending = false;
                        match result {
                            ReadResult::Data(count) => Ok(Some(count)),
                            ReadResult::Eof => Ok(Some(0)),
                            ReadResult::Cancelled => Err("input cancelled".into()),
                            ReadResult::TimedOut | ReadResult::ConfigurationChanged => {
                                input.start_read(
                                    bytes.len().min(hyper_os::stream::MAX_DATA),
                                    hyper_os::DEADLINE_INFINITE,
                                )?;
                                *pending = true;
                                Ok(None)
                            }
                        }
                    }
                }
            }
        }
    }
    fn wait_item(&self) -> Option<WaitItem<'_>> {
        match self {
            Self::Stream {
                input,
                pending: true,
            } => Some(input.wait_item()),
            Self::Pipe(handle) => Some(WaitItem::new(
                handle.as_handle_ref(),
                ObjectSignals::<ByteChannelObject>::READABLE
                    .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
            )),
            _ => None,
        }
    }
}
struct Pager {
    content: Content,
    top: usize,
    left: usize,
    end: bool,
    bottom: bool,
    prompt: Option<String>,
    query: String,
    message: String,
}
impl Pager {
    fn key(&mut self, key: Key, height: usize) -> bool {
        if let Some(mut query) = self.prompt.take() {
            match key {
                Key::Escape | Key::Control(3) => return false,
                Key::Enter => {
                    self.query = query;
                    self.search();
                    return false;
                }
                Key::Backspace => {
                    query.pop();
                }
                Key::Char(ch) if query.len() + ch.len_utf8() <= 1024 => query.push(ch),
                _ => {}
            }
            self.prompt = Some(query);
            return false;
        }
        self.bottom = false;
        match key {
            Key::Char('q' | 'Q') | Key::Control(3) => return true,
            Key::PageDown | Key::Char(' ' | 'f') | Key::Control(6) => {
                self.top = self.top.saturating_add(height)
            }
            Key::PageUp | Key::Char('b') | Key::Control(2) => {
                self.top = self.top.saturating_sub(height)
            }
            Key::Down | Key::Char('j') | Key::Enter => self.top = self.top.saturating_add(1),
            Key::Up | Key::Char('k') => self.top = self.top.saturating_sub(1),
            Key::Home | Key::Char('g') => self.top = 0,
            Key::End | Key::Char('G') => self.bottom = true,
            Key::Left => self.left = self.left.saturating_sub(8),
            Key::Right => self.left = self.left.saturating_add(8).min(hyper_less::MAX_BYTES),
            Key::Char('/') => self.prompt = Some(String::new()),
            Key::Char('n') => self.search(),
            _ => {}
        }
        self.top = self.top.min(self.content.lines().saturating_sub(1));
        false
    }
    fn search(&mut self) {
        if let Some(row) = self.content.find(self.top + 1, &self.query) {
            self.top = row;
            self.message.clear();
        } else {
            self.message = "Pattern not found in buffered input".into();
        }
    }
    fn render(&mut self, session: &Session, name: &str) -> Result<(), Error> {
        let height = session.rows - 1;
        if self.bottom {
            self.top = self.content.lines().saturating_sub(height);
        }
        let mut screen = Screen::new(session.columns);
        for row in 0..height {
            let n = self.top + row;
            let line = self.content.line(n);
            screen.row(
                row,
                if n < self.content.lines() { &line } else { "~" },
                self.left,
                false,
            );
        }
        let status = if let Some(query) = &self.prompt {
            format!("/{query}")
        } else if !self.message.is_empty() {
            self.message.clone()
        } else {
            format!(
                "less | {name} | lines {}-{} / {}{} | q quit",
                (self.top + 1).min(self.content.lines()),
                (self.top + height).min(self.content.lines()),
                self.content.lines(),
                if self.end { " (END)" } else { " (reading)" }
            )
        };
        screen.row(session.rows - 1, &status, 0, true);
        session.write(screen.bytes())
    }
}
fn run() -> Result<bool, Error> {
    let path = hyper_less::Args::parse()
        .file
        .filter(|p| p != std::path::Path::new("-"));
    if !io::stdout().is_terminal() {
        let mut out = io::stdout().lock();
        if let Some(path) = path {
            io::copy(&mut File::open(path)?, &mut out)?;
        } else {
            io::copy(&mut io::stdin().lock(), &mut out)?;
        }
        out.flush()?;
        return Ok(true);
    }
    if path.is_none() && io::stdin().is_terminal() {
        return Err("provide a file or pipe, for example: cat FILE | less".into());
    }
    let name = path
        .as_ref()
        .map_or("stdin".into(), |p| p.display().to_string());
    let mut startup = hyper_rt::process::startup()?;
    let mut input = if let Some(path) = &path {
        Input::File(File::open(path)?)
    } else if hyper_rt::process::stdin_stream()?.is_some() {
        Input::Stream {
            input: ProcessInput::acquire()?,
            pending: false,
        }
    } else {
        // This application has made no stdio reads. Read complete pipe messages
        // directly, with the runtime retaining the handle until process exit.
        Input::Pipe(hyper_rt::process::stdin()?)
    };
    let mut session = Session::open(&mut startup)?;
    let mut pager = Pager {
        content: Content::default(),
        top: 0,
        left: 0,
        end: false,
        bottom: false,
        prompt: None,
        query: String::new(),
        message: String::new(),
    };
    let mut failed = false;
    let mut dirty = true;
    let mut next_render = 0;
    // Byte channels preserve whole messages; stdin producers may write 64 KiB.
    // Keep this off the thread stack and reuse it for every read.
    let mut bytes = vec![0; hyper_os::channel::MAX_MESSAGE_BYTES];
    loop {
        let mut progress = false;
        if let Some(key) = session.poll()? {
            if pager.key(key, session.rows - 1) {
                return Ok(!failed);
            }
            while let Some(key) = session.buffered_key() {
                if pager.key(key, session.rows - 1) {
                    return Ok(!failed);
                }
            }
            dirty = true;
            next_render = 0;
            progress = true;
        }
        if !pager.end {
            match input.read(&mut bytes) {
                Ok(None) => {}
                Ok(Some(0)) => {
                    pager.end = true;
                    dirty = true;
                }
                Ok(Some(count)) => {
                    if let Err(error) = pager.content.append(&bytes[..count]) {
                        pager.message = format!("{error}; q quits (input incomplete)");
                        pager.end = true;
                        failed = true;
                    }
                    dirty = true;
                    progress = true;
                }
                Err(error) => {
                    pager.message = format!("Read failed: {error}; q quits");
                    pager.end = true;
                    failed = true;
                    dirty = true;
                }
            }
        }
        if session.resized {
            session.resized = false;
            dirty = true;
            next_render = 0;
        }
        let now = hyper_os::time::monotonic_now()?.as_nanoseconds();
        if dirty && (now >= next_render || !progress) {
            pager.render(&session, &name)?;
            dirty = false;
            next_render = now.saturating_add(100_000_000);
        }
        if !progress {
            session.wait(if pager.end { None } else { input.wait_item() })?;
        }
    }
}
fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("less: {error}");
            ExitCode::FAILURE
        }
    }
}
