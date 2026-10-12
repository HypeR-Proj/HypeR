<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Shell and text filtering

The formal shell is `app/shell`, installed as `/system/bin/sh`. It retains the
interactive editing, command launch, pipeline and redirection features described
below. It is not a Bash-compatible shell.

`bootstrap/stage1/fallback-sh` is a separate, statically linked recovery service,
installed as `/svc/fallback-sh` in the initial ramdisk and retained at
`/boot/svc/fallback-sh` after root switch. It has no clap dependency, command-line
options, history, pipelines or file redirection. Its builtins are `cd`, `pwd`,
`help` and `exit`; bounded line editing and quoted executable arguments remain.
It can launch recovery tools or explicitly selected system programs when those
are accessible. The `svc` directories are never searched for bare command names.

Stage-1 init launches fallback directly only for failures before supervision
handoff. Its input and output are raw byte streams from `console-raw`, with no
terminal settings or line discipline. After ownership transfers, formal init
starts `session-manager`, which launches `terminal-service` and the formal
shell. Terminal or shell exit retires the old session and starts a fresh one,
with bounded retries and a delay. Failures after handoff never select bootstrap
recovery. See [Native terminals](terminal.md) for ownership and restart behavior.

Both shells open `mount` and `umount` through their fixed boot image environment;
other bare commands, including `sh`, resolve under `/system/bin`. Explicit paths
resolve from the live namespace or current directory. In the
[default deployment](boot-layout.md), there is no root-level `/bin` alias.
Launching a boot tool does not open the system library directory, so storage
failure cannot prevent static recovery commands. Use `help` for builtins and
`APP --help` for application options.

After repairing storage from recovery, run `/system/bin/sh` to enter the formal
shell; `exit` returns to the recovery shell. Launching that exact trusted path
also works from another formal shell. It receives the bootstrap environment,
diagnostic authority and available filesystem/VM client connectors with rights
to delegate them to its own tools. Ordinary commands, including copies of these
tools at other paths, receive only their existing command-specific grants.

## Editing and history

Up/Down recalls up to 32 commands from this shell instance. Down past the newest
entry restores the unfinished draft. Editing a recalled line does not change the
saved entry. Ctrl-U clears the line, Ctrl-L clears/redraws the screen, Ctrl-C
cancels input, and Backspace removes the last UTF-8 character. Unsupported terminal
CSI key sequences are discarded instead of becoming command text.

History is not written to disk and disappears when the session restarts the
shell. Consecutive duplicates, empty lines, rejected overlong lines and commands
starting with whitespace are not saved. A leading space can therefore keep a
sensitive command out of history, though it is still echoed to the terminal.

## Pipelines and files

```sh
ls -1 /system/bin | grep '^c'
cat /data/vms.json | grep -n image
echo first > /notes
echo second >> /notes
grep -i second < /notes > /matches
cat /missing 2> /errors
pwd | grep /
```

`|` connects one command's stdout to the next command's stdin. All stages run
concurrently; stdout data passes directly between their byte channels with
bounded backpressure. The shell waits for every stage and reports failure
according to the last stage's exit status.

`<` reads stdin from a file, `>` creates or truncates stdout's file, and `>>`
appends. `2>` and `2>>` do the same for stderr. Redirects apply after pipe
connections and are opened left to right; the last redirect for a stream wins.
Paths may be quoted, and operators do not require surrounding spaces. File
transfers use bounded shell buffers and standard filesystem APIs, without
collecting complete command output in memory. File I/O currently runs on the
shell's relay thread; this is not an asynchronous disk-I/O implementation.

Single/double quotes and backslash escaping protect literal operators. A `#`
at the beginning of a word starts a comment. There is no variable expansion,
command substitution, globbing, background execution, `&&`/`||`, descriptor
duplication (`2>&1`) or here-document support. Lines are limited to 512 bytes,
commands to 32 words including the command name, and pipelines to eight stages.

`pwd`, `help` and `clear` can run as pipeline stages or redirect their output.
`cd` and `exit` require a standalone command without redirection. A command with
unredirected terminal stdin receives a scoped terminal lease; in a pipeline this
is the first stage. Its default input is canonical. Ctrl-D after a partial line
makes that line readable, and at an empty line produces one zero-length read;
it does not permanently close the input stream. Commands that select raw mode
receive Ctrl-D as an ordinary byte. At an empty shell prompt, Ctrl-D exits the
shell. This is not Unix terminal job control.

## echo and cat

`echo -n TEXT` omits the trailing newline. `-e` interprets `\\`, `\a`, `\b`,
`\e`, `\f`, `\n`, `\r`, `\t`, `\v`, `\0NNN` (up to three octal digits)
and `\xHH` (up to two hex digits). `\c` stops all remaining output, including
the newline. `-E` restores literal backslashes; the last `-e`/`-E` wins. Unknown
escapes remain literal. Options precede the first text operand; unknown options
such as `--help` are text. Use `--` to make a leading `-n` or `-e` an operand.

`cat -n` numbers every line, `-b` numbers only nonempty lines and overrides `-n`,
and `-s` squeezes repeated empty lines. Numbering and empty-line state continue
across file boundaries, including an input without a final newline. With no
options, data passes through unchanged, including binary bytes.

## grep

`grep PATTERN [FILE ...]` reads stdin when no file is supplied; `-` explicitly
selects stdin. Useful options are:

- `-i`: ignore ASCII case; `-v`: select nonmatching lines.
- `-n`: show line numbers; `-c`: count selected lines per input.
- `-l`: print names of matching inputs; `-q`: stop after a match, without output.
- `-m NUM`: stop after NUM selected lines per input (`0` reads no lines).
- `-x`: require a whole-line match; `--line-buffered`: flush each selected line.
- `-F`: match literal text; `-e PATTERN`: supply multiple alternative patterns.
- `-H` / `-h`: force / suppress filename prefixes (automatic for multiple files).

Patterns use the Rust regex crate's byte-oriented syntax, including `^`, `$`,
character classes and alternation. They are not POSIX basic regular expressions;
backreferences and lookaround are not supported. Input need not be UTF-8, and
matching excludes each line's trailing newline. A missing final newline is
added when printing a selected line.

Exit statuses are 0 for a match, 1 for no match, and 2 for an error. `-q` returns
0 once a match is found even if an earlier input failed. Use `--help` for help;
`-h` is reserved for suppressing filenames.
