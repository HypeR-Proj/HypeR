<!-- SPDX-FileCopyrightText: 2026 roolrz -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Editor

`editor` is a formal-system application. It uses the terminal
service's foreground lease, raw input and size API; it does not access a UART,
implement a line discipline or change bootstrap. A VT-compatible terminal
emulator provides screen presentation. Ordinary programs still only need stdio.

## Usage

```sh
editor /data/notes.txt
editor                       # unnamed buffer; save asks for a path
```

| Key | Action |
| --- | --- |
| Arrows, Home/End | Move by Unicode scalar, line or line boundary |
| Page Up/Page Down | Move one screen of lines |
| Enter, Tab, Backspace, Delete | Insert a line/tab or remove text |
| Ctrl-S | Save; ask for a name for an unnamed buffer |
| Ctrl-O | Save as another path |
| Ctrl-F | Find a literal string, wrapping to the beginning |
| Ctrl-Q | Quit; repeat to discard unsaved edits |
| Escape / Ctrl-C in a prompt | Cancel the prompt |

Input must be UTF-8 without NUL, at most 4 MiB and 131072 lines. Existing LF/CRLF
endings and a missing final newline are preserved. New lines use the first line's
ending style. Navigation accounts for wide Unicode scalars and tabs; it is not
grapheme-cluster editing. Long lines scroll horizontally. This initial editor
has no undo history, selections or syntax highlighting.

Save first writes and syncs a complete temporary copy in the destination
directory. Replacement by rename is used where supported. The current FAT
backend rejects replacement, so the editor falls back to writing the existing
file, truncating it to the new length and syncing it. On failure the temporary
copy is retained, its path is shown, and the editing buffer remains dirty. The
editor follows existing symlinks and refuses dangling links and directories.
Existing file permissions are preserved. There is no concurrent-writer detection
or power-loss transaction, especially on FAT; avoid simultaneous edits to the
same file. A successful rename replaces the directory entry rather than
preserving the identity of any hard links to the old file.

Control bytes in content and filenames are displayed as text, never interpreted
as terminal instructions. Bracketed paste cannot trigger save/quit controls and
CRLF paste inserts one line break. Fullscreen state and settings are restored on
exit; terminal-service independently restores the parent lease if the process
terminates unexpectedly. Existing direct kernel/service log output may still
interrupt a full-screen display; Ctrl-L redraws it.

## Source and tests

- `app/editor`: document editing and filesystem saves.
- `lib/text-ui`: shared application display/key/session helpers, delivered as
  `libhyper_text_ui_shared.so`; no server-side TTY state.
- `lib/terminal`: optional independent keyboard connector and control API.
- `app/shell`: foreground lease lifetime and capability delegation.

```sh
make app-test APP_TEST_PACKAGE=hyper-editor
make app-test APP_TEST_PACKAGE=hyper-text-ui
make test-editor ARCH=aarch64
```

The QEMU test verifies edited and saved bytes, failed-save recovery, protected
paste and subsequent shell input. It is part of `native-apps` CI and uses the
ordinary development fixture.
