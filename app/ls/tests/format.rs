// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;
#[test]
fn mode_and_size_are_readable() {
    assert_eq!(mode_text(0o755, true, false), "drwxr-xr-x");
    assert_eq!(mode_text(0o777, false, true), "lrwxrwxrwx");
    assert_eq!(mode_text(0o4755, false, false), "-rwsr-xr-x");
    assert_eq!(mode_text(0o1644, false, false), "-rw-r--r-T");
    assert_eq!(readable_size(0), "0 B");
    assert_eq!(readable_size(1536), "1.5 KiB");
    assert_eq!(readable_size(u64::MAX), "15.9 EiB");
}

#[test]
fn paths_cannot_inject_terminal_control_sequences() {
    assert_eq!(escape_path("../目标"), "../目标");
    assert_eq!(
        escape_path("目标\n\t\\\x1b[31m"),
        "目标\\n\\t\\\\\\u{1b}[31m"
    );
}
