#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

"""Build the pinned Fuchsia F2FS core with Native platform adapters."""

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from source_archive import sources


def build(sdk, output, cache, fetch=False):
    output.mkdir(parents=True, exist_ok=True)
    spec = importlib.util.spec_from_file_location('hyper_cxx_build', ROOT / 'stdlib/cxx/build.py')
    runtime = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(runtime)
    if sdk is None:
        flags = [os.environ.get('HYPER_HOST_CXX', 'clang++'), '-std=c++20', '-g', '-O1',
                 '-fno-exceptions', '-fno-rtti', '-fPIC']
        if sys.platform == 'darwin':
            host_sdk = subprocess.check_output(['xcrun', '--show-sdk-path'], text=True).strip()
            flags += ['-isystem', str(Path(host_sdk) / 'usr/include/c++/v1')]
        cpp_archive = None
    else:
        flags, cpp_archive = runtime.build(sdk, output / 'cxx', cache, fetch)
    upstream = sources(HERE / 'sources.json', cache, fetch)
    tree = output / 'source'
    digest = hashlib.sha256((HERE / 'fuchsia.patch').read_bytes() + (HERE / 'sources.json').read_bytes())
    for path in sorted((HERE / 'cpp').rglob('*')):
        if path.is_file():
            digest.update(str(path.relative_to(HERE)).encode())
            digest.update(path.read_bytes())
    source_stamp = output / 'source.sha256'
    if not tree.exists() or not source_stamp.exists() or source_stamp.read_text() != digest.hexdigest():
        shutil.rmtree(tree, ignore_errors=True)
        for path, source in upstream.items():
            shutil.copytree(source, tree / path, dirs_exist_ok=True)
        subprocess.run(['patch', '-p1', '--batch', '-i', str(HERE / 'fuchsia.patch')], cwd=tree,
                       check=True, stdout=subprocess.DEVNULL)
        shutil.copytree(HERE / 'cpp', tree, dirs_exist_ok=True)
        source_stamp.write_text(digest.hexdigest())
    include_paths = ['', 'zircon/system/public', 'sdk/lib/zircon-assert',
                     'zircon/system/ulib/fbl/include', 'zircon/system/ulib/zx/include',
                     'zircon/system/ulib/zircon-internal/include',
                     'zircon/third_party/ulib/safemath/include', 'sdk/lib/fit/include',
                     'sdk/lib/fit-promise/include', 'sdk/lib/stdcompat/include',
                     'zircon/system/ulib/bitmap/include']
    flags += ([] if sdk is None else ['-DHYPER_F2FS_NATIVE=1']) + [arg for path in include_paths for arg in ('-I', str(tree / path))]
    excluded = {'bcache', 'component_runner', 'inspect', 'memory_watcher', 'vmo_manager', 'fsck'}
    files = [p for p in (tree / 'src/storage/f2fs').glob('*.cc') if p.stem not in excluded]
    files += list((tree / 'sdk/lib/fit-promise').glob('*.cc'))
    files += [tree / path for path in ['src/storage/f2fs/third_party/ext2_hash/hash.cc',
        'bridge.cc', 'src/storage/lib/vfs/cpp/journal/background_executor.cc',
        'src/storage/lib/vfs/cpp/transaction/buffered_operations_builder.cc',
        'zircon/system/ulib/bitmap/raw-bitmap.cc', 'zircon/system/ulib/fbl/alloc_checker.cc',
        'src/storage/lib/buffer/block_buffer.cc']]
    digest.update(json.dumps(flags).encode())
    digest.update(subprocess.check_output([flags[0], "--version"]))
    if sdk is not None:
        digest.update((output / 'cxx/build.sha256').read_bytes())
    digest.update(Path(__file__).read_bytes())
    stamp = output / 'build.sha256'
    archive = output / ('libhyper-f2fs.so' if sdk is None else 'libhyper-f2fs.a')
    if not archive.exists() or not stamp.exists() or stamp.read_text() != digest.hexdigest():
        objects = output / 'objects'
        objects.mkdir(exist_ok=True)
        def compile(source):
            obj = objects / (source.stem + '.o')
            subprocess.run(flags + ['-c', str(source), '-o', str(obj)], check=True)
            return str(obj)
        with ThreadPoolExecutor(max_workers=3) as pool:
            built = list(pool.map(compile, files))
        temporary = output / 'libhyper-f2fs.tmp'
        temporary.unlink(missing_ok=True)
        command = ([flags[0], '-dynamiclib' if sys.platform == 'darwin' else '-shared', '-o', str(temporary)] if sdk is None else [os.environ.get('LLVM_AR', str(Path(shutil.which(flags[0]) or flags[0]).with_name('llvm-ar'))), 'rcs', str(temporary)])
        subprocess.run(command + built, check=True)
        temporary.replace(archive)
        stamp.write_text(digest.hexdigest())
    return archive, cpp_archive


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--sdk', type=Path)
    mode.add_argument('--host', action='store_true')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--cache', type=Path, default=ROOT / 'target/f2fs-sources')
    parser.add_argument('--fetch', action='store_true')
    args = parser.parse_args()
    build(args.sdk, args.output.resolve(), args.cache.resolve(), args.fetch)
