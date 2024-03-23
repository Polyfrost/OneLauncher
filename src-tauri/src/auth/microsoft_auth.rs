use std::{io::{Read, Write}, net::{SocketAddr, TcpListener, TcpStream}};

use serde_json::json;
use tauri::{AppHandle, Runtime};
use tauri_plugin_shell::ShellExt;
use thiserror::Error;

use crate::{constants::CLIENT_ID, utils::http::create_client, PolyError, PolyResult};

use super::{Account, AuthenticationError, AuthenticationMethod};

// TODO: store auth keys with https://beta.tauri.app/features/stronghold/
// TODO: store auth key public in prisma for easy access

#[derive(Debug, Error)]
pub enum MicrosoftAuthError {
    #[error("failed to get Minecraft access token")]
    MSAccessToken,
    #[error("failed to get XSTS token")]
    XSTSToken,
    #[error("failed to get XBL token")]
    XBLToken,
    #[error("failed to get XBL user hash")]
    XBLUserHash,
    #[error("this account doesn't have an xbox account, please sign in through minecraft.net and try again")]
    NoXBoxAccount,
    #[error("this account is from a country where xbox live is not available")]
    XBoxLiveBlocked,
    #[error("this account needs adult verification on the xbox page")]
    AdultVerification,
    #[error("this account is under 18, please add this account to a family by an adult and try again")]
    ChildAccount,
    #[error("unknown xbox live error: {0}")]
    UnknownError(u64)
}


pub(super) struct MicrosoftAuthenticationMethod;
impl AuthenticationMethod for MicrosoftAuthenticationMethod {
    async fn auth<R: Runtime, F>(handle: &AppHandle<R>, stage: F) -> PolyResult<Account>
            where F: Fn(String, u8, bool) -> () {
        
        stage("Authenticating with Microsoft".into(), 0, false);
        let msa_code: String = msa_code(handle).await?;

        stage("Authenticating with Microsoft".into(), 1, false);
        let msa_token: String = msa_code_to_token(msa_code).await?;

        stage("Authenticating with Xbox Live".into(), 2, false);
        let (xbl_token, user_hash): (String, String) = auth_xbl(msa_token).await?;

        stage("Retrieving XSTS token".into(), 3, false);
        let xsts_token: String = auth_xsts(xbl_token).await?;

        stage("Authenticating with Minecraft".into(), 4, false);
        let access_token: String = auth_minecraft(xsts_token, user_hash).await?;

        stage("Retrieving Minecraft profile".into(), 5, true);
        let account: Account = Self::get_profile(access_token).await?;

        Ok(account)
    }
}

async fn auth_minecraft(xsts_token: String, user_hash: String) -> PolyResult<String> {
    let response = create_client()?
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&json!({
            "identityToken": format!("XBL3.0 x={};{}", user_hash, xsts_token)
        }))
        .send()
        .await?;

    let response = response.json::<serde_json::Value>().await?;
    let access_token = match response.get("access_token") {
        Some(token) => token.as_str().unwrap(),
        None => return Err(PolyError::AuthError(AuthenticationError::MicrosoftError(MicrosoftAuthError::MSAccessToken)))
    };

    Ok(access_token.to_string())
}

async fn auth_xsts(token: String) -> PolyResult<String> {
    let response = create_client()?
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [
                    token
                ]
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }))
        .send()
        .await?;

    if response.status().as_u16() == 401 {
        let error_code = response.json::<serde_json::Value>().await?["XErr"].as_u64().unwrap();

        match error_code {
            2148916233 => return Err(PolyError::AuthError(AuthenticationError::MicrosoftError(MicrosoftAuthError::NoXBoxAccount))),
            2148916235 => return Err(PolyError::AuthError(AuthenticationError::MicrosoftError(MicrosoftAuthError::XBoxLiveBlocked))),
            2148916236 | 2148916237 => return Err(PolyError::AuthError(AuthenticationError::MicrosoftError(MicrosoftAuthError::AdultVerification))),
            2148916238 => return Err(PolyError::AuthError(AuthenticationError::MicrosoftError(MicrosoftAuthError::ChildAccount))),
            _ => return Err(PolyError::AuthError(AuthenticationError::MicrosoftError(MicrosoftAuthError::UnknownError(error_code)))),
        }
    }

    let response = response.json::<serde_json::Value>().await?;
    let xsts_token = match response.get("Token") {
        Some(token) => token.as_str().unwrap(),
        None => return Err(PolyError::AuthError(AuthenticationError::MicrosoftError(MicrosoftAuthError::XSTSToken)))
    };

    Ok(xsts_token.to_string())
}

