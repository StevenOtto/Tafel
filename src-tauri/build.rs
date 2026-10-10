use std::path::{Path, PathBuf};
use std::process::Command;
use swift_rs::SwiftLinker;

/// swift-rs runtime helpers the `swift-rs` crate links against.
const SWIFT_RS_HELPERS: [&str; 4] = [
    "_data_from_bytes",
    "_retain_object",
    "_release_object",
    "_string_from_bytes",
];

fn main() {
    // Must match `minimumSystemVersion` in tauri.conf.json and the platform
    // in swift/TafelVision/Package.swift.
    SwiftLinker::new("12.0")
        .with_package("TafelVision", "./swift/TafelVision")
        .link();
    globalize_swift_rs_helpers();
    println!("cargo:rustc-link-lib=framework=Vision");
    println!("cargo:rerun-if-changed=swift/TafelVision/Sources");
    println!("cargo:rerun-if-changed=swift/TafelVision/Package.swift");
    tauri_build::build()
}

/// Xcode 27 release builds make swift-rs's own @_cdecl helpers local in the
/// static library, so the release binary fails to link. swift-rs 1.0.8 only
/// re-exports the package's own symbols; promote its helpers here too.
/// Remove once swift-rs handles this itself.
fn globalize_swift_rs_helpers() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is set"));
    let archives = find_archives(&out_dir.join("swift-rs/TafelVision"), "libTafelVision.a");
    for archive in archives {
        let Ok(nm) = Command::new("nm").arg(&archive).output() else {
            continue;
        };
        let listing = String::from_utf8_lossy(&nm.stdout);
        let local: Vec<&str> = SWIFT_RS_HELPERS
            .into_iter()
            .filter(|name| listing.lines().any(|l| l.ends_with(&format!(" t {name}"))))
            .collect();
        if local.is_empty() {
            continue;
        }
        let objcopy = llvm_objcopy().expect(
            "Xcode 27 release builds need llvm-objcopy: run `rustup component add llvm-tools`",
        );
        let mut command = Command::new(objcopy);
        command.args(
            local
                .iter()
                .map(|name| format!("--globalize-symbol={name}")),
        );
        let status = command.arg(&archive).status().expect("llvm-objcopy runs");
        assert!(
            status.success(),
            "llvm-objcopy failed on {}",
            archive.display()
        );
    }
}

fn find_archives(dir: &Path, file_name: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .flat_map(|path| {
            if path.is_dir() {
                find_archives(&path, file_name)
            } else if path.file_name().is_some_and(|name| name == file_name) {
                vec![path]
            } else {
                Vec::new()
            }
        })
        .collect()
}

fn llvm_objcopy() -> Option<PathBuf> {
    let sysroot = Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .ok()?;
    let sysroot = String::from_utf8_lossy(&sysroot.stdout).trim().to_string();
    let host = format!("{}-apple-darwin", std::env::consts::ARCH);
    let path = Path::new(&sysroot)
        .join("lib/rustlib")
        .join(host)
        .join("bin/llvm-objcopy");
    path.exists().then_some(path)
}
