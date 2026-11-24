//! Иконка приложения в exe: ресурс собирается из `assets/icon.ico`
//! через `windres` (если он есть в системе) и отдаётся линкеру.
//! Без windres сборка просто идёт без иконки — ничего не ломается.

use std::path::{Path, PathBuf};
use std::process::Command;

fn find_windres() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            let p = dir.join("windres.exe");
            if p.is_file() {
                return Some(p);
            }
        }
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(home) = std::env::var("USERPROFILE") {
        candidates.push(
            PathBuf::from(&home)
                .join("mingw64")
                .join("mingw64")
                .join("bin")
                .join("windres.exe"),
        );
        candidates.push(PathBuf::from(&home).join("mingw64").join("bin").join("windres.exe"));
    }
    candidates.push(PathBuf::from(r"C:\mingw64\bin\windres.exe"));
    candidates.push(PathBuf::from(r"C:\msys64\mingw64\bin\windres.exe"));
    candidates.into_iter().find(|p| p.is_file())
}

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    let icon = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("icon.ico");
    if !icon.is_file() {
        return;
    }
    let Some(windres) = find_windres() else {
        println!("cargo:warning=windres не найден: exe собирается без иконки");
        return;
    };
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let rc = out.join("shatmat_icon.rc");
    let obj = out.join("shatmat_icon.o");
    // В .rc путь к иконке пишем с прямыми слэшами: так понимает windres.
    let icon_path = icon.display().to_string().replace('\\', "/");
    if std::fs::write(&rc, format!("1 ICON \"{}\"\n", icon_path)).is_err() {
        return;
    }
    let status = Command::new(&windres)
        .arg(&rc)
        .arg("-O")
        .arg("coff")
        .arg("-o")
        .arg(&obj)
        .status();
    match status {
        Ok(s) if s.success() && obj.is_file() => {
            println!("cargo:rustc-link-arg={}", obj.display());
        }
        Ok(s) => println!("cargo:warning=windres завершился с {s}: иконка не встроена"),
        Err(e) => println!("cargo:warning=windres не запустился ({e}): иконка не встроена"),
    }
}