async fn auth_xbl(code: String) -> PolyResult<(String, String)> {
    let response = create_client()?
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": &format!("d={}", code)
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }))
        .send()
        .await?;

    let response = response.json::<serde_json::Value>().await?;
    let xbl_token = match response.get("Token") {
        Some(token) => token.as_str().unwrap(),
        None => return Err(PolyError::AuthError(AuthenticationError::MicrosoftError(MicrosoftAuthError::XBLToken)))
    };

    let user_hash = match response.get("DisplayClaims").and_then(|claims| claims.get("xui").and_then(|xui| xui.get(0).and_then(|xui| xui.get("uhs")))) {
        Some(hash) => hash.as_str().unwrap(),
        None => return Err(PolyError::AuthError(AuthenticationError::MicrosoftError(MicrosoftAuthError::XBLUserHash)))
    };

    Ok((xbl_token.to_string(), user_hash.to_string()))
}

async fn msa_code_to_token(code: String) -> PolyResult<String> {
    let token = create_client()?
        .post("https://login.live.com/oauth20_token.srf")
        .form(&[
            ("client_id", CLIENT_ID),
            ("redirect_uri", &format!("http://localhost:{}/", PORT)),
            ("code", &code),
            ("grant_type", "authorization_code")
        ])
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    Ok(token["access_token"].as_str().unwrap().to_string())
}

const PORT: u16 = 13523;
async fn msa_code<R: Runtime>(handle: &AppHandle<R>) -> PolyResult<String> {
    let url = format!(
        "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize?client_id={}&response_type=code&redirect_uri=http://localhost:{}/&response_mode=query&scope=XboxLive.signin%20offline_access&prompt=consent",
        CLIENT_ID, 
        PORT
    );

    handle.shell().open(url, None)?;

    let mut token: String = String::new();

    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], PORT)))?;
    println!("Started local server for MSA on port {}", PORT);

    for conn in listener.incoming() {
        match conn {
            Err(err) => return Err(err.into()),
            Ok(stream) => {
                if let Some(code) = handle_connection(stream) {
                    token = code;
                    break;
                }
            }
        }
    }

    Ok(token)
}

fn handle_connection(mut stream: TcpStream) -> Option<String> {
    let mut buffer = [0; 512];
    if let Err(err) = stream.read(&mut buffer) {
        eprintln!("Failed to read from stream: {}", err);
        return None;
    }

    let request = String::from_utf8_lossy(&buffer[..]);
    let url = request.split_whitespace().nth(1)?;
    let code = url.split('?').nth(1)?.split('&').find(|s| s.starts_with("code="))?.split_once('=').unwrap().1;
    
    let response = r"
<!DOCTYPE html>
<html>
<head>
    <title>Microsoft Authentication</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, 'Open Sans', 'Helvetica Neue', sans-serif;
        }
        
        div {
            position: absolute;
            top: 45%;
            left: 50%;
            transform: translate(-50%, -50%);
            text-align: center;
            opacity: 0;
            animation: fade-in 0.5s ease-in-out 0.5s forwards;
        }

        @keyframes fade-in {
            0% { opacity: 0; }
            100% { opacity: 1; }
        }
    </style>
</head>
<body>
    <div>
        <h1>Microsoft Authentication</h1>
        <p>Authentication successful. You can close this window now.</p>
    </div>
</body>
</html>
";

    stream.write_all(
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
            response.len(),
            response
        )
        .as_bytes(),
    ).unwrap();

    Some(code.to_string())
}
