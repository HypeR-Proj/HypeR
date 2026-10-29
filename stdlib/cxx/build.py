#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

"""Build the experimental Native C++ runtime against an installed Native SDK."""

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
from source_archive import sources

HERE = Path(__file__).resolve().parent
SOURCES = '''algorithm atomic call_once condition_variable condition_variable_destructor
exception functional future memory mutex mutex_destructor new new_handler new_helpers
optional shared_mutex stdexcept string thread typeinfo vector'''.split()


def build(sdk, output, cache, fetch=False, jobs=3):
    sdk = sdk.resolve()
    identity = dict(line.split('=', 1) for line in
                    (sdk / 'share/hyper/manifest').read_text().splitlines() if '=' in line)
    arch = identity['architecture']
    if arch not in ('aarch64', 'riscv64'):
        raise ValueError('C++ runtime requires an AArch64 or RISC-V Native SDK')
    upstream = sources(HERE / 'sources.json', cache, fetch)
    output.mkdir(parents=True, exist_ok=True)
    headers = output / 'include'
    headers.mkdir(exist_ok=True)
    for source in (HERE / 'include').iterdir():
        shutil.copyfile(source, headers / source.name)
    musl = upstream['musl']
    bits = headers / 'bits'
    bits.mkdir(exist_ok=True)
    # Only C declarations are used. No musl implementation or Linux syscall ABI is linked.
    with (bits / 'alltypes.h').open('w') as generated:
        subprocess.run(['sed', '-f', str(musl / 'tools/mkalltypes.sed'),
                        str(musl / f'arch/{arch}/bits/alltypes.h.in'),
                        str(musl / 'include/alltypes.h.in')], stdout=generated, check=True)
    cpp = upstream['libcxx']
    includes = [cpp / 'include', headers, musl / 'include', musl / f'arch/{arch}',
                musl / 'arch/generic', sdk / 'include']
    clang = os.environ.get('HYPER_CXX', os.environ.get('HYPER_CLANG', 'clang++'))
    flags = [clang, f'--target={arch}-none-elf', '-std=c++20', '-D_GNU_SOURCE=1',
             '-g', '-O1', '-fno-exceptions', '-fno-rtti', '-fno-c++-static-destructors',
             '-ffunction-sections', '-fdata-sections', '-fPIC', '-nostdinc++', '-ffreestanding']
    if arch == 'riscv64':
        flags += ['-march=rv64gc', '-mabi=lp64d', '-mno-relax']
    flags += [arg for path in includes for arg in ('-I', str(path))]
    files = [cpp / 'src' / (name + '.cpp') for name in SOURCES]
    files += [HERE / 'runtime.cc', HERE / 'os.cc']
    fingerprint = hashlib.sha256(json.dumps(flags).encode())
    fingerprint.update(subprocess.check_output([flags[0], "--version"]))
    for source in files + sorted((HERE / 'include').iterdir()) + [HERE / 'build.py', HERE / 'sources.json']:
        fingerprint.update(source.read_bytes())
    fingerprint.update((sdk / 'share/hyper/manifest').read_bytes())
    stamp = output / 'build.sha256'
    archive = output / 'libhyper-cxx.a'
    if not archive.exists() or not stamp.exists() or stamp.read_text() != fingerprint.hexdigest():
        def compile(source):
            obj = output / (source.stem + '.o')
            extra = ['-D_LIBCPP_BUILDING_LIBRARY'] if source.suffix == '.cpp' else []
            subprocess.run(flags + extra + ['-c', str(source), '-o', str(obj)], check=True)
            return str(obj)
        with ThreadPoolExecutor(max_workers=jobs) as pool:
            objects = list(pool.map(compile, files))
        temporary = output / 'libhyper-cxx.tmp.a'
        temporary.unlink(missing_ok=True)
        subprocess.run([os.environ.get('LLVM_AR', str(Path(shutil.which(flags[0]) or flags[0]).with_name('llvm-ar'))), 'rcs', str(temporary)] + objects, check=True)
        temporary.replace(archive)
        stamp.write_text(fingerprint.hexdigest())
    (output / 'compile.json').write_text(json.dumps({'flags': flags, 'archive': str(archive)}, indent=2)+'\n')
    return flags, archive


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sdk', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--cache', type=Path, default=ROOT / 'target/f2fs-sources')
    parser.add_argument('--fetch', action='store_true')
    args = parser.parse_args()
    build(args.sdk, args.output.resolve(), args.cache.resolve(), args.fetch)
