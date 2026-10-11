// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Host-side admission check using init's production parser and policy.
use hyper_service_manager::{bootstrap_policy::BootstrapPolicy, manifest, supervision};
use std::{env, fs, process::ExitCode};

fn check() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let path = args
        .next()
        .ok_or("usage: check-services MANIFEST [ARCHIVE_PATH ...]")?;
    let mut core_path = None;
    let mut images = Vec::new();
    while let Some(argument) = args.next() {
        if argument == "--core" {
            core_path = Some(args.next().ok_or("--core requires a manifest path")?);
        } else {
            images.push(argument);
        }
    }
    let text = fs::read_to_string(&path).map_err(|error| format!("{path}: {error}"))?;
    let system = manifest::parse(&text).map_err(|error| format!("{path}: {error:?}"))?;
    let core_text = core_path
        .as_ref()
        .map(fs::read_to_string)
        .transpose()
        .map_err(|error| error.to_string())?;
    let manifest = match core_text.as_ref() {
        Some(core) => manifest::parse(core)
            .map_err(|error| format!("core: {error:?}"))?
            .append_system(system)
            .ok_or("invalid boot/system service separation")?,
        None => system,
    };
    manifest::validate(&manifest, &BootstrapPolicy)
        .map_err(|error| format!("{path}: {error:?}"))?;
    supervision::validate(&manifest).map_err(|error| format!("{path}: {error:?}"))?;
    if !images.is_empty() {
        for service in manifest.services() {
            if !images.iter().any(|image| {
                image.trim_start_matches('/') == service.image().trim_start_matches('/')
            }) {
                return Err(format!(
                    "{path}: service {:?}: image {:?} is absent from the archive",
                    service.name(),
                    service.image()
                ));
            }
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match check() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("service manifest: {error}");
            ExitCode::FAILURE
        }
    }
}
