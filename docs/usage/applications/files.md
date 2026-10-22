<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# File tools


The normal profile installs these independent std/clap applications under
`/bin`. The default shell PATH searches only that directory, on the independent system root:

| Command | Supported operations |
| --- | --- |
| `ls [-ad1rtS] [--bytes] PATH...` | List entries or directory operands (`-d`), sort by name/size/modification time, and show permissions, sizes and symlink targets (`name@ -> target`, including dangling links). `-1` omits metadata and targets but retains known type suffixes. Unreadable child attributes appear as `?`; healthy entries remain visible, the failed path is reported on stderr, and the command returns failure. |
| `cat [-nbs] FILE...` | Stream files or stdin; number all/nonblank lines and squeeze repeated blank lines. State continues across inputs. |
| `grep [-ix] [-m NUM] PATTERN FILE...` | Filter text; `-m NUM` limits selected lines per input, `-x` requires a whole-line match, and `--line-buffered` flushes selected lines. See [text filtering](shell.md#grep). |
| `echo [-n] [-e\|-E] TEXT...` | Print arguments, optionally omit the newline or interpret backslash escapes. `--` ends option parsing. |
| `cp [-RnvT] SOURCE... DEST` | Copy files or trees; `-n` skips existing entries using exclusive file creation, `-T` selects an exact destination, and `-v` reports copied files/links. |
| `mv [-vT] SOURCE... DEST` | Rename files, links, or directories; `-T` selects an exact destination and `-v` reports moves. Multiple sources require a directory. |
| `ln [-snvT] TARGET LINK` | Create hard or symbolic links; `-n` does not enter a symlinked destination directory, `-T` treats LINK as an exact name, and `-v` reports the link. |
| `rm [-rdfv] PATH...` | Remove files or trees; `-d` removes only empty directories, `-f` ignores missing paths, and `-v` reports removed operands. |
| `chmod [-Rv] MODE PATH...` | Octal modes or symbolic clauses such as `u+rw,go-rwx`; `-v` shows each processed path and its old/new mode. |
| `mkdir [-pv] [-m OCTAL] PATH...` | Create directories and optional parents; `-v` reports newly created operand directories. Explicit modes apply to new final directories. |
| `rmdir [-pv] PATH...` | Remove empty directories; `-p` then removes empty ancestors, stopping at the first error, and rejects `..` components. `-v` reports each removal. |
| `touch [-acm] [-r REFERENCE] [-d @SECONDS] PATH...` | Create files or update timestamps without truncation. `-d` accepts nonnegative Unix seconds with up to nine fractional digits; it conflicts with `-r`. |

Tools accept `--help` (except `echo`, where it is text) and `--` before operands
beginning with `-`. File operations report path-specific errors and return
failure if any requested operation fails. `grep` returns 1 for no matches and
2 for errors; its `-q` mode can return success after finding a match despite an
earlier input error.
`chmod` treats an omitted user/group/other selector as `a`; Native mode bits do
not imply a Unix credential or umask implementation. Recursive chmod skips
nested symbolic links; an explicitly named symbolic link changes its target.

Recursive `cp` preserves symbolic links, including dangling links. It refuses
existing destination symlinks, same-file copies (including hard links), and
copying a directory into itself. Regular destination files are overwritten unless `-n` is selected.
`ln` refuses existing link names. `mv` uses filesystem rename and reports
cross-filesystem moves as unsupported by that operation; it does not silently
fall back to a non-atomic copy/delete sequence. Recursive `rm` does not follow
symbolic links and refuses the root and final `.`/`..` operands. These are basic
file tools, not full GNU coreutils option compatibility.
