use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=src/game/game_mode.m");
    println!("cargo:rerun-if-env-changed=MACOSX_DEPLOYMENT_TARGET");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

    let minimum = env::var("MACOSX_DEPLOYMENT_TARGET").unwrap_or_else(|_| "11.0".into());
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("gamemode");
    let status = Command::new("xcrun")
        .args([
            "--sdk", "macosx",
            "clang",
            "-arch", "arm64",
            "-arch", "x86_64",
        ])
        .arg(format!("-mmacosx-version-min={minimum}"))
        .args([
            "-Os",
            "-fobjc-arc",
            "-Wl,-export_dynamic",
            "-framework", "Foundation",
            "-framework", "CoreServices",
        ])
        .arg("src/game/game_mode.m")
        .arg("-o").arg(&output)
        .status();
    assert!(
        status.is_ok() && status.unwrap().success(),
        "failed to compile the macOS Game Mode helper"
    );

    let status = Command::new("codesign")
        .args(["-f", "-s", "-"])
        .arg(&output)
        .status();
    assert!(
        status.is_ok() && status.unwrap().success(),
        "failed to ad-hoc sign the macOS Game Mode helper"
    );
}
