#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

"""Exercise paced terminal input, stale readiness, and bidirectional pressure."""
import os
import random
import re
import sys

from session import Session, native_command
from guest_console import GUEST_LOG, append_console_output


def console_lines(*lines):
    """Match exact terminal lines while allowing interleaved kernel records."""
    kernel_record = rb'<[0-7]>\[[ ]*[0-9]+\.[0-9]+\] HypeR: [^\n]*\n'
    boundary = rb'\n(?:' + kernel_record + rb')*'
    return rb'(?m)^' + boundary.join(re.escape(line) for line in lines)


def guest_console_identity_command(aarch64):
    uart = 'serial@9000000' if aarch64 else 'soc/serial@10000000'
    virtio = 'virtio_mmio@a002000' if aarch64 else 'soc/virtio_mmio@10001000'
    tree = '/sys/firmware/devicetree/base/'
    return (
        "echo CONSOLE_ID_BEGIN; printf 'cmdline:'; cat /proc/cmdline; "
        "printf 'tty:'; tty; printf 'kernel:'; cat /sys/class/tty/console/active; "
        f"printf 'uart:'; tr -d '\\000' < {tree}{uart}/compatible; echo; "
        f"printf 'virtio:'; tr -d '\\000' < {tree}{virtio}/compatible; echo; "
        "for d in /sys/bus/virtio/devices/*; do "
        "printf 'virtio-device:'; cat \"$d/device\"; done; echo CONSOLE_ID_END\n"
    ).encode()


def verify_guest_console_identity(output, *, aarch64, expected_transport=None):
    """Check the actual tty and printk route independently of configured intent."""
    # Linux boot work may finish while the shell prints these records. Preserve
    # raw diagnostics in Session's log but remove complete printk lines here.
    output = GUEST_LOG.sub(b'', output)
    # The echoed command can wrap onto a line beginning with a field name.
    # Parse only records emitted between complete marker lines, never the echo.
    blocks = re.findall(
        rb'(?ms)^CONSOLE_ID_BEGIN\n(.*?)^CONSOLE_ID_END\n', output)
    if len(blocks) != 1:
        raise AssertionError(f'guest console identity lacks unique marker block: {output!r}')
    output = blocks[0]
    fields = {}
    for name in (b'cmdline', b'tty', b'kernel', b'uart', b'virtio'):
        values = re.findall(rb'(?m)^' + name + rb':([^\n]*)$', output)
        if len(values) != 1:
            raise AssertionError(f'guest console identity lacks unique {name!r}: {output!r}')
        fields[name] = values[0]
    transport = b'uart'
    for argument in fields[b'cmdline'].split():
        if argument.startswith(b'hyper.console='):
            transport = argument.partition(b'=')[2]
    if transport not in (b'uart', b'virtio'):
        raise AssertionError(f'invalid guest console transport: {transport!r}')
    if expected_transport is not None and transport != expected_transport.encode():
        raise AssertionError(f'expected {expected_transport} console, got {transport!r}')
    uart_tty = b'ttyAMA0' if aarch64 else b'ttyS0'
    expected_tty = b'hvc0' if transport == b'virtio' else uart_tty
    if fields[b'tty'] != b'/dev/' + expected_tty:
        raise AssertionError(f'shell tty does not match transport: {fields!r}')
    consoles = fields[b'kernel'].split()
    if uart_tty not in consoles or b'hvc0' in consoles:
        raise AssertionError(f'kernel diagnostics must remain on the UART: {fields!r}')
    expected_uart = b'arm,sbsa-uart' if aarch64 else b'ns16550a'
    if fields[b'uart'] != expected_uart or fields[b'virtio'] != b'virtio,mmio':
        raise AssertionError(f'guest device-tree console identity mismatch: {fields!r}')
    devices = re.findall(rb'(?m)^virtio-device:(0x[0-9a-fA-F]+)$', output)
    if not any(int(device, 16) == 3 for device in devices):
        raise AssertionError(f'virtio console device ID 3 was not discovered: {output!r}')


