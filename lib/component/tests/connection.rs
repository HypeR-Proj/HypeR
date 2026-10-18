// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn diagnostics_fit_after_json_encoding() -> Result<()> {
    for text in [
        String::new(),
        "short diagnostic".into(),
        "a".repeat(1024),
        "\u{1f642}".repeat(256),
        "\0".repeat(256),
        "\"\\\n\t\0\u{1f642}".repeat(256),
    ] {
        for failure in [false, true] {
            let request = if failure {
                Request::ReportFailure {
                    message: text.clone(),
                }
            } else {
                Request::Status {
                    message: text.clone(),
                }
            };
            let bytes = encode_request(request)?;
            assert!(bytes.len() <= MAX_MESSAGE);
            let decoded: Request =
                serde_json::from_slice(&bytes).map_err(|_| Error::InvalidResponse)?;
            let message = match decoded {
                Request::ReportFailure { message } if failure => message,
                Request::Status { message } if !failure => message,
                _ => return Err(Error::InvalidResponse),
            };
            assert!(text.starts_with(&message));
            assert!(message.chars().count() <= 256);
            if text.chars().count() < 256 {
                assert_eq!(message, text);
            } else {
                assert!(!message.is_empty());
            }
            // Truncation preserves the longest prefix allowed by both bounds.
            if message.chars().count() < 256
                && let Some(next) = text[message.len()..].chars().next()
            {
                let mut longer = message;
                longer.push(next);
                let request = if failure {
                    Request::ReportFailure { message: longer }
                } else {
                    Request::Status { message: longer }
                };
                assert!(
                    serde_json::to_vec(&request)
                        .map_err(|_| Error::InvalidResponse)?
                        .len()
                        > MAX_MESSAGE
                );
            }
        }
    }
    Ok(())
}

#[test]
fn oversized_routes_are_rejected_without_truncation() {
    for request in [
        Request::Connect {
            name: "a".repeat(MAX_MESSAGE),
            deadline_ns: 1,
        },
        Request::Publish {
            name: "a".repeat(MAX_MESSAGE),
        },
    ] {
        assert!(matches!(
            encode_request(request),
            Err(Error::InvalidResponse)
        ));
    }
}
