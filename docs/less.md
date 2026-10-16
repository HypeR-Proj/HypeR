<!-- SPDX-FileCopyrightText: 2026 roolrz -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Less

`less` is a formal-system pager. It uses a foreground terminal lease for screen
output and a private keyboard connection, keeping piped stdin separate from
navigation keys. A VT-compatible terminal emulator provides screen presentation.

## Usage

```sh
less /data/notes.txt
cat /data/notes.txt | less
journalctl | less
```

Page Down, Space or `f` advances one screen; Page Up or `b` goes back. Up/Down or
`k`/`j` moves one line. `g`/Home goes to the beginning; `G`/End follows the current
end as more input arrives. Left/Right scrolls long lines horizontally. `/` asks
for a literal search and `n` repeats it in the buffered input. `q` or Ctrl-C exits.

The footer distinguishes input still being read from EOF. Navigation remains
available while a producer waits, and paging continues after the producer exits.
The pager retains at most 16 MiB or 262144 line starts in memory. Exceeding a bound
stops reading with a visible incomplete-input message and a failing exit status;
already read content remains available until `q`. This first version is a bounded
pager, not a disk-backed viewer for arbitrarily large files. Redirected stdout
copies input without these history bounds or terminal formatting.

Control bytes in content and filenames are displayed as text, never interpreted
as terminal instructions. Invalid UTF-8 is replaced only for display; redirected
output copies the original bytes. Fullscreen state and settings are restored on
exit; terminal-service independently restores the parent lease if the process
terminates unexpectedly. Existing direct kernel/service log output may still
interrupt a full-screen display; Ctrl-L redraws it.

## Source and tests

- `app/less`: bounded incremental input history, search and paging.
- `lib/text-ui`: shared application display/key/session helpers, delivered as
  `libhyper_text_ui_shared.so`; no server-side TTY state.
- `app/shell`: the final pipeline stage owns the foreground lease, preserving
  keyboard access after an upstream producer exits.

```sh
make app-test APP_TEST_PACKAGE=hyper-less
make test-less ARCH=aarch64
```

The QEMU test verifies file and pipeline paging, search, redirected copies,
navigation while a producer has not reached EOF, large pipe messages and
subsequent shell input. It is part of `native-apps` CI and uses the ordinary
development fixture.
