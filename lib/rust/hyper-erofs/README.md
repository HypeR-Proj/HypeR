<!-- SPDX-FileCopyrightText: 2026 roolrz -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# EROFS media reader

`hyper-erofs` reads immutable EROFS images through a bounded random-read device.
The Native `fs-erofs` process adapts that interface to its exclusive block-volume
capability. Generic filesystem RPC, mount publication, and namespace symlink
resolution remain outside the format reader.

The supported format is uncompressed EROFS with 512, 1024, 2048, or 4096-byte
blocks, compact and extended inodes, flat contiguous data and inline tails,
regular files, directories, and symbolic links. A present superblock CRC32C is
validated. Unsupported incompatible features and inode layouts, including
compression, chunked files, external devices, and extended attributes, return
an explicit error. Device nodes, sockets, and FIFOs are also unsupported.

The parser uses safe Rust and checks disk offsets against the actual device
length. It caches one 4 KiB metadata block and one resolved path, without loading
whole files or images. Directory scans are limited to 16 MiB per directory;
paths and symlink targets to 4096 bytes; filenames to 255 bytes. HypeR's namespace
requires UTF-8 names. Symlink target bytes are returned without interpretation.
The device must remain immutable for the lifetime of a mounted reader.

The implementation follows the published [EROFS core disk format](https://erofs.docs.kernel.org/en/latest/ondisk/core_ondisk.html).
It does not incorporate Linux filesystem implementation code.

Run parser tests with `cargo test --manifest-path lib/rust/Cargo.toml -p hyper-erofs`.
The host `inspect` example lists a directory or emits file/link bytes from an image:

```sh
cargo run --manifest-path lib/rust/Cargo.toml -p hyper-erofs --example inspect -- IMAGE [PATH]
```
