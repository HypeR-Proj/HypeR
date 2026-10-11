#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

"""Check terminal expectations against multiplexed kernel log records."""
import importlib.util
from pathlib import Path
import re
import subprocess
import sys
import unittest

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
