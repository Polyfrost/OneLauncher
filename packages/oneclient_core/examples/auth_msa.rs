
use std::process::Command;

use oneclient_core::auth::{self, MinecraftLogin};
use oneclient_core::{dev, logger, LauncherResult};

#[tokio::main]
async fn main() -> LauncherResult<()> {
    logger::init_debug()?;
    dev::initialize().await?;

    let flow = auth::begin_microsoft_login().await?;

    match &flow {
        MinecraftLogin::DeviceCode(device) => {
            println!("{}\n", device.message);
            println!("Verification URL: {}", device.verification_uri);
            println!("Enter this code: {}\n", device.user_code);
            try_open_browser(&device.verification_uri);
        }
        MinecraftLogin::Browser(browser) => {
            println!("Opening browser to sign in...\n");
            try_open_browser(&browser.auth_url);
        }
    }

    println!("Waiting for you to sign in...");
    let account = auth::finish_microsoft_login(flow).await?;

    println!(
        "Signed in as {} ({})\n  token expires: {}",
        account.username, account.id, account.expires
    );
    println!("Account saved to {}", oneclient_core::paths::auth_file()?.display());

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
