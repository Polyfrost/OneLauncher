
use std::future::Future;
use std::time::Duration;

use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::de::Error as DeError;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use url::Url;
use uuid::Uuid;

use super::data::{AccountKind, BrowserLogin, DeviceCodeLogin, MinecraftAccount};
use super::error::{MinecraftAuthError, MinecraftAuthStep};
use crate::constants::{MICROSOFT_CLIENT_ID, MINECRAFT_SCOPES};

const DEVICE_CODE_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";

const AUTHORIZE_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize";

const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";

const AUTH_STEPS: u64 = 5;


pub async fn begin_device_login(client: &Client) -> Result<DeviceCodeLogin, MinecraftAuthError> {
    request_device_code(client).await
}

pub async fn finish_dual_login(
    client: &Client,
    pending: PendingBrowserLogin,
    device: &DeviceCodeLogin,
    on_progress: impl Fn(&str, u64, u64),
) -> Result<MinecraftAccount, MinecraftAuthError> {
    on_progress("Waiting for sign-in", 0, AUTH_STEPS);

    let deadline = Duration::from_secs(device.expires_in);
    let msa = tokio::time::timeout(deadline, race_msa_token(client, &pending, device))
        .await
        .map_err(|_| MinecraftAuthError::DeviceAuthorizationExpired)??;

    account_from_msa_token(client, msa, on_progress).await
}

async fn race_msa_token(
    client: &Client,
    pending: &PendingBrowserLogin,
    device: &DeviceCodeLogin,
) -> Result<MsaToken, MinecraftAuthError> {
    let browser = browser_msa_token(client, pending);
    let device = poll_device_token(client, device);
    tokio::pin!(browser, device);

    let mut browser_done = false;
    let mut device_done = false;
    let mut last_err: Option<MinecraftAuthError> = None;

    loop {
        tokio::select! {
            res = &mut browser, if !browser_done => match res {
                Ok(token) => return Ok(token),
                Err(err) => {
                    browser_done = true;
                    if device_done {
                        return Err(err);
                    }
                    last_err = Some(err);
                }
            },
            res = &mut device, if !device_done => match res {
                Ok(token) => return Ok(token),
                Err(err) => {
                    device_done = true;
                    if browser_done {
                        return Err(err);
                    }
                    last_err = Some(err);
                }
            },
            else => return Err(last_err.unwrap_or(MinecraftAuthError::DeviceAuthorizationExpired)),
        }
    }
}

pub struct PendingBrowserLogin {
    listener: TcpListener,
    pkce_verifier: String,
    redirect_uri: String,
    csrf_state: String,
}

pub async fn begin_browser_login() -> Result<(BrowserLogin, PendingBrowserLogin), MinecraftAuthError>
{
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|err| MinecraftAuthError::LoopbackBind(err.to_string()))?;
    let port = listener
        .local_addr()
        .map_err(|err| MinecraftAuthError::LoopbackBind(err.to_string()))?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}");

    let pkce_verifier = random_token();
    let code_challenge = pkce_challenge(&pkce_verifier);
    let csrf_state = random_token();

    let mut url = Url::parse(AUTHORIZE_URL).expect("authorize url is valid");
    url.query_pairs_mut()
        .append_pair("client_id", MICROSOFT_CLIENT_ID)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("response_mode", "query")
        .append_pair("scope", MINECRAFT_SCOPES)
        .append_pair("state", &csrf_state)
        .append_pair("code_challenge", &code_challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("prompt", "select_account");

    let login = BrowserLogin {
        auth_url: url.to_string(),
        state: csrf_state.clone(),
        redirect_uri: redirect_uri.clone(),
    };
    let pending = PendingBrowserLogin {
        listener,
        pkce_verifier,
        redirect_uri,
        csrf_state,
    };

    Ok((login, pending))
}

