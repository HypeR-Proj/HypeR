// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Versioned terminal control messages. No pointers or target-sized integers.

use crate::settings::{ControlCharacters, InputFlags, OutputFlags, ReadPolicy, Settings};

pub const MAGIC: u32 = 0x5445_524d;
pub const VERSION: u16 = 1;
pub const HEADER_BYTES: usize = 32;
pub const SETTINGS_BYTES: usize = 64;
pub const MAX_MESSAGE: usize = HEADER_BYTES + SETTINGS_BYTES;
pub const PREPARE: u16 = 1;
pub const BIND: u16 = 2;
pub const RELEASE: u16 = 3;
pub const GET_SETTINGS: u16 = 4;
pub const SET_SETTINGS: u16 = 5;
pub const GET_SIZE: u16 = 6;
pub const SET_SIZE: u16 = 7;
pub const TAKE_EVENTS: u16 = 8;
pub const CLEAR_INPUT: u16 = 9;
pub const PAUSE_OUTPUT: u16 = 10;
pub const PRESENTATION: u16 = 11;
pub const RESPONSE: u16 = 0x8000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Header {
    pub operation: u16,
    pub id: u64,
    pub value: u64,
    pub status: i64,
}

impl Header {
    #[must_use]
    pub fn encode(self) -> [u8; HEADER_BYTES] {
        let mut bytes = [0; HEADER_BYTES];
        bytes[..4].copy_from_slice(&MAGIC.to_le_bytes());
        bytes[4..6].copy_from_slice(&VERSION.to_le_bytes());
        bytes[6..8].copy_from_slice(&self.operation.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.id.to_le_bytes());
        bytes[16..24].copy_from_slice(&self.value.to_le_bytes());
        bytes[24..].copy_from_slice(&self.status.to_le_bytes());
        bytes
    }
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let header = bytes.get(..HEADER_BYTES)?;
        if header[..4] != MAGIC.to_le_bytes() || header[4..6] != VERSION.to_le_bytes() {
            return None;
        }
        Some(Self {
            operation: u16::from_le_bytes([header[6], header[7]]),
            id: word(header, 8)?,
            value: word(header, 16)?,
            status: word(header, 24)? as i64,
        })
    }
}

fn word(bytes: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        bytes.get(offset..offset + 8)?.try_into().ok()?,
    ))
}

#[must_use]
pub fn encode_settings(settings: Settings) -> [u8; SETTINGS_BYTES] {
    let mut bytes = [0; SETTINGS_BYTES];
    let flags = [
        settings.canonical,
        settings.echo,
        settings.echo_controls,
        settings.visual_erase,
        settings.utf8,
        settings.input.ignore_cr,
        settings.input.cr_to_lf,
        settings.input.lf_to_cr,
        settings.input.receive_flow_control,
        settings.input.send_flow_control,
        settings.input.resume_on_any,
        settings.output.lf_to_crlf,
        settings.output.cr_to_lf,
    ];
    for (index, flag) in flags.iter().enumerate() {
        bytes[index] = u8::from(*flag);
    }
    let extended = [
        settings.echo_newline,
        settings.echo_kill_line,
        settings.input.strip_high_bit,
        settings.input.ascii_lowercase,
        settings.output.expand_tabs,
        settings.output.suppress_cr_at_column_zero,
        settings.output.newline_resets_column,
    ];
    for (index, flag) in extended.iter().enumerate() {
        bytes[56 + index] = u8::from(*flag);
    }
    let controls = settings.controls;
    for (index, value) in [
        controls.eof,
        controls.erase,
        controls.kill,
        controls.word_erase,
        controls.literal_next,
        controls.reprint,
        controls.end_of_line,
        controls.start,
        controls.stop,
    ]
    .iter()
    .enumerate()
    {
        let encoded = value.map_or(256, u16::from);
        bytes[16 + index * 2..18 + index * 2].copy_from_slice(&encoded.to_le_bytes());
    }
    let (kind, minimum, time) = match settings.read_policy {
        ReadPolicy::Available => (0, 0, 0),
        ReadPolicy::AtLeast { minimum } => (1, minimum, 0),
        ReadPolicy::FirstByteTimeout { timeout_ns } => (2, 0, timeout_ns),
        ReadPolicy::InterByteTimeout { minimum, gap_ns } => (3, minimum, gap_ns),
    };
    bytes[40] = kind;
    bytes[44..48].copy_from_slice(&minimum.to_le_bytes());
    bytes[48..56].copy_from_slice(&time.to_le_bytes());
    bytes
}

pub fn decode_settings(bytes: &[u8]) -> Option<Settings> {
    if bytes.len() != SETTINGS_BYTES
        || bytes[..13].iter().any(|value| *value > 1)
        || bytes[56..63].iter().any(|value| *value > 1)
        || bytes[13..16]
            .iter()
            .chain(bytes[34..40].iter())
            .chain(bytes[41..44].iter())
            .any(|value| *value != 0)
        || bytes[63] != 0
    {
        return None;
    }
    let mut controls = [None; 9];
    for (index, control) in controls.iter_mut().enumerate() {
        let value = u16::from_le_bytes([bytes[16 + 2 * index], bytes[17 + 2 * index]]);
        *control = if value == 256 {
            None
        } else {
            Some(u8::try_from(value).ok()?)
        };
    }
    let minimum = u32::from_le_bytes(bytes[44..48].try_into().ok()?);
    let time = word(bytes, 48)?;
    let read_policy = match (bytes[40], minimum, time) {
        (0, 0, 0) => ReadPolicy::Available,
        (1, minimum, 0) => ReadPolicy::AtLeast { minimum },
        (2, 0, timeout_ns) => ReadPolicy::FirstByteTimeout { timeout_ns },
        (3, minimum, gap_ns) => ReadPolicy::InterByteTimeout { minimum, gap_ns },
        _ => return None,
    };
    let settings = Settings {
        canonical: bytes[0] != 0,
        echo: bytes[1] != 0,
        echo_controls: bytes[2] != 0,
        visual_erase: bytes[3] != 0,
        utf8: bytes[4] != 0,
        echo_newline: bytes[56] != 0,
        echo_kill_line: bytes[57] != 0,
        input: InputFlags {
            ignore_cr: bytes[5] != 0,
            cr_to_lf: bytes[6] != 0,
            lf_to_cr: bytes[7] != 0,
            strip_high_bit: bytes[58] != 0,
            ascii_lowercase: bytes[59] != 0,
            receive_flow_control: bytes[8] != 0,
            send_flow_control: bytes[9] != 0,
            resume_on_any: bytes[10] != 0,
        },
        output: OutputFlags {
            lf_to_crlf: bytes[11] != 0,
            cr_to_lf: bytes[12] != 0,
            expand_tabs: bytes[60] != 0,
            suppress_cr_at_column_zero: bytes[61] != 0,
            newline_resets_column: bytes[62] != 0,
        },
        controls: ControlCharacters {
            eof: controls[0],
            erase: controls[1],
            kill: controls[2],
            word_erase: controls[3],
            literal_next: controls[4],
            reprint: controls[5],
            end_of_line: controls[6],
            start: controls[7],
            stop: controls[8],
        },
        read_policy,
    };
    settings.validate().ok()?;
    Some(settings)
}