def main():
    qemu, image, initramfs, logfile = sys.argv[1:]
    verify_vm = os.environ.get("HYPER_TEST_VM", "1")
    if verify_vm not in ("0", "1"):
        raise ValueError("HYPER_TEST_VM must be 0 or 1")
    verify_vm = verify_vm == "1"
    command = native_command(qemu, image, initramfs)
    with Session(command, logfile, output_filter=append_console_output) as session:
        pending = session.pending
        pump = session.pump
        send = session.send
        rng = random.Random(761)

        def await_text(pattern, timeout=10):
            return session.await_text(pattern, timeout)

        def typed(text):
            for byte in text.encode():
                send(bytes([byte]))
                pump(rng.uniform(0.015, 0.12))
                # Do not send a rescue key: it can accidentally wake a router
                # blocked on the wrong input and conceal the original fault.
                await_text(re.escape(bytes([byte])))
            send(b'\r')

        def run(text, expected, timeout=30):
            send(text.encode() + b'\n')
            output = await_text(rb'hyper-sh\$ ', timeout=timeout)
            if not re.search(expected, output) or b'sh: command failed' in output:
                raise AssertionError(f'{text}: {output!r}')

        # Independently scheduled services can log between session readiness
        # and the first prompt. Require both in order, not adjacent bytes.
        await_text(rb'HypeR session: console ready\n', timeout=60)
        await_text(rb'hyper-sh\$ ', timeout=60)
        pump(5)
        pending.clear()
        run('echo HISTORY_RECALL', rb'\nHISTORY_RECALL\n')
        # Split the escape sequence across transport writes, as real terminals do.
        for part in (b'\x1b', b'[', b'A\n'):
            send(part)
            pump(0.01)
        await_text(console_lines(b'HISTORY_RECALL', b'hyper-sh$ '))
        send(b'echo DISCARDED\x15echo LINE_CLEARED\n')
        await_text(console_lines(b'LINE_CLEARED', b'hyper-sh$ '))
        # Deliberately send typeahead in one burst: a non-reading child
        # must never steal the following command into a disposable pipe.
        send(b'echo TYPEAHEAD_FIRST\necho TYPEAHEAD_SECOND\n')
        await_text(rb'(?m)^TYPEAHEAD_FIRST\n')
        await_text(console_lines(b'TYPEAHEAD_SECOND', b'hyper-sh$ '))
        # CRLF may be split by the hardware; EOF belongs only to cat.
        send(b'cat\r')
        pump(0.03)
        send(b'\nterminal-cat\r\n\x04echo AFTER_TERMINAL_EOF\r\n')
        await_text(rb'(?m)^terminal-cat\n')
        await_text(console_lines(b'AFTER_TERMINAL_EOF', b'hyper-sh$ '))
        send(b'/system/bin/std-test --child terminal-line\r\nterminal-line\r\n')
        await_text(console_lines(b'TERMINAL_LINE_OK', b'hyper-sh$ '))
        send(b'/system/bin/std-test --child terminal-inherit\ninherit-data\r\n\x04')
        await_text(console_lines(b'inherit-data', b'TERMINAL_INHERIT_OK', b'hyper-sh$ '))
        run('/system/bin/std-test --child binary-eof-byte', rb'BINARY_EOF_BYTE_OK')
        # The console service survives both a requested shell exit and EOF.
        for exit_input in (b'exit\n', b'\x04'):
            send(exit_input)
            await_text(rb'HypeR virtual console: shell exited; restarting\n')
            await_text(console_lines(b'HypeR session: console ready', b'hyper-sh$ '))
            run('echo AFTER_SHELL_RESTART', rb'\nAFTER_SHELL_RESTART\n')
        rounds = int(os.environ.get('CONSOLE_TYPED_ROUNDS', '40'))
        for index in range(rounds):
            token = f'CONSOLE_{index:03d}'
            typed(f'echo {token}')
            await_text(console_lines(token.encode(), b'hyper-sh$ '))
            pump(rng.uniform(0.01, 0.3))
        # Exercise the idle-to-input transition without a continuously
        # queued producer masking a missed notification.
        pump(3)
        typed('echo AFTER_IDLE')
        await_text(console_lines(b'AFTER_IDLE', b'hyper-sh$ '))
        for _ in range(8):
            for _ in range(200):
                send(b'x' * 16)
                pump(0.002)
            pump(0.3)
            send(b'\x03')
            await_text(rb'\^C\n')
            await_text(rb'hyper-sh\$ ')
            run('echo AFTER_BURST', rb'\nAFTER_BURST\n')
        send(b'top -d 0.1\n')
        await_text(rb'CPU: user-thread')
        pump(1)
        send(b'q')
        await_text(rb'hyper-sh\$ ')
        run('cat /boot/etc/hyper/vms.json', rb'"hyper.vm-config"')
        # A surviving descendant may retain stdout; command termination
        # must drain available output without waiting for that child's EOF.
        run('/system/bin/std-test --child detached-output', rb'OUTPUT_OWNER_EXITED', timeout=5)
        if verify_vm:
            send(b'vmm console alpine\n')
            await_text(rb'Connected to alpine\.', timeout=30)
            await_text(rb'~ # ', timeout=60)
            aarch64 = 'aarch64' in os.path.basename(qemu)
            uart_tty = 'ttyAMA0' if aarch64 else 'ttyS0'
            # The first hvc prompt can overtake boot diagnostics still draining
            # through the UART. Flush that stream before strict terminal checks.
            # Keep emergency printk visible for the diagnostic-path check below.
            send(("PS1=''; dmesg -n 1; printf '\\nCONSOLE_UART_DRAINED\\n' "
                  f"> /dev/{uart_tty}; read -r console_ack\n").encode())
            await_text(rb'\nCONSOLE_UART_DRAINED\n', timeout=120)
            send(b"\nPS1='~ # '; printf '\\nCONSOLE_READY\\n'\n")
            await_text(rb'\nCONSOLE_READY\n~ # ')
            send(guest_console_identity_command(aarch64))
            output = await_text(rb'\nCONSOLE_ID_END\n~ # ')
            verify_guest_console_identity(
                output, aarch64=aarch64,
                expected_transport=os.environ.get('HYPER_TEST_GUEST_CONSOLE', 'virtio'))
            # Kernel log emission uses the diagnostic UART even while the
            # interactive tty is hvc0. This also checks that UART service stays
            # live after the virtio driver has initialized both queues.
            send(b"PS1=''; printf '<0>HYPER_UART_LOG_PATH\\n' > /dev/kmsg; read -r console_ack\n")
            await_text(rb'\[[ ]*[0-9]+\.[0-9]+\] HYPER_UART_LOG_PATH\n')
            # Keep the hvc prompt silent until the UART record is complete;
            # the two independent streams can otherwise interleave mid-record.
            send(b"\nPS1='~ # '; printf '\\nCONSOLE_LOG_VERIFIED\\n'\n")
            await_text(rb'\nCONSOLE_LOG_VERIFIED\n~ # ')
            # Each character must wake the sleeping runtime and echo without
            # a following key or periodic collection timeout to rescue it.
            for index in range(12):
                token = f'GUEST_WAKE_{index:02d}'
                typed(f'echo {token}')
                await_text(rb'\n' + token.encode() + rb'\n~ # ')
                pump(0.2)
            # Span several transport messages and compare every byte in order.
            # This is a correctness check, not a throughput requirement for CI.
            send(b"i=0; while [ $i -lt 256 ]; do "
                 b"printf 'CONSOLE-%03d-abcdefghijklmnopqrstuvwxyz\\n' $i; "
                 b"i=$((i+1)); done\n")
            expected = b''.join(
                f'CONSOLE-{index:03d}-abcdefghijklmnopqrstuvwxyz\n'.encode()
                for index in range(256))
            await_text(rb'\n' + re.escape(expected) + rb'~ # ', timeout=120)
            # A continuously busy TX queue must still consume input readiness.
            # Without polling between service batches, Ctrl-C can starve forever.
            send(b'yes CONSOLE_BUSY\n')
            await_text(rb'\nCONSOLE_BUSY\n')
            send(b'\x03')
            await_text(rb'~ # ', timeout=30)
            send(b'echo GUEST_AFTER_BUSY\n')
            await_text(rb'\nGUEST_AFTER_BUSY\n~ # ')
            pump(3)
            typed('echo GUEST_AFTER_IDLE')
            await_text(rb'\nGUEST_AFTER_IDLE\n~ # ')
            send(b'\x1d')
            await_text(rb'd/q: detach, any other key: resume')
            pump(0.2)
            send(b'q')
            await_text(console_lines(b'[vmm] detached', b'hyper-sh$ '))
        typed('echo CONSOLE_OK')
        await_text(console_lines(b'CONSOLE_OK', b'hyper-sh$ '))
        print(f'verified {rounds} paced commands, idle input, burst recovery, and top return' +
              (' with guest console wakeups' if verify_vm else ''))


if __name__ == '__main__':
    main()