async fn browser_msa_token(
    client: &Client,
    pending: &PendingBrowserLogin,
) -> Result<MsaToken, MinecraftAuthError> {
    let code = wait_for_redirect(pending).await?;
    exchange_auth_code(client, &code, pending).await
}

async fn wait_for_redirect(pending: &PendingBrowserLogin) -> Result<String, MinecraftAuthError> {
    loop {
        let (mut stream, _) = pending
            .listener
            .accept()
            .await
            .map_err(|err| MinecraftAuthError::LoopbackBind(err.to_string()))?;

        let mut buf = [0u8; 2048];
        let read = stream.read(&mut buf).await.unwrap_or(0);
        let request = String::from_utf8_lossy(&buf[..read]);
        let Some(target) = request.split_whitespace().nth(1) else {
            respond(&mut stream, RedirectPage::Waiting).await;
            continue;
        };

        if !target.contains('?') {
            respond(&mut stream, RedirectPage::Waiting).await;
            continue;
        }

        let full = format!("http://127.0.0.1{target}");
        let parsed = Url::parse(&full).ok();
        let mut code = None;
        let mut state = None;
        let mut error = None;
        if let Some(parsed) = &parsed {
            for (key, value) in parsed.query_pairs() {
                match key.as_ref() {
                    "code" => code = Some(value.into_owned()),
                    "state" => state = Some(value.into_owned()),
                    "error" => error = Some(value.into_owned()),
                    "error_description" => error = Some(value.into_owned()),
                    _ => {}
                }
            }
        }

        if let Some(error) = error {
            respond(&mut stream, RedirectPage::Failed).await;
            return Err(MinecraftAuthError::BrowserAuthorizationFailed { error });
        }

        if state.as_deref() != Some(pending.csrf_state.as_str()) {
            respond(&mut stream, RedirectPage::Failed).await;
            continue;
        }

        if let Some(code) = code {
            respond(&mut stream, RedirectPage::Success).await;
            return Ok(code);
        }

        respond(&mut stream, RedirectPage::Waiting).await;
    }
}

#[derive(Clone, Copy)]
enum RedirectPage {
    Waiting,
    Success,
    Failed,
}

async fn respond(stream: &mut tokio::net::TcpStream, page: RedirectPage) {
    let body = redirect_page_html(page);
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.flush().await;
}

