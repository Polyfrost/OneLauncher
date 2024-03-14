use core::hash;
use std::{error::Error, sync::mpsc};

use serde_json::json;
use tauri::{http::request, AppHandle, Runtime, Url};
use tauri_plugin_http::reqwest::{Client, ClientBuilder, Request};

use super::{Account, AuthenticationMethod};
    
#[tauri::command]
pub async fn login_msa<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<Account, String> {
    match MicrosoftAuthenticationMethod::auth(&app).await {
        Ok(account) => Ok(account),
        Err(err) => Err(err.to_string())
    }
}

// Uses the official Minecraft launcher Azure client id
pub const OAUTH_URL: &str = "https://login.live.com/oauth20_authorize.srf?client_id=00000000402b5328&response_type=code&redirect_uri=https://login.live.com/oauth20_desktop.srf&scope=XboxLive.signin%20offline_access";
pub struct MicrosoftAuthenticationMethod;

impl AuthenticationMethod for MicrosoftAuthenticationMethod {
    async fn auth<R: Runtime>(handle: &AppHandle<R>) -> Result<Account, Box<dyn Error>> {
        let code: String = controlled_webview(handle).await?;
        if code.is_empty() {
            return Err("No code received".into());
        }

        let (xbl_token, hash) = auth_xbl(code).await?;

        Err("".into())
    }
}

async fn auth_xbl(code: String) -> Result<(String, String), Box<dyn Error>> {

    let json = &json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": format!("d={}", code)
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT"
    });

    println!("{:?}", json.as_str());

    // figure out why it returns 400: Bad Request
    let response = Client::new().post("https://user.auth.xboxlive.com/user/authenticate")
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .json(json).send().await?;

    println!("{:#?}", response);

    Ok(("".into(), "".into()))
}

async fn controlled_webview<R: tauri::Runtime>(handle: &AppHandle<R>) -> Result<String, Box<dyn Error>> {
    let (sender, receiver) = mpsc::channel();

    let window = tauri::WebviewWindowBuilder::new(handle, "microsoft_login", tauri::WebviewUrl::External(Url::parse(OAUTH_URL)?))
        .center()
        .enable_clipboard_access()
        .focused(true)
        .title("Login to Microsoft")
        .resizable(false)
        .inner_size(450.0, 650.0)
        .on_navigation(move |url: &Url| {
            if url.to_string().starts_with("https://login.live.com/oauth20_desktop.srf") {
                let pair = url.query_pairs().find(|search| search.0 == "code");

                let _ = match pair {
                    Some(pair) => sender.send(pair.1.to_string()),
                    None => sender.send("".to_string())
                };
            }

            true
        })
        .build()?;

    let result = receiver.recv();
    window.close()?;
    Ok(result?)
}
