
use std::process::Command;

use oneclient_core::{dev, logger, LauncherResult};

#[tokio::main]
async fn main() -> LauncherResult<()> {
    logger::init_debug()?;
    let state = dev::initialize().await?;

    let session = state.auth.begin_microsoft_login().await?;

    println!("Sign in with EITHER method:\n");
    println!("  Browser: opening {}", session.browser.auth_url);
    try_open_browser(&session.browser.auth_url);
    println!(
        "  Device code: go to {} and enter {}\n",
        session.device.verification_uri, session.device.user_code
    );

    println!("Waiting for you to sign in...");
    let account = state.auth.finish_microsoft_login(session).await?;

    println!(
        "Signed in as {} ({})\n  token expires: {}",
        account.username, account.id, account.expires
    );
    println!("Account saved to {}", oneclient_common::paths::auth_file()?.display());

    Ok(())
}

fn try_open_browser(url: &str) {
    let result = if cfg!(target_os = "windows") {
        Command::new("cmd").args(["/C", "start", "", url]).spawn()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(url).spawn()
    } else {
        Command::new("xdg-open").arg(url).spawn()
    };

    match result {
        Ok(_) => println!("Opened verification page in your default browser.\n"),
        Err(err) => {
            println!("Could not open browser automatically ({err}). Open the URL above manually.\n");
        }
    }
}
