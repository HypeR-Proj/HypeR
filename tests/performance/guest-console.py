#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

"""Manually compare guest terminal transports on one local QEMU TCG instance."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import statistics
import subprocess
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'qemu'))
from session import Session, native_command
from guest_console import append_console_output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--qemu', default='qemu-system-aarch64')
    parser.add_argument('--image', type=Path, required=True)
    parser.add_argument('--initramfs', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--repeat', type=int, default=3)
    parser.add_argument('--kib', type=int, default=32)
    parser.add_argument('--device', action='append', choices=('ttyAMA0', 'ttyS0', 'hvc0'))
    args = parser.parse_args()
    if any(os.environ.get(key, '').lower() == 'true' for key in ('CI', 'GITHUB_ACTIONS')):
        parser.error('performance measurement is local/manual only')
    if args.repeat < 1 or not 1 <= args.kib <= 256:
        parser.error('repeat must be positive and kib must be in 1..256')
    if args.output.exists():
        parser.error('output exists; choose a new directory to preserve previous evidence')
    args.output.mkdir(parents=True)
    uart = 'ttyS0' if 'riscv' in Path(args.qemu).name else 'ttyAMA0'
    command = native_command(args.qemu, args.image.resolve(), args.initramfs.resolve())
    command += ['-accel', 'tcg,thread=multi']
    report = {
        'host': platform.platform(),
        'qemu': subprocess.check_output([args.qemu, '--version'], text=True),
        'command': command,
        'artifacts': {key: {'path': str(path.resolve()),
                            'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}
                      for key, path in (('image', args.image), ('initramfs', args.initramfs))},
        'payload_bytes': args.kib * 1024 + 1,
        'metric': 'command submission through receipt of complete payload and END, seconds',
        'limits': ['Local TCG comparison, not a physical UART throughput prediction.',
                   'Includes guest command execution, host scheduling and console relay.'],
        'samples': [],
    }
    destination = args.output / 'summary.json'

    def save():
        destination.write_text(json.dumps(report, indent=2) + '\n')

    save()
    with Session(command, args.output / 'serial.log', output_filter=append_console_output) as session:
        boot = session.await_text(rb'HypeR vm-manager: ready', timeout=120)
        if b'vCPU start submitted' not in boot:
            session.await_text(rb'vCPU start submitted[^\n]*\n', timeout=120)
        session.pump(1)
        session.send(b'\necho CONSOLE_HOST_READY\n')
        session.await_text(rb'\nCONSOLE_HOST_READY\n.*hyper-sh\$ ', timeout=30)
        session.send(b'vmm console alpine\n')
        session.await_text(rb'Connected to alpine\.', timeout=30)
        session.await_text(rb'~ # ', timeout=180)
        # Kernel diagnostics and the terminal intentionally use different
        # devices. Drain the UART before timing either one, and suppress new
        # informational printk records during this manual measurement.
        session.send((f"PS1=''; dmesg -n 1; printf '\\nUART_DRAINED\\n' > /dev/{uart}; "
                      "read -r bench_ack\n").encode())
        session.await_text(rb'\nUART_DRAINED\n', timeout=300)
        session.send(b'\n')
        session.pump(1)
        session.send(b"PS1=''; printf '\\nBENCH_READY\\n'\n")
        session.await_text(rb'\nBENCH_READY\n', timeout=30)
        session.send((f"dd if=/dev/zero bs=1024 count={args.kib} 2>/dev/null | "
                      "tr '\\000' x > /tmp/console-bench; echo >> /tmp/console-bench; "
                      "printf '\\nPAYLOAD_READY\\n'\n").encode())
        session.await_text(rb'\nPAYLOAD_READY\n', timeout=30)
        devices = args.device or [uart, 'hvc0']
        for iteration in range(args.repeat):
            # Alternate order to reduce monotonic warm-up/thermal bias.
            for device in devices if iteration % 2 == 0 else reversed(devices):
                token = f'{device}_{iteration}'
                started = time.monotonic()
                # Both markers must travel with the payload: delivery order
                # between UART and virtio is deliberately unspecified.
                session.send((f"{{ printf '\\nBEGIN_{token}\\n'; "
                              "cat /tmp/console-bench; "
                              f"printf '\\nEND_{token}\\n'; }} > /dev/{device}; "
                              "read -r bench_ack\n").encode())
                session.await_text(rb'\nBEGIN_' + token.encode() + rb'\n', timeout=30)
                output = session.await_text(rb'\nEND_' + token.encode() + rb'\n', timeout=300)
                elapsed = time.monotonic() - started
                expected = b'x' * (args.kib * 1024) + b'\n\nEND_' + token.encode() + b'\n'
                if output != expected:
                    raise AssertionError(f'{device}: payload mismatch, got {len(output)} bytes')
                # A prompt on hvc0 can precede the final UART output. Use a
                # fresh round trip rather than waiting for that old prompt.
                session.send(f"\nprintf '\\nREADY_{token}\\n'\n".encode())
                session.await_text(rb'\nREADY_' + token.encode() + rb'\n', timeout=30)
                sample = {'device': device, 'iteration': iteration, 'seconds': elapsed,
                          'bytes_per_second': report['payload_bytes'] / elapsed}
                report['samples'].append(sample)
                save()
                print(f'{device}: {elapsed:.3f}s, {sample["bytes_per_second"] / 1024:.1f} KiB/s', flush=True)
        report['median_seconds'] = {
            device: statistics.median(row['seconds'] for row in report['samples']
                                      if row['device'] == device) for device in devices}
        save()
    print(destination)


if __name__ == '__main__':
    main()
