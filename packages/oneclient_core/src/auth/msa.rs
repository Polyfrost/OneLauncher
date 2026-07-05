
use std::future::Future;
use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::de::Error as DeError;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use super::data::{AccountKind, MinecraftAccount, MinecraftLogin};
use super::error::{MinecraftAuthError, MinecraftAuthStep};
use crate::constants::{MICROSOFT_CLIENT_ID, MINECRAFT_SCOPES};

const DEVICE_CODE_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";

const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";

const AUTH_STEPS: u64 = 5;

pub async fn begin_login(client: &Client) -> Result<MinecraftLogin, MinecraftAuthError> {
    request_device_code(client).await
}

pub async fn finish_login(
    client: &Client,
    flow: &MinecraftLogin,
    on_progress: impl Fn(&str, u64, u64),
) -> Result<MinecraftAccount, MinecraftAuthError> {
    on_progress("Waiting for sign-in", 0, AUTH_STEPS);
    let msa = poll_device_token(client, flow).await?;
    account_from_msa_token(client, msa, on_progress).await
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

async fn request_device_code(client: &Client) -> Result<MinecraftLogin, MinecraftAuthError> {
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
    flow: &MinecraftLogin,
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
