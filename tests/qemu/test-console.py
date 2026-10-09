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
