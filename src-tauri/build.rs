use tauri_build::{Attributes, InlinedPlugin};

fn main() {
    let _ = tauri_build::try_build(Attributes::new()
        .plugin(
            "auth",
            InlinedPlugin::new().commands(&["login_msa"]) 
        ));
}
