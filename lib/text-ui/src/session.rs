// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use crate::{Decoder, Error, Key};
use hyper_os::stream::{Connection, ReadResult};
use hyper_os::wait::{WaitItem, wait_many};
use hyper_terminal::{Settings, Terminal};
use std::io::{IsTerminal, Write};

/// Restores settings and fullscreen presentation on every returning path. The
/// terminal server independently restores the parent when this process dies.
pub struct Session {
    terminal: Terminal,
    original: Settings,
    input: Connection,
    pending: bool,
    decoder: Decoder,
    partial_until: u64,
    pub columns: usize,
    pub rows: usize,
    pub resized: bool,
}
impl Session {
    pub fn buffered_key(&mut self) -> Option<Key> {
        self.decoder.pop()
    }
    pub fn open(startup: &mut hyper_os::startup::Startup<'_>) -> Result<Self, Error> {
        if !std::io::stdout().is_terminal() {
            return Err("stdout is not a terminal".into());
        }
        let mut terminal = Terminal::take(startup)?.ok_or("no terminal control capability")?;
        let original = terminal.settings()?;
        let input = terminal.open_input()?;
        let (columns, rows) = terminal.size()?;
        let mut session = Self {
            terminal,
            original,
            input,
            pending: false,
            decoder: Decoder::default(),
            partial_until: 0,
            columns: usize::from(columns).clamp(10, 400),
            rows: usize::from(rows).clamp(4, 120),
            resized: false,
        };
        session.terminal.set_settings(Settings::raw())?;
        session.terminal.presentation(true, true)?;
        session.write(b"\x1b[?2004h\x1b[2J")?;
        Ok(session)
    }
    pub fn write(&self, bytes: &[u8]) -> Result<(), Error> {
        let mut out = std::io::stdout().lock();
        out.write_all(bytes)?;
        out.flush()?;
        Ok(())
    }
    pub fn poll(&mut self) -> Result<Option<Key>, Error> {
        if let Some(event) = self.terminal.event_wait_item()
            && wait_many(&[event], 0).is_ok()
        {
            self.terminal.take_events()?;
            let (columns, rows) = self.terminal.size()?;
            self.columns = usize::from(columns).clamp(10, 400);
            self.rows = usize::from(rows).clamp(4, 120);
            self.resized = true;
        }
        if let Some(key) = self.decoder.pop() {
            return Ok(Some(key));
        }
        if !self.pending {
            self.input.start_read(4096, hyper_os::DEADLINE_INFINITE)?;
            self.pending = true;
        }
        let mut bytes = [0; 4096];
        if let Some(result) = self.input.try_complete(&mut bytes)? {
            self.pending = false;
            match result {
                ReadResult::Data(count) => {
                    self.decoder.feed(&bytes[..count]);
                    self.partial_until = hyper_os::time::monotonic_now()?
                        .as_nanoseconds()
                        .saturating_add(100_000_000);
                    if let Some(key) = self.decoder.pop() {
                        return Ok(Some(key));
                    }
                }
                ReadResult::Eof | ReadResult::Cancelled => {
                    return Err("terminal input closed".into());
                }
                ReadResult::TimedOut | ReadResult::ConfigurationChanged => {}
            }
        }
        if !self.pending {
            self.input.start_read(4096, hyper_os::DEADLINE_INFINITE)?;
            self.pending = true;
        }
        if self.decoder.incomplete()
            && hyper_os::time::monotonic_now()?.as_nanoseconds() >= self.partial_until
        {
            return Ok(self.decoder.expire());
        }
        Ok(None)
    }
    pub fn wait(&self, extra: Option<WaitItem<'_>>) -> Result<(), Error> {
        self.wait_until(extra, hyper_os::DEADLINE_INFINITE)
    }
    /// Wait for input, resize, an optional source, or an absolute monotonic deadline.
    pub fn wait_until(&self, extra: Option<WaitItem<'_>>, deadline: u64) -> Result<(), Error> {
        let mut waits = vec![self.input.wait_item()];
        if let Some(event) = self.terminal.event_wait_item() {
            waits.push(event);
        }
        if let Some(extra) = extra {
            waits.push(extra);
        }
        let deadline = if self.decoder.incomplete() {
            self.partial_until.min(deadline)
        } else {
            deadline
        };
        match wait_many(&waits, deadline) {
            Ok(_) | Err(hyper_os::Error::Status(hyper_os::Status::TIMED_OUT)) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.write(b"\x1b[?2004l\x1b[0m");
        let _ = self.terminal.presentation(false, false);
        let _ = self.terminal.set_settings(self.original);
    }
}
