//! Embed the Petsona icon into `petsona-windows.exe`.
//!
//! `rc.exe` (MSVC target) or `windres.exe` (GNU target) is invoked directly, so
//! the workspace keeps its "no new third-party crates" rule. When neither tool
//! is reachable the resource is skipped with a warning: the binary still
//! builds, it just keeps the default executable icon.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let resource = manifest_dir.join("../../packaging/windows/Petsona.rc");
    let icon = manifest_dir.join("../../packaging/windows/Petsona.ico");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", resource.display());
    println!("cargo:rerun-if-changed={}", icon.display());

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let object = match env::var("CARGO_CFG_TARGET_ENV")
        .unwrap_or_default()
        .as_str()
    {
        "msvc" => compile_msvc(&resource, &out_dir),
        "gnu" => compile_gnu(&resource, &out_dir),
        _ => None,
    };

    match object {
        Some(object) => println!("cargo:rustc-link-arg-bins={}", object.display()),
        None => println!(
            "cargo:warning=could not embed the Petsona icon (no rc.exe/windres.exe found); \
             the executable keeps the default icon"
        ),
    }
}

fn compile_msvc(resource: &Path, out_dir: &Path) -> Option<PathBuf> {
    let rc = env::var_os("RC")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(|| find_in_path("rc.exe"))
        .or_else(find_windows_kit_rc)?;
    let object = out_dir.join("Petsona.res");
    let status = Command::new(rc)
        .arg("/nologo")
        .arg("/fo")
        .arg(&object)
        .arg(resource)
        .current_dir(resource.parent()?)
        .status()
        .ok()?;
    status.success().then_some(object)
}

fn compile_gnu(resource: &Path, out_dir: &Path) -> Option<PathBuf> {
    let windres = env::var_os("WINDIRES")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(|| {
            // Cross-compiling from Linux uses the target-prefixed tool.
            let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
            find_in_path(&format!("{arch}-w64-mingw32-windres"))
        })
        .or_else(|| find_in_path("windres.exe"))
        .or_else(|| find_in_path("windres"))?;
    let object = out_dir.join("Petsona.res.o");
    let status = Command::new(windres)
        .arg("--input")
        .arg(resource)
        .arg("--output")
        .arg(&object)
        .arg("--include-dir")
        .arg(resource.parent()?)
        .arg("-O")
        .arg("coff")
        .status()
        .ok()?;
    status.success().then_some(object)
}

fn find_in_path(program: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|directory| directory.join(program))
        .find(|candidate| candidate.is_file())
}

/// `rc.exe` ships with the Windows SDK, which is not always on `PATH`.
fn find_windows_kit_rc() -> Option<PathBuf> {
    let kits = PathBuf::from(env::var_os("ProgramFiles(x86)")?)
        .join("Windows Kits")
        .join("10")
        .join("bin");
    let mut versions: Vec<PathBuf> = std::fs::read_dir(kits)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    versions.sort();
    versions.iter().rev().find_map(|version| {
        ["x64", "x86", "arm64"]
            .iter()
            .map(|arch| version.join(arch).join("rc.exe"))
            .find(|candidate| candidate.is_file())
    })
}
