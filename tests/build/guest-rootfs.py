#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0
"""Check guest ext4 packaging preserves Linux ownership and failed-build output."""
import importlib.util
import io
import json
import os
from pathlib import Path
import pty
import shlex
import subprocess
import sys
import tarfile
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
spec = importlib.util.spec_from_file_location('guest_disk', ROOT / 'scripts/pack-guest-disk.py')
guest_disk = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guest_disk)


class GuestConsoleTests(unittest.TestCase):
    def run_init(self, arguments, *, virtio=False, module_ok=True):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for directory in ('proc', 'sys/class/tty/console', 'sys/class/net',
                              'dev', 'etc', 'root', 'bin'):
                (root / directory).mkdir(parents=True, exist_ok=True)
            (root / 'proc/cmdline').write_text(arguments + '\n')
            (root / 'sys/class/tty/console/active').write_text('ttyAMA0\n')
            master, slave = pty.openpty()
            self.addCleanup(os.close, master)
            self.addCleanup(os.close, slave)
            if virtio:
                (root / 'dev/hvc0').symlink_to(os.ttyname(slave))
            log = root / 'calls'
            busybox = root / 'bin/busybox'
            busybox.write_text(
                '#!/bin/sh\n'
                f'printf "%s\\n" "$*" >> {shlex.quote(str(log))}\n'
                'case "$1" in\n'
                'mkdir) shift; exec /bin/mkdir "$@" ;;\n'
                f'modprobe) if [ "$2" = virtio_console ]; then exit {0 if module_ok else 1}; fi ;;\n'
                'esac\nexit 0\n')
            busybox.chmod(0o755)
            # Run the real init control flow against private filesystem and
            # command fixtures, without mounting or changing host networking.
            script = (ROOT / 'kernel/tools/guest/init').read_text()
            for path in ('/bin/busybox', '/proc', '/sys', '/dev', '/etc', '/root'):
                script = script.replace(path, str(root) + path)
            result = subprocess.run(['/bin/sh', '-c', script, 'init', 'root-mounted'],
                                    text=True, capture_output=True, timeout=5)
            os.set_blocking(master, False)
            output = bytearray()
            while True:
                try:
                    output.extend(os.read(master, 4096))
                except BlockingIOError:
                    break
            inittab = root / 'etc/inittab'
            return (result, inittab.read_text() if inittab.exists() else None,
                    log.read_text() if log.exists() else '', output.decode())

    def test_uart_remains_the_default_and_explicit_fallback(self):
        for option in ('', ' hyper.console=uart'):
            result, inittab, calls, output = self.run_init('console=ttyAMA0' + option)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertTrue(inittab.startswith('ttyAMA0::respawn:'))
            self.assertIn('Linux userspace is running', result.stdout)
            self.assertNotIn('modprobe virtio_console', calls)
            self.assertEqual(output, '')

    def test_virtio_shell_does_not_depend_on_the_printk_console_selection(self):
        result, inittab, calls, output = self.run_init(
            'console=ttyAMA0 earlycon hyper.console=virtio', virtio=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(inittab.startswith('hvc0::respawn:'))
        self.assertIn('modprobe virtio_console\n', calls)
        self.assertIn('Linux userspace is running', output)
        self.assertNotIn('Linux userspace is running', result.stdout)

    def test_missing_virtio_console_is_an_error_instead_of_a_uart_fallback(self):
        result, inittab, calls, _ = self.run_init('hyper.console=virtio')
        self.assertNotEqual(result.returncode, 0)
        self.assertIsNone(inittab)
        self.assertIn('requested virtio console is unavailable', result.stderr)
        self.assertEqual(calls.count('sleep 1\n'), 10)
        self.assertNotIn('\ninit\n', calls)

    def test_driver_failure_and_unknown_transport_fail_before_shell_start(self):
        for option, module_ok, message in (
                ('virtio', False, 'cannot load virtio console driver'),
                ('other', True, 'unsupported console transport')):
            result, inittab, calls, _ = self.run_init(
                'hyper.console=' + option, virtio=True, module_ok=module_ok)
            self.assertNotEqual(result.returncode, 0)
            self.assertIsNone(inittab)
            self.assertIn(message, result.stderr)
            self.assertNotIn('\ninit\n', calls)


class GuestDiskTests(unittest.TestCase):
    def test_rootfs_ownership_and_failed_rebuild(self):
        try:
            debugfs = guest_disk.packer.image_tool('debugfs', 'debugfs', 'e2fsprogs')
            guest_disk.packer.image_tool('mke2fs', 'mke2fs', 'e2fsprogs')
        except ValueError as error:
            self.skipTest(str(error))
        board = json.loads((ROOT / 'boards/qemu.json').read_text())
        next(part for part in board['disk']['partitions']
             if part.get('image') == 'alpine-rootfs')['size-mib'] = 32
        with tempfile.TemporaryDirectory() as temporary:
            temporary = Path(temporary)
            archive = temporary / 'rootfs.tar'
            with tarfile.open(archive, 'w') as target:
                info = tarfile.TarInfo('owned')
                info.size = 5
                info.uid, info.gid, info.mode = 123, 456, 0o640
                target.addfile(info, io.BytesIO(b'proof'))
            output = temporary / 'root.ext4'
            guest_disk.build(guest_disk.Board.parse(board), archive, output)
            self.assertEqual(output.stat().st_size, 32 * 1024**2)
            result = subprocess.run([debugfs, '-R', 'stat /owned', str(output)],
                                    check=True, capture_output=True, text=True)
            self.assertRegex(result.stdout, r'User:\s+123\s+Group:\s+456')
            self.assertIn('0640', result.stdout)
            restored = temporary / 'restored'
            subprocess.run([debugfs, '-R', f'dump /owned {restored}', str(output)],
                           check=True, capture_output=True)
            self.assertEqual(restored.read_bytes(), b'proof')
            before = output.stat().st_ino
            archive.write_bytes(b'not a tar')
            with self.assertRaises(tarfile.ReadError):
                guest_disk.build(guest_disk.Board.parse(board), archive, output)
            self.assertEqual(output.stat().st_ino, before)


if __name__ == '__main__':
    unittest.main()
