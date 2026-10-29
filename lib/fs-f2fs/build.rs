// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0
use std::{env, path::PathBuf, process::Command};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=cpp");
    println!("cargo:rerun-if-changed=fuchsia.patch");
    println!("cargo:rerun-if-changed=sources.json");
    println!("cargo:rerun-if-changed=build.py");
    println!("cargo:rerun-if-changed=../../stdlib/cxx");
    println!("cargo:rerun-if-changed=../../scripts/source_archive.py");
    for name in [
        "HYPER_SYSROOT",
        "HYPER_CXX",
        "HYPER_CLANG",
        "LLVM_AR",
        "HYPER_DOCUMENTATION",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    // Rustdoc describes the owned Rust boundary without linking an executable.
    // The repository docs driver sets this explicitly; normal builds still
    // require the verified media sources and a complete Native link.
    if env::var("HYPER_DOCUMENTATION").as_deref() == Ok("1")
        || env::var("CARGO_CFG_TARGET_OS")? != "hyper"
    {
        return Ok(());
    }
    let sdk = env::var("HYPER_SYSROOT")
        .map_err(|_| "F2FS requires HYPER_SYSROOT and the installed hyper-cargo driver")?;
    println!("cargo:rerun-if-changed={sdk}/share/hyper/manifest");
    let output = PathBuf::from(env::var("OUT_DIR")?).join("f2fs");
    let result = Command::new("python3")
        .args(["build.py", "--sdk", &sdk, "--output"])
        .arg(&output)
        .output()?;
    if !result.status.success() {
        return Err(format!(
            "F2FS C++ build failed:\n{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        )
        .into());
    }
    println!("cargo:rustc-link-search=native={}", output.display());
    println!(
        "cargo:rustc-link-search=native={}",
        output.join("cxx").display()
    );
    println!("cargo:rustc-link-lib=static=hyper-f2fs");
    println!("cargo:rustc-link-lib=static=hyper-cxx");
    Ok(())
}
