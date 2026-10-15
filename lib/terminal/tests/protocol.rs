// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use hyper_terminal::Settings;

#[test]
fn wire_roundtrip_rejects_reserved_fields_and_conflicting_controls() {
    use hyper_terminal::protocol::{decode_settings, encode_settings};
    for settings in [Settings::default(), Settings::raw()] {
        let bytes = encode_settings(settings);
        assert_eq!(decode_settings(&bytes), Some(settings));
        for length in 0..bytes.len() {
            assert!(decode_settings(&bytes[..length]).is_none());
        }
    }
    let mut bytes = encode_settings(Settings::default());
    bytes[63] = 1;
    assert!(decode_settings(&bytes).is_none());
    let mut settings = Settings::default();
    settings.controls.erase = settings.controls.kill;
    assert!(settings.validate().is_err());
}

#[test]
fn formatting_settings_roundtrip_and_reject_nonboolean_values() {
    use hyper_terminal::protocol::{decode_settings, encode_settings};
    for mask in 0..8 {
        let mut settings = Settings {
            visual_kill: mask & 1 != 0,
            echo_erased: mask & 2 != 0,
            ..Settings::default()
        };
        settings.output.uppercase = mask & 4 != 0;
        assert_eq!(decode_settings(&encode_settings(settings)), Some(settings));
    }
    for index in [13, 14, 15] {
        let mut bytes = encode_settings(Settings::default());
        bytes[index] = 2;
        assert!(decode_settings(&bytes).is_none());
    }
}
