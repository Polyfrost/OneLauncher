use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=src/game/game_mode_stub.m");
    println!("cargo:rerun-if-env-changed=MACOSX_DEPLOYMENT_TARGET");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

    let arch = match env::var("CARGO_CFG_TARGET_ARCH").unwrap().as_str() {
        "aarch64" => "arm64",
        "x86_64" => "x86_64",
        arch => panic!("unsupported macOS architecture: {arch}"),
    };
    let minimum = env::var("MACOSX_DEPLOYMENT_TARGET")
        .unwrap_or_else(|_| if arch == "arm64" { "11.0" } else { "10.14" }.into());
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("game-mode-stub");
    let status = Command::new("xcrun")
        .args(["--sdk", "macosx", "clang", "-arch", arch])
        .arg(format!("-mmacosx-version-min={minimum}"))
        .args([
            "-Os",
            "-fobjc-arc",
            "-Wl,-export_dynamic",
            "-framework",
            "Foundation",
            "-framework",
            "CoreServices",
        ])
        .arg("src/game/game_mode_stub.m")
        .arg("-o")
        .arg(output)
        .status()
        .expect("failed to compile the macOS Game Mode helper");
    assert!(
        status.success(),
        "failed to compile the macOS Game Mode helper"
    );
}