fn redirect_page_html(page: RedirectPage) -> String {
    let (accent, glyph, heading, detail) = match page {
        RedirectPage::Waiting => (
            "#2567d8",
            // Spinning ring.
            r#"<div class="ring"></div>"#,
            "Signing you in",
            "Finish signing in with Microsoft in this window.",
        ),
        RedirectPage::Success => (
            "#45de2b",
            r#"<svg viewBox="0 0 24 24" class="glyph"><path d="M20 6 9 17l-5-5"/></svg>"#,
            "You're signed in!",
            "You can close this tab and return to OneClient.",
        ),
        RedirectPage::Failed => (
            "#ff0000",
            r#"<svg viewBox="0 0 24 24" class="glyph"><path d="M18 6 6 18M6 6l12 12"/></svg>"#,
            "Sign-in failed",
            "Something went wrong. Close this tab and try again from OneClient.",
        ),
    };

    format!(
        r##"<!doctype html><html lang="en"><head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>OneClient</title>
<style>
*{{box-sizing:border-box;margin:0;padding:0}}
:root{{--accent:{accent}}}
html,body{{height:100%}}
body{{
  font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Helvetica,Arial,sans-serif;
  color:#111827;
  background:#ffffff;
  display:flex;align-items:center;justify-content:center;min-height:100vh;padding:24px;
}}
.card{{
  position:relative;width:100%;max-width:420px;text-align:center;
  padding:44px 36px 40px;border-radius:20px;
  background:#ffffff;
  border:1px solid #e5e7eb;
  box-shadow:0 10px 30px rgba(17,24,39,.10);
  animation:rise .5s cubic-bezier(.2,.8,.2,1) both;
}}
.badge{{width:76px;height:76px;margin:0 auto 26px;border-radius:50%;
  display:flex;align-items:center;justify-content:center;
  background:color-mix(in srgb, var(--accent) 12%, transparent);
  border:1px solid color-mix(in srgb, var(--accent) 40%, transparent)}}
.glyph{{width:34px;height:34px;fill:none;stroke:var(--accent);stroke-width:2.4;
  stroke-linecap:round;stroke-linejoin:round;animation:pop .4s .15s cubic-bezier(.2,1.4,.3,1) both}}
.ring{{width:34px;height:34px;border-radius:50%;
  border:3px solid rgba(37,103,216,.22);border-top-color:#2567d8;animation:spin .8s linear infinite}}
h1{{font-size:21px;font-weight:600;color:#111827;margin-bottom:10px}}
p{{font-size:14px;line-height:1.55;color:#6b7280}}
@keyframes spin{{to{{transform:rotate(360deg)}}}}
@keyframes rise{{from{{opacity:0;transform:translateY(10px)}}to{{opacity:1;transform:none}}}}
@keyframes pop{{from{{opacity:0;transform:scale(.6)}}to{{opacity:1;transform:none}}}}
</style></head>
<body>
  <main class="card">
    <div class="badge">{glyph}</div>
    <h1>{heading}</h1>
    <p>{detail}</p>
  </main>
</body></html>"##
    )
}

async fn exchange_auth_code(
    client: &Client,
    code: &str,
    pending: &PendingBrowserLogin,
) -> Result<MsaToken, MinecraftAuthError> {
    let body = [
        ("client_id", MICROSOFT_CLIENT_ID),
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", pending.redirect_uri.as_str()),
        ("code_verifier", pending.pkce_verifier.as_str()),
        ("scope", MINECRAFT_SCOPES),
    ];

    let res = auth_retry(|| {
        client
            .post(TOKEN_URL)
            .header("Accept", "application/json")
            .form(&body)
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthError::RequestError {
        step: MinecraftAuthStep::AuthCodeExchange,
        source,
    })?;

    parse_json_response(res, MinecraftAuthStep::AuthCodeExchange)
        .await
        .map(MsaToken::from_response)
}

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    bytes[..16].copy_from_slice(Uuid::new_v4().as_bytes());
    bytes[16..].copy_from_slice(Uuid::new_v4().as_bytes());
    BASE64_URL_SAFE_NO_PAD.encode(bytes)
}

fn pkce_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    BASE64_URL_SAFE_NO_PAD.encode(digest)
}

pub async fn refresh_microsoft_account(
    client: &Client,
    creds: &MinecraftAccount,
) -> Result<MinecraftAccount, MinecraftAuthError> {
    let msa = refresh_msa_token(client, &creds.refresh_token).await?;
    let mut account = account_from_msa_token(client, msa, |_, _, _| {}).await?;
    account.id = creds.id;
    account.username = creds.username.clone();
    Ok(account)
}

async fn request_device_code(client: &Client) -> Result<DeviceCodeLogin, MinecraftAuthError> {
    let body = [
        ("client_id", MICROSOFT_CLIENT_ID),
        ("scope", MINECRAFT_SCOPES),
    ];

    let res = auth_retry(|| {
        client
            .post(DEVICE_CODE_URL)
            .header("Accept", "application/json")
            .form(&body)
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthError::RequestError {
        step: MinecraftAuthStep::DeviceCodeRequest,
        source,
    })?;

    parse_json_response(res, MinecraftAuthStep::DeviceCodeRequest).await
}

async fn poll_device_token(
    client: &Client,
    flow: &DeviceCodeLogin,
) -> Result<MsaToken, MinecraftAuthError> {
    let deadline = Utc::now() + chrono::TimeDelta::seconds(flow.expires_in as i64);
    let mut interval_secs = flow.interval.max(5);

    loop {
        if Utc::now() >= deadline {
            return Err(MinecraftAuthError::DeviceAuthorizationExpired);
        }

        let body = [
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("client_id", MICROSOFT_CLIENT_ID),
            ("device_code", flow.device_code.as_str()),
        ];

        let res = auth_retry(|| {
            client
                .post(TOKEN_URL)
                .header("Accept", "application/json")
                .form(&body)
                .send()
        })
        .await
        .map_err(|source| MinecraftAuthError::RequestError {
            step: MinecraftAuthStep::DeviceCodePoll,
            source,
        })?;

        let status = res.status();
        let text = res
            .text()
            .await
            .map_err(|source| MinecraftAuthError::RequestError {
                step: MinecraftAuthStep::DeviceCodePoll,
                source,
            })?;

        if status.is_success() {
            let raw: MsaTokenResponse = serde_json::from_str(&text).map_err(|source| {
                MinecraftAuthError::DeserializeError {
                    step: MinecraftAuthStep::DeviceCodePoll,
                    raw: text,
                    source,
                    status_code: status,
                }
            })?;
            return Ok(MsaToken::from_response(raw));
        }

        if let Ok(err) = serde_json::from_str::<OAuthErrorResponse>(&text) {
            match err.error.as_str() {
                "authorization_pending" => {
                    tokio::time::sleep(Duration::from_secs(interval_secs)).await;
                    continue;
                }
                "slow_down" => {
                    interval_secs = interval_secs.saturating_add(5);
                    tokio::time::sleep(Duration::from_secs(interval_secs)).await;
                    continue;
                }
                "expired_token" => return Err(MinecraftAuthError::DeviceAuthorizationExpired),
                other => {
                    return Err(MinecraftAuthError::DeviceAuthorizationFailed {
                        error: err
                            .error_description
                            .unwrap_or_else(|| other.to_string()),
                    });
                }
            }
        }

        return Err(MinecraftAuthError::DeserializeError {
            step: MinecraftAuthStep::DeviceCodePoll,
            raw: text,
            source: DeError::custom("unexpected device code poll response"),
            status_code: status,
        });
    }
}

async fn refresh_msa_token(
    client: &Client,
    refresh_token: &str,
) -> Result<MsaToken, MinecraftAuthError> {
    let body = [
        ("client_id", MICROSOFT_CLIENT_ID),
        ("refresh_token", refresh_token),
        ("grant_type", "refresh_token"),
        ("scope", MINECRAFT_SCOPES),
    ];

    let res = auth_retry(|| {
        client
            .post(TOKEN_URL)
            .header("Accept", "application/json")
            .form(&body)
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthError::RequestError {
        step: MinecraftAuthStep::RefreshToken,
        source,
    })?;

    parse_json_response(res, MinecraftAuthStep::RefreshToken).await
        .map(MsaToken::from_response)
}

struct MsaToken {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
    obtained_at: DateTime<Utc>,
}

impl MsaToken {
    fn from_response(raw: MsaTokenResponse) -> Self {
        Self {
            access_token: raw.access_token,
            refresh_token: raw.refresh_token,
            expires_in: raw.expires_in,
            obtained_at: Utc::now(),
        }
    }
}

async fn account_from_msa_token(
    client: &Client,
    msa: MsaToken,
    on_progress: impl Fn(&str, u64, u64),
) -> Result<MinecraftAccount, MinecraftAuthError> {
    let rps_ticket = format!("d={}", msa.access_token);
    on_progress("Authenticating with Xbox Live", 1, AUTH_STEPS);
    let xbl = xbl_authenticate(client, &rps_ticket).await?;
    on_progress("Authorizing access (XSTS)", 2, AUTH_STEPS);
    let xsts = xsts_authorize(client, &xbl.token).await?;

    let xsts_token = xsts.token.ok_or(MinecraftAuthError::HashError)?;

    let uhs: &str = xsts
        .display_claims
        .as_ref()
        .and_then(|c| c.xui.first())
        .map(|x| x.uhs.as_str())
        .ok_or(MinecraftAuthError::HashError)?;

    on_progress("Requesting Minecraft token", 3, AUTH_STEPS);
    let minecraft_token = minecraft_login(client, uhs, &xsts_token).await?;
    on_progress("Verifying game ownership", 4, AUTH_STEPS);
    minecraft_entitlements(client, &minecraft_token.access_token).await?;
    let profile = minecraft_profile(client, &minecraft_token.access_token).await?;
    on_progress("Loading profile", AUTH_STEPS, AUTH_STEPS);

    Ok(MinecraftAccount {
        id: profile.id.unwrap_or_default(),
        username: profile.name,
        access_token: minecraft_token.access_token,
        refresh_token: msa.refresh_token,
        #[allow(clippy::cast_possible_wrap)]
        expires: msa.obtained_at + chrono::TimeDelta::seconds(msa.expires_in as i64),
        kind: AccountKind::Microsoft,
    })
}

async fn xbl_authenticate(
    client: &Client,
    rps_ticket: &str,
) -> Result<XblUserToken, MinecraftAuthError> {
    let res = auth_retry(|| {
        client
            .post("https://user.auth.xboxlive.com/user/authenticate")
            .header("Accept", "application/json")
            .header("x-xbl-contract-version", "1")
            .json(&json!({
                "Properties": {
                    "AuthMethod": "RPS",
                    "SiteName": "user.auth.xboxlive.com",
                    "RpsTicket": rps_ticket
                },
                "RelyingParty": "http://auth.xboxlive.com",
                "TokenType": "JWT"
            }))
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthError::RequestError {
        step: MinecraftAuthStep::XblAuthenticate,
        source,
    })?;

    let token: XboxTokenResponse =
        parse_json_response(res, MinecraftAuthStep::XblAuthenticate).await?;
    check_xbox_error(MinecraftAuthStep::XblAuthenticate, &token)?;

    Ok(XblUserToken {
        token: token.token.ok_or(MinecraftAuthError::DeserializeError {
            step: MinecraftAuthStep::XblAuthenticate,
            raw: String::new(),
            source: DeError::custom("missing Token"),
            status_code: reqwest::StatusCode::OK,
        })?,
    })
}

async fn xsts_authorize(
    client: &Client,
    user_token: &str,
) -> Result<XboxTokenResponse, MinecraftAuthError> {
    let res = auth_retry(|| {
        client
            .post("https://xsts.auth.xboxlive.com/xsts/authorize")
            .header("Accept", "application/json")
            .header("x-xbl-contract-version", "1")
            .json(&json!({
                "Properties": {
                    "SandboxId": "RETAIL",
                    "UserTokens": [user_token]
                },
                "RelyingParty": "rp://api.minecraftservices.com/",
                "TokenType": "JWT"
            }))
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthError::RequestError {
        step: MinecraftAuthStep::XstsAuthorize,
        source,
    })?;

    let token: XboxTokenResponse =
        parse_json_response(res, MinecraftAuthStep::XstsAuthorize).await?;
    check_xbox_error(MinecraftAuthStep::XstsAuthorize, &token)?;
    Ok(token)
}

#[derive(Deserialize)]
struct MinecraftToken {
    access_token: String,
}

async fn minecraft_login(
    client: &Client,
    uhs: &str,
    xsts_token: &str,
) -> Result<MinecraftToken, MinecraftAuthError> {
    let res = auth_retry(|| {
        client
            .post("https://api.minecraftservices.com/authentication/login_with_xbox")
            .header("Accept", "application/json")
            .json(&json!({
                "identityToken": format!("XBL3.0 x={uhs};{xsts_token}"),
            }))
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthError::RequestError {
        step: MinecraftAuthStep::MinecraftToken,
        source,
    })?;

    parse_json_response(res, MinecraftAuthStep::MinecraftToken).await
}

#[derive(Deserialize)]
struct MinecraftProfile {
    id: Option<Uuid>,
    name: String,
}

async fn minecraft_profile(
    client: &Client,
    token: &str,
) -> Result<MinecraftProfile, MinecraftAuthError> {
    let res = auth_retry(|| {
        client
            .get("https://api.minecraftservices.com/minecraft/profile")
            .header("Accept", "application/json")
            .bearer_auth(token)
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthError::RequestError {
        step: MinecraftAuthStep::MinecraftProfile,
        source,
    })?;

    parse_json_response(res, MinecraftAuthStep::MinecraftProfile).await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MinecraftEntitlements {}

async fn minecraft_entitlements(
    client: &Client,
    token: &str,
) -> Result<MinecraftEntitlements, MinecraftAuthError> {
    let res = auth_retry(|| {
        client
            .get("https://api.minecraftservices.com/entitlements/mcstore")
            .header("Accept", "application/json")
            .bearer_auth(token)
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthError::RequestError {
        step: MinecraftAuthStep::MinecraftEntitlements,
        source,
    })?;

    parse_json_response(res, MinecraftAuthStep::MinecraftEntitlements).await
}

#[derive(Deserialize)]
struct OAuthErrorResponse {
    error: String,
    error_description: Option<String>,
}

#[derive(Deserialize)]
struct MsaTokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
}

struct XblUserToken {
    token: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XboxTokenResponse {
    #[serde(default)]
    #[serde(rename = "XErr")]
    x_err: u64,
    #[serde(default)]
    #[serde(rename = "Message")]
    message: String,
    #[serde(rename = "Redirect", default)]
    redirect: Option<String>,
    #[serde(default)]
    token: Option<String>,
    #[serde(default)]
    display_claims: Option<DisplayClaims>,
}

#[derive(Debug, Deserialize)]
struct DisplayClaims {
    #[serde(default)]
    xui: Vec<XuiClaim>,
}

#[derive(Debug, Deserialize)]
struct XuiClaim {
    uhs: String,
}

fn check_xbox_error(
    step: MinecraftAuthStep,
    token: &XboxTokenResponse,
) -> Result<(), MinecraftAuthError> {
    if token.x_err == 0 {
        return Ok(());
    }

    Err(MinecraftAuthError::XboxError {
        step,
        error_code: token.x_err,
        message: token.message.clone(),
        redirect: token.redirect.clone(),
    })
}

async fn parse_json_response<T: for<'de> Deserialize<'de>>(
    res: reqwest::Response,
    step: MinecraftAuthStep,
) -> Result<T, MinecraftAuthError> {
    let status = res.status();
    let text = res
        .text()
        .await
        .map_err(|source| MinecraftAuthError::RequestError { step, source })?;

    if !status.is_success() {
        tracing::error!("[auth] step={step:?} status={status} body={text}");
    }

    serde_json::from_str(&text).map_err(|source| MinecraftAuthError::DeserializeError {
        step,
        raw: text,
        source,
        status_code: status,
    })
}

async fn auth_retry<F>(request_fn: impl Fn() -> F) -> Result<reqwest::Response, reqwest::Error>
where
    F: Future<Output = Result<reqwest::Response, reqwest::Error>>,
{
    const RETRY_COUNT: usize = 5;
    const RETRY_WAIT: Duration = Duration::from_millis(250);
    let mut resp = request_fn().await;

    for i in 0..RETRY_COUNT {
        match &resp {
            Ok(_) => break,
            Err(err) if (err.is_connect() || err.is_timeout()) && i < RETRY_COUNT - 1 => {
                tokio::time::sleep(RETRY_WAIT).await;
                resp = request_fn().await;
            }
            _ => break,
        }
    }

    resp
}
