#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

"""Reassemble guest console bytes without hiding host or guest failures."""

import argparse
from contextlib import contextmanager
import os
import re
import sys
import time


HOST_LOG = re.compile(rb'<[0-7]>\[[ \t]*[0-9]+\.[0-9]+\] [^\n]*\n')
GUEST_LOG = re.compile(rb'\[[ \t]*[0-9]+\.[0-9]+\] [^\n]*\n')
FAILURE_MARKERS = (
    b'HypeR: fatal', b'kernel panic', b'Kernel panic', b'HypeR KERNEL PANIC',
    b'HypeR crash monitor', b'kernel startup failed',
)
UART_DRAINED = b'HYPER_GUEST_UART_DRAINED'
CONSOLE_READY = b'HYPER_GUEST_READY'


def uart_console(qemu):
    """Select the guest diagnostic UART for the architecture under test."""
    name = os.path.basename(qemu)
    if 'aarch64' in name:
        return 'ttyAMA0'
    if 'riscv64' in name:
        return 'ttyS0'
    raise ValueError(f'unknown QEMU guest console architecture: {qemu}')


def startup_drain_command(qemu):
    # Keep the interactive shell in read: even an empty PS1 can issue ESC[6n
    # while entering line editing and split the independent UART marker.
    return (b"PS1=''; dmesg -n 1; printf '\\n" + UART_DRAINED
            + b"\\n' > /dev/" + uart_console(qemu).encode()
            + b'; read -r console_ack\n')


def startup_ack_command():
    return b"\nPS1='~ # '; printf '\\n" + CONSOLE_READY + b"\\n'\n"


@contextmanager
def guest_console_startup(session):
    """Preserve hvc bytes inside UART printk lines until both streams drain.

    Enter before attaching so a read that contains both the attach reply and
    the first prompt cannot discard that prompt through printk filtering.
    Host records and failure detection remain active throughout the handshake.
    """
    saved_filter = session.output_filter
    session.output_filter = append_console_output
    try:
        yield
    finally:
        session.output_filter = saved_filter


def synchronize_guest_console(send, await_text, qemu, *, timeout=60):
    """After the first prompt, drain UART and require a fresh hvc/tty reply.

    Both callbacks transfer complete byte strings; send must not append a
    newline. Linux printk filtering must remain disabled until this returns.
    Informational printk is suppressed only in this test guest; emergency
    diagnostics remain visible. One deadline bounds the entire handshake.
    """
    deadline = time.monotonic() + timeout

    def expect(pattern):
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError('guest console startup handshake timed out')
        return await_text(pattern, timeout=remaining)

    send(startup_drain_command(qemu))
    expect(rb'\n' + UART_DRAINED + rb'\r?\n')
    send(startup_ack_command())
    expect(rb'\n' + CONSOLE_READY + rb'\r?\n~ # ')


def append_console_output(pending, data, *, preserve_cr=False, filter_guest_logs=False):
    """Reassemble console text across complete interleaved host log records.

    The caller archives raw bytes first. Keep unfinished records in pending so
    arbitrary read boundaries do not leak log fragments into prompt matching.
    Check failures before removing records, and again after joining guest text.
    Linux printk filtering is opt-in for tests matching shell responses rather
    than boot diagnostics. Raw guest diagnostics remain in the caller's log.
    """
    pending.extend(data if preserve_cr else data.replace(b'\r', b''))
    if any(marker in pending for marker in FAILURE_MARKERS):
        raise RuntimeError('host or guest kernel failure')
    pending[:] = HOST_LOG.sub(b'', pending)
    # Host records can themselves split a guest panic message. Check the
    # reconstructed guest stream before filtering any guest printk records.
    if any(marker in pending for marker in FAILURE_MARKERS):
        raise RuntimeError('host or guest kernel failure')
    if filter_guest_logs:
        pending[:] = GUEST_LOG.sub(b'', pending)
    if any(marker in pending for marker in FAILURE_MARKERS):
        raise RuntimeError('host or guest kernel failure')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--filter-guest-logs', action='store_true',
                        help='remove Linux printk records when matching shell responses')
    parser.add_argument('--startup-command', choices=('drain', 'ack'),
                        help='emit a guest startup handshake command for shell tests')
    parser.add_argument('--qemu', help='QEMU executable, used to select the diagnostic UART')
    args = parser.parse_args()
    if args.startup_command:
        if args.filter_guest_logs:
            parser.error('startup commands cannot filter logs')
        if args.startup_command == 'drain' and args.qemu is None:
            parser.error('--startup-command drain requires --qemu')
        sys.stdout.buffer.write(startup_drain_command(args.qemu)
                                if args.startup_command == 'drain' else startup_ack_command())
        sys.exit(0)
    output = bytearray()
    append_console_output(output, sys.stdin.buffer.read(), preserve_cr=True,
                          filter_guest_logs=args.filter_guest_logs)
    sys.stdout.buffer.write(output)
