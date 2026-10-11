#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

"""Check terminal expectations against multiplexed kernel log records."""
import importlib.util
from functools import partial
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

from guest_console import (CONSOLE_READY, UART_DRAINED, append_console_output,
                           guest_boot_deadline, guest_console_startup, startup_ack_command,
                           startup_drain_command, synchronize_guest_console)
from session import Session

spec = importlib.util.spec_from_file_location(
    'verify_console', Path(__file__).with_name('verify-console.py'))
console = importlib.util.module_from_spec(spec)
spec.loader.exec_module(console)


class ConsoleTests(unittest.TestCase):
    @staticmethod
    def identity(*, transport='virtio', tty='hvc0', consoles='ttyAMA0',
                 uart='arm,sbsa-uart', device='0x0003'):
        return (f'CONSOLE_ID_BEGIN\n'
                f'cmdline:console={consoles.split()[0]} earlycon hyper.console={transport}\n'
                f'tty:/dev/{tty}\nkernel:{consoles}\nuart:{uart}\n'
                f'virtio:virtio,mmio\nvirtio-device:{device}\nCONSOLE_ID_END\n').encode()

    def test_guest_virtio_tty_keeps_printk_on_the_architecture_uart(self):
        console.verify_guest_console_identity(self.identity(), aarch64=True,
                                              expected_transport='virtio')
        console.verify_guest_console_identity(
            self.identity(consoles='ttyS0', uart='ns16550a'), aarch64=False,
            expected_transport='virtio')
        # The explicit UART mode remains useful for local A/B measurements.
        console.verify_guest_console_identity(
            self.identity(transport='uart', tty='ttyAMA0'), aarch64=True,
            expected_transport='uart')

    def test_guest_console_identity_rejects_fallback_and_duplicate_printk(self):
        for output in (
            self.identity(tty='ttyAMA0'),
            self.identity(consoles='ttyAMA0 hvc0'),
            self.identity(consoles='hvc0'),
            self.identity(uart='arm,pl011'),
            self.identity(device='0x0001'),
            self.identity(transport='unknown'),
            self.identity().replace(b'tty:/dev/hvc0\n', b''),
            self.identity().replace(b'tty:/dev/hvc0\n', b'tty:/dev/hvc0\ntty:/dev/hvc0\n'),
            self.identity().replace(b'CONSOLE_ID_BEGIN\n', b''),
            self.identity().replace(b'CONSOLE_ID_END\n', b''),
            self.identity() * 2,
        ):
            with self.subTest(output=output), self.assertRaises(AssertionError):
                console.verify_guest_console_identity(output, aarch64=True)
        with self.assertRaises(AssertionError):
            console.verify_guest_console_identity(
                self.identity(transport='uart', tty='ttyAMA0'), aarch64=True,
                expected_transport='virtio')

    def test_guest_identity_ignores_wrapped_command_echo_outside_markers(self):
        echo = console.guest_console_identity_command(True).replace(
            b"printf 'virtio:'", b"printf '\nvirtio:'")
        output = b'\x1b[6n' + echo + self.identity() + b'~ # '
        console.verify_guest_console_identity(output, aarch64=True,
                                              expected_transport='virtio')
        # Fields outside the marker block do not describe this identity sample.
        console.verify_guest_console_identity(
            self.identity() + b'tty:/dev/ttyAMA0\n', aarch64=True)

    def test_guest_identity_tolerates_a_linux_record_interrupting_output(self):
        output = self.identity().replace(
            b'tty:/dev/hvc0', b'tty:/dev/[ 17.230001] random: crng init done\nhvc0')
        console.verify_guest_console_identity(output, aarch64=True)

    def test_guest_echo_cli_reassembles_printk_but_rejects_panics(self):
        command = [sys.executable, '-B', str(Path(__file__).with_name('guest_console.py')),
                   '--filter-guest-logs']
        output = subprocess.run(command, input=(
            b'e[   19.492054] clk: Disabling unused clocks\r\n'
            b'cho HYPER_GUEST_CONSOLE_RX\r\nHYPER_GUEST_CONSOLE_RX\r\n'),
            capture_output=True, check=True)
        self.assertEqual(output.stdout,
                         b'echo HYPER_GUEST_CONSOLE_RX\r\nHYPER_GUEST_CONSOLE_RX\r\n')
        failure = subprocess.run(command, input=b'[ 20.0] Kernel panic - not syncing\r\n',
                                 capture_output=True)
        self.assertNotEqual(failure.returncode, 0)
        self.assertEqual(failure.stdout, b'')

    def test_startup_handshake_preserves_hvc_prompt_inside_uart_printk(self):
        # Replay the CI failure: filtering the complete printk line removes
        # the only interactive prompt even though hvc and userspace are ready.
        mixed = (b'[ HypeR guest: /init reached\r\n'
                 b' HypeR guest: Linux userspace is running3\r\n'
                 b'5.451684] Asymmetric key parser registered\r\n'
                 b'[   35.454723] Block lay~ # e\x1b[6nr SCSI generic loaded\r\n')
        filtered = bytearray()
        append_console_output(filtered, mixed, filter_guest_logs=True)
        self.assertNotIn(b'~ # ', filtered)
        for qemu in ('/usr/bin/qemu-system-aarch64', '/usr/bin/qemu-system-riscv64'):
            with self.subTest(qemu=qemu), tempfile.TemporaryDirectory() as directory:
                drain = startup_drain_command(qemu)
                ack = startup_ack_command()
                script = (
                    'import sys,time\n'
                    'def emit(data):\n'
                    ' sys.stdout.buffer.write(data); sys.stdout.flush()\n'
                    "assert sys.stdin.buffer.readline() == b'vmm console alpine\\n'\n"
                    f"emit(b'Connected to alpine.\\r\\n' + {mixed!r})\n"
                    f'assert sys.stdin.buffer.readline() == {drain!r}\n'
                    # The echoed source contains the marker text, but never
                    # the complete marker line required by the handshake.
                    f'emit({drain!r})\n'
                    "emit(b'[ 42.644524] Run /init as init process\\r\\n')\n"
                    f"emit(b'\\r\\n' + {UART_DRAINED!r} + b'\\r\\n')\n"
                    "assert sys.stdin.buffer.readline() == b'\\n'\n"
                    f'assert sys.stdin.buffer.readline() == {ack[1:]!r}\n'
                    f"emit(b'\\r\\n' + {CONSOLE_READY!r} + b'\\r\\n~ # ')\n"
                    "assert sys.stdin.buffer.readline() == b'echo AFTER_DRAIN\\n'\n"
                    "emit(b'AFT[ 43.0] diagnostic\\r\\nER_DRAIN\\r\\n~ # ')\n"
                    'time.sleep(30)\n')
                output_filter = partial(append_console_output, filter_guest_logs=True)
                with Session([sys.executable, '-u', '-c', script],
                             Path(directory) / 'console.log',
                             output_filter=output_filter) as session:
                    with guest_console_startup(session):
                        session.send(b'vmm console alpine\n')
                        session.await_text(rb'Connected to alpine\.', timeout=5)
                        session.await_text(rb'~ # ', timeout=5)
                        synchronize_guest_console(session.send, session.await_text,
                                                  qemu, timeout=5)
                    self.assertIs(session.output_filter, output_filter)
                    session.send(b'echo AFTER_DRAIN\n')
                    session.await_text(rb'AFTER_DRAIN\n~ # ', timeout=5)
                self.assertIn(mixed, (Path(directory) / 'console.log').read_bytes())

    def test_startup_cli_uses_the_same_architecture_commands(self):
        helper = str(Path(__file__).with_name('guest_console.py'))
        for architecture, uart in (('aarch64', b'ttyAMA0'), ('riscv64', b'ttyS0')):
            qemu = f'/usr/bin/qemu-system-{architecture}'
            command = subprocess.run(
                [sys.executable, '-B', helper, '--startup-command', 'drain', '--qemu', qemu],
                capture_output=True, check=True).stdout
            self.assertEqual(command, startup_drain_command(qemu))
            self.assertIn(b'/dev/' + uart, command)
            self.assertIsNone(re.search(rb'\n' + UART_DRAINED + rb'\r?\n', command))
        command = subprocess.run(
            [sys.executable, '-B', helper, '--startup-command', 'ack'],
            capture_output=True, check=True).stdout
        self.assertEqual(command, startup_ack_command())
        self.assertIsNone(re.search(rb'\n' + CONSOLE_READY + rb'\r?\n~ # ', command))

    def test_boot_evidence_survives_a_host_record_inside_guest_printk(self):
        raw = (b'[ 28.348014] Run /init as in<4>[ 51.268594] HypeR: '
               b'masked virtual timer PPI without an active vCPU\r\n'
               b'it process\r\n')
        command = [sys.executable, '-B', str(Path(__file__).with_name('guest_console.py'))]
        output = subprocess.run(command, input=raw, capture_output=True, check=True).stdout
        self.assertEqual(output, b'[ 28.348014] Run /init as init process\r\n')

    def test_startup_filter_is_restored_after_timeout(self):
        with tempfile.TemporaryDirectory() as directory:
            output_filter = partial(append_console_output, filter_guest_logs=True)
            with Session([sys.executable, '-u', '-c', 'import time; time.sleep(30)'],
                         Path(directory) / 'console.log', output_filter=output_filter) as session:
                with self.assertRaises(TimeoutError):
                    with guest_console_startup(session):
                        synchronize_guest_console(session.send, session.await_text,
                                                  'qemu-system-aarch64', timeout=0.02)
                self.assertIs(session.output_filter, output_filter)

    def test_guest_startup_waits_share_one_deadline(self):
        # Completing one boot marker cannot grant the next marker a fresh
        # full timeout; command scenarios outside startup keep their own limits.
        with patch('guest_console.time.monotonic', side_effect=(100, 101, 104, 105)):
            remaining = guest_boot_deadline(5)
            self.assertEqual(remaining(), 4)
            self.assertEqual(remaining(), 1)
            with self.assertRaises(TimeoutError):
                remaining()
        session = Mock(output_filter=None)
        with patch('guest_console.time.monotonic', side_effect=(100, 103, 105)):
            with guest_console_startup(session, timeout=5) as remaining:
                self.assertEqual(remaining(), 2)
                with self.assertRaises(TimeoutError):
                    remaining()
        self.assertIsNone(session.output_filter)

    def test_guest_boot_budget_cli_respects_explicit_override(self):
        command = [sys.executable, '-B', str(Path(__file__).with_name('guest_console.py')),
                   '--boot-timeout']
        environment = dict(os.environ, QEMU_GUEST_BOOT_TIMEOUT_SECONDS='17')
        output = subprocess.run(command, env=environment, capture_output=True, check=True)
        self.assertEqual(output.stdout, b'17\n')
        for invalid in ('0', '-1', 'invalid'):
            environment['QEMU_GUEST_BOOT_TIMEOUT_SECONDS'] = invalid
            output = subprocess.run(command, env=environment, capture_output=True)
            self.assertNotEqual(output.returncode, 0)
            self.assertEqual(output.stdout, b'')

    def test_interleaved_kernel_records_preserve_terminal_contract(self):
        pattern = console.console_lines(b'inherit-data', b'TERMINAL_INHERIT_OK', b'hyper-sh$ ')
        record = b'<4>[    6.454394] HypeR: masked virtual timer PPI without an active vCPU\n'
        for separator in (b'', record, record * 2):
            output = b'inherit-data\n' + separator + b'TERMINAL_INHERIT_OK\n' + separator + b'hyper-sh$ '
            self.assertIsNotNone(re.fullmatch(pattern, output))
        for output in (
            b'inherit-data\nunexpected\nTERMINAL_INHERIT_OK\nhyper-sh$ ',
            b'inherit-data\nhyper-sh$ ',
            b'TERMINAL_INHERIT_OK\ninherit-data\nhyper-sh$ ',
            b'inherit-data\nTERMINAL_INHERIT_OK\n',
            b'inherit-data\n<4>[    6.4] HypeR: incomplete TERMINAL_INHERIT_OK\nhyper-sh$ ',
        ):
            self.assertIsNone(re.fullmatch(pattern, output))


if __name__ == '__main__':
    unittest.main()
