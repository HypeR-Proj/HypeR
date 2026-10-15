// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native settings; these values deliberately do not encode Linux termios flags.

/// Input conversion and software-flow choices, applied before line editing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InputFlags {
    pub ignore_cr: bool,
    pub cr_to_lf: bool,
    pub lf_to_cr: bool,
    pub strip_high_bit: bool,
    /// Traditional Latin-1 byte conversion, not Unicode case folding.
    pub lowercase: bool,
    /// Recognize the peer's STOP/START characters to pause/resume our output.
    pub receive_flow_control: bool,
    /// Generate STOP/START as our own input queue crosses its watermarks.
    pub send_flow_control: bool,
    /// An ordinary byte resumes output paused by a received STOP.
    pub resume_on_any: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutputFlags {
    pub lf_to_crlf: bool,
    pub cr_to_lf: bool,
    pub expand_tabs: bool,
    pub suppress_cr_at_column_zero: bool,
    pub newline_resets_column: bool,
    /// Traditional Latin-1 byte conversion, not Unicode case folding.
    pub uppercase: bool,
}

/// Disabled controls are `None`; NUL remains a valid configured character.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlCharacters {
    pub eof: Option<u8>,
    pub erase: Option<u8>,
    pub kill: Option<u8>,
    pub word_erase: Option<u8>,
    pub literal_next: Option<u8>,
    pub reprint: Option<u8>,
    pub end_of_line: Option<u8>,
    pub start: Option<u8>,
    pub stop: Option<u8>,
}

impl Default for ControlCharacters {
    fn default() -> Self {
        Self {
            eof: Some(4),
            erase: Some(127),
            kill: Some(21),
            word_erase: Some(23),
            literal_next: Some(22),
            reprint: Some(18),
            end_of_line: None,
            start: Some(17),
            stop: Some(19),
        }
    }
}

/// Four explicit noncanonical read policies. Timeouts are not end-of-file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadPolicy {
    Available,
    AtLeast { minimum: u32 },
    FirstByteTimeout { timeout_ns: u64 },
    InterByteTimeout { minimum: u32, gap_ns: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Settings {
    pub canonical: bool,
    pub echo: bool,
    pub echo_controls: bool,
    pub visual_erase: bool,
    /// Echo an unquoted canonical newline even when general echo is disabled.
    pub echo_newline: bool,
    /// Append a newline after echoing kill, unless visual kill is enabled.
    pub echo_kill_line: bool,
    /// Erase the killed line when echo, visual erase and kill echo are enabled.
    pub visual_kill: bool,
    /// Print erased characters between backslash and slash (hardcopy echo).
    pub echo_erased: bool,
    /// Canonical erase removes one Unicode scalar rather than one byte.
    /// Grapheme-cluster editing belongs to the application editor.
    pub utf8: bool,
    pub input: InputFlags,
    pub output: OutputFlags,
    pub controls: ControlCharacters,
    pub read_policy: ReadPolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingsError {
    ConflictingControlCharacters,
    MissingFlowCharacters,
    InvalidReadPolicy,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            canonical: true,
            echo: true,
            echo_controls: true,
            visual_erase: true,
            echo_newline: false,
            echo_kill_line: true,
            visual_kill: true,
            echo_erased: false,
            utf8: true,
            input: InputFlags {
                ignore_cr: false,
                cr_to_lf: true,
                lf_to_cr: false,
                strip_high_bit: false,
                lowercase: false,
                receive_flow_control: true,
                send_flow_control: false,
                resume_on_any: false,
            },
            output: OutputFlags {
                lf_to_crlf: true,
                cr_to_lf: false,
                expand_tabs: false,
                suppress_cr_at_column_zero: false,
                newline_resets_column: true,
                uppercase: false,
            },
            controls: ControlCharacters::default(),
            read_policy: ReadPolicy::AtLeast { minimum: 1 },
        }
    }
}

impl Settings {
    /// Binary-transparent input and output, including NUL, CR, EOF and XON/XOFF.
    #[must_use]
    pub fn raw() -> Self {
        Self {
            canonical: false,
            echo: false,
            echo_controls: false,
            visual_erase: false,
            echo_kill_line: false,
            visual_kill: false,
            echo_erased: false,
            utf8: false,
            input: InputFlags {
                ignore_cr: false,
                cr_to_lf: false,
                lf_to_cr: false,
                strip_high_bit: false,
                lowercase: false,
                receive_flow_control: false,
                send_flow_control: false,
                resume_on_any: false,
            },
            output: OutputFlags {
                lf_to_crlf: false,
                cr_to_lf: false,
                expand_tabs: false,
                suppress_cr_at_column_zero: false,
                newline_resets_column: false,
                uppercase: false,
            },
            ..Self::default()
        }
    }

    pub fn validate(self) -> Result<(), SettingsError> {
        if self.input.receive_flow_control || self.input.send_flow_control {
            if self.controls.start.is_none() || self.controls.stop.is_none() {
                return Err(SettingsError::MissingFlowCharacters);
            }
            if self.controls.start == self.controls.stop {
                return Err(SettingsError::ConflictingControlCharacters);
            }
        }
        let edit = [
            self.controls.eof,
            self.controls.erase,
            self.controls.kill,
            self.controls.word_erase,
            self.controls.literal_next,
            self.controls.reprint,
            self.controls.end_of_line,
        ];
        if self.canonical {
            for (index, value) in edit.iter().enumerate() {
                if value.is_some() && edit[index + 1..].contains(value) {
                    return Err(SettingsError::ConflictingControlCharacters);
                }
                if value == &Some(b'\n')
                    || (self.input.receive_flow_control
                        && value.is_some()
                        && [self.controls.start, self.controls.stop].contains(value))
                {
                    return Err(SettingsError::ConflictingControlCharacters);
                }
            }
        }
        match self.read_policy {
            ReadPolicy::AtLeast { minimum: 0 }
            | ReadPolicy::InterByteTimeout { minimum: 0, .. }
            | ReadPolicy::FirstByteTimeout { timeout_ns: 0 }
            | ReadPolicy::InterByteTimeout { gap_ns: 0, .. } => {
                Err(SettingsError::InvalidReadPolicy)
            }
            _ => Ok(()),
        }
    }
}
