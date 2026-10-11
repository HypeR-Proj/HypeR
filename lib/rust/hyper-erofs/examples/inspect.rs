// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Read a mkfs.erofs image through ordinary host file I/O for interoperability checks.

use hyper_erofs::{Device, Error, Volume};
use std::io::{Read, Seek, SeekFrom, Write};

struct Image(std::fs::File, u64);
impl Device for Image {
    fn len(&self) -> u64 {
        self.1
    }
    fn read_exact_at(&mut self, offset: u64, output: &mut [u8]) -> Result<(), Error> {
        self.0
            .seek(SeekFrom::Start(offset))
            .map_err(|_| Error::Io)?;
        self.0.read_exact(output).map_err(|_| Error::Io)
    }
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let image = args.next().ok_or("usage: inspect IMAGE [PATH]")?;
    let file = std::fs::File::open(image).map_err(|error| error.to_string())?;
    let size = file.metadata().map_err(|error| error.to_string())?.len();
    let mut volume =
        Volume::mount(Image(file, size)).map_err(|error| format!("mount: {error:?}"))?;
    let path = args.next().unwrap_or_default();
    let entry = volume
        .stat(&path)
        .map_err(|error| format!("stat: {error:?}"))?;
    match entry.kind {
        hyper_erofs::Kind::Directory => {
            let mut index = 0;
            while let Some(entry) = volume
                .entry(&path, index)
                .map_err(|error| format!("entry: {error:?}"))?
            {
                println!(
                    "{:?}\t{:o}\t{}\t{}",
                    entry.kind, entry.mode, entry.size, entry.name
                );
                index += 1;
            }
        }
        kind => {
            let mut bytes = [0; 32768];
            let mut offset = 0;
            loop {
                let count = if kind == hyper_erofs::Kind::Symlink {
                    volume.readlink(&path, &mut bytes)
                } else {
                    volume.read(&path, offset, &mut bytes)
                }
                .map_err(|error| format!("read: {error:?}"))?;
                std::io::stdout()
                    .write_all(&bytes[..count])
                    .map_err(|error| error.to_string())?;
                if count == 0 || kind == hyper_erofs::Kind::Symlink {
                    break;
                }
                offset += count as u64;
            }
        }
    }
    Ok(())
}
