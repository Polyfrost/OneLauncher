//! Adapted from Modrinth's `minecraft-auth-errors.ts`
//! (GPL-3.0)

use oneclient_net::{NetworkFailure, classify_network_failure};
use reqwest::StatusCode;

use super::error::{friendly_xbox_error, MinecraftAuthError, MinecraftAuthStep};

#[derive(Debug, Clone, serde::Serialize)]
pub struct AuthErrorGuidance {
    pub what_happened: String,
    pub steps_to_fix: Vec<String>,
}

impl AuthErrorGuidance {
    fn new(what_happened: &str, steps_to_fix: &[&str]) -> Self {
        Self {
            what_happened: what_happened.to_string(),
            steps_to_fix: steps_to_fix.iter().map(|s| (*s).to_string()).collect(),
        }
    }
}

#[must_use]
pub fn diagnose_auth_error(err: &MinecraftAuthError) -> Option<AuthErrorGuidance> {
    match err {
        MinecraftAuthError::XboxError { error_code, .. } => guidance_for_xerr(*error_code),

        MinecraftAuthError::ServiceError { step, status_code } => {
            guidance_for_service_error(*step, *status_code)
        }

        MinecraftAuthError::DeserializeError { step, .. } => guidance_for_step(*step),

        MinecraftAuthError::RequestError { source, .. } => {
            Some(network_guidance(classify_network_failure(source)))
        }

        MinecraftAuthError::BrowserAuthorizationExpired
        | MinecraftAuthError::BrowserLoginNotFound => Some(AuthErrorGuidance::new(
            "The Microsoft sign-in window was closed or expired before sign-in finished.",
            &[
                "Start the sign-in again",
                "Complete the Microsoft sign-in in your browser without closing the window",
                "If your browser blocked the redirect back, allow it and try once more",
            ],
        )),

        _ => None,
    }
}

fn guidance_for_xerr(code: u64) -> Option<AuthErrorGuidance> {
    Some(match code {
        2_148_916_222 => AuthErrorGuidance::new(
            "This account requires age verification to comply with UK regulations before it can sign in.",
            &[
                "Go to the Minecraft login page and sign in (https://www.minecraft.net/en-us/login)",
                "Follow the instructions to verify your age",
                "Once verified, try signing in again",
                "For more help see UK age verification on Xbox (https://support.xbox.com/en-GB/help/family-online-safety/online-safety/UK-age-verification)",
            ],
        ),
        2_148_916_227 => AuthErrorGuidance::new(
            "This account was suspended for violating Xbox Community Standards.",
            &[
                "Visit Xbox Support and review the enforcement details (https://support.xbox.com)",
                "Submit an appeal if one is available",
            ],
        ),
        2_148_916_229 => AuthErrorGuidance::new(
            "This account is restricted and does not have permission to play online.",
            &[
                "Have a guardian sign in to Microsoft Family (https://account.microsoft.com/family/)",
                "Update the online play permissions",
                "Once finished, try signing in again",
            ],
        ),
        2_148_916_233 => AuthErrorGuidance::new(
            "This account does not have an Xbox profile set up, or does not own Minecraft.",
            &[
                "Make sure Minecraft is purchased on this account",
                "Visit the Minecraft login page and sign in (https://www.minecraft.net/en-us/login)",
                "Complete Xbox profile setup if prompted",
                "Once finished, try signing in again",
            ],
        ),
        2_148_916_234 => AuthErrorGuidance::new(
            "This account has not accepted Xbox's Terms of Service.",
            &[
                "Visit Xbox and sign in (https://www.xbox.com)",
                "Accept the Terms if prompted",
                "Once finished, try signing in again",
            ],
        ),
        2_148_916_235 => AuthErrorGuidance::new(
            "Xbox Live is not available in your region, so sign-in is blocked.",
            &[
                "Xbox services must be supported in your country before you can sign in",
                "Check Xbox availability for supported regions (https://www.xbox.com/en-US/regions)",
            ],
        ),
        2_148_916_236 | 2_148_916_237 => AuthErrorGuidance::new(
            "This account requires adult verification under South Korean regulations.",
            &[
                "Visit Xbox and sign in (https://www.xbox.com)",
                "Complete the identity verification process",
                "Once finished, try signing in again",
            ],
        ),
        2_148_916_238 => AuthErrorGuidance::new(
            "This account is underage and not linked to a Microsoft family group.",
            &[
                "Review the Minecraft Family Setup guide (https://help.minecraft.net/hc/en-us/articles/4408968616077)",
                "Join or create a family group as instructed",
                "Once finished, try signing in again",
            ],
        ),
        _ => return None,
    })
}

fn guidance_for_service_error(
    step: MinecraftAuthStep,
    status: StatusCode,
) -> Option<AuthErrorGuidance> {
    if step == MinecraftAuthStep::MinecraftToken {
        if status == StatusCode::TOO_MANY_REQUESTS {
            return Some(AuthErrorGuidance::new(
                "Microsoft or Minecraft temporarily blocked the sign-in because there were too many recent attempts.",
                &[
                    "Wait about an hour before trying again",
                    "Restart OneClient after waiting",
                    "Try signing in once more",
                    "If it keeps happening, wait longer before retrying so the temporary limit can clear",
                ],
            ));
        }
        if status.is_server_error() {
            return Some(AuthErrorGuidance::new(
                "Minecraft's authentication service is returning a server error, so sign-in cannot finish right now.",
                &[
                    "Wait a few minutes and try signing in again",
                    "Check Xbox status for current service issues (https://support.xbox.com/xbox-live-status)",
                    "Try the official Minecraft Launcher to confirm whether Minecraft sign-in is affected there too (https://www.minecraft.net/en-us/download)",
                    "If the service is healthy and this keeps happening, contact support with the debug information",
                ],
            ));
        }
    }

    guidance_for_step(step)
}

fn guidance_for_step(step: MinecraftAuthStep) -> Option<AuthErrorGuidance> {
    Some(match step {
        MinecraftAuthStep::RefreshToken | MinecraftAuthStep::AuthCodeExchange => {
            AuthErrorGuidance::new(
                "Your saved Microsoft sign-in has expired or was revoked, so your Minecraft session could not be renewed.",
                &[
                    "Sign out of the affected Minecraft account in OneClient",
                    "Sign in to the account again",
                    "Once the new sign-in finishes, try launching Minecraft again",
                ],
            )
        }
        MinecraftAuthStep::XblAuthenticate => AuthErrorGuidance::new(
            "Xbox rejected the first sign-in step. This is most often caused by your system clock or time zone being out of sync, or by a temporary Xbox block.",
            &[
                "Open your system date and time settings",
                "Turn on automatic time zone and automatic time, if available",
                "Use the sync option in your settings to synchronise the clock",
                "Restart OneClient and try signing in again",
                "If it persists, check Xbox status (https://support.xbox.com/xbox-live-status)",
            ],
        ),
        MinecraftAuthStep::XstsAuthorize => AuthErrorGuidance::new(
            "Xbox rejected the request to authorize this account for Minecraft, but did not return a specific account restriction we recognise.",
            &[
                "Sign in with the official Minecraft Launcher (https://www.minecraft.net/en-us/download)",
                "Complete any prompts shown by Microsoft, Xbox, or Minecraft",
                "Try signing in to OneClient again",
                "If the official launcher also fails, follow the error shown there or contact Xbox Support",
            ],
        ),
        MinecraftAuthStep::MinecraftProfile => AuthErrorGuidance::new(
            "Minecraft services could not return a Java Edition profile for this account. This usually means the game was purchased recently, the Java profile is not finished being created, or the wrong Microsoft account is being used.",
            &[
                "Sign in with the official Minecraft Launcher and launch Java Edition once (https://www.minecraft.net/en-us/download)",
                "Wait up to an hour if the purchase or profile setup was recent",
                "Make sure you are using the Microsoft account that owns Minecraft",
                "Try signing in to OneClient again",
            ],
        ),
        _ => return None,
    })
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AuthErrorSample {
    pub label: &'static str,
    pub message: String,
    pub guidance: Option<AuthErrorGuidance>,
}

#[must_use]
pub fn preview_samples() -> Vec<AuthErrorSample> {
    use MinecraftAuthStep::{
        MinecraftProfile, MinecraftToken, RefreshToken, XblAuthenticate, XstsAuthorize,
    };

    let errors: Vec<(&'static str, MinecraftAuthError)> = vec![
        (
            "Xbox 403 (RUST-4)",
            MinecraftAuthError::ServiceError {
                step: XblAuthenticate,
                status_code: StatusCode::FORBIDDEN,
            },
        ),
        ("No Xbox profile", xerr(2_148_916_233)),
        ("Region blocked", xerr(2_148_916_235)),
        ("Child account", xerr(2_148_916_238)),
        ("Account suspended", xerr(2_148_916_227)),
        ("Restricted / no online play", xerr(2_148_916_229)),
        ("ToS not accepted", xerr(2_148_916_234)),
        ("UK age verification", xerr(2_148_916_222)),
        ("KR adult verification", xerr(2_148_916_236)),
        (
            "Rate limited (429)",
            MinecraftAuthError::ServiceError {
                step: MinecraftToken,
                status_code: StatusCode::TOO_MANY_REQUESTS,
            },
        ),
        (
            "MC service error (503)",
            MinecraftAuthError::ServiceError {
                step: MinecraftToken,
                status_code: StatusCode::SERVICE_UNAVAILABLE,
            },
        ),
        (
            "Profile fetch failed",
            MinecraftAuthError::ServiceError {
                step: MinecraftProfile,
                status_code: StatusCode::NOT_FOUND,
            },
        ),
        (
            "Session expired",
            MinecraftAuthError::ServiceError {
                step: RefreshToken,
                status_code: StatusCode::BAD_REQUEST,
            },
        ),
        (
            "Generic Xbox reject",
            MinecraftAuthError::ServiceError {
                step: XstsAuthorize,
                status_code: StatusCode::UNAUTHORIZED,
            },
        ),
    ];

    let mut out: Vec<AuthErrorSample> = errors
        .into_iter()
        .map(|(label, err)| AuthErrorSample {
            label,
            message: err.to_string(),
            guidance: diagnose_auth_error(&err),
        })
        .collect();

    out.push(AuthErrorSample {
        label: "Network unreachable",
        message: "error sending request for url (https://user.auth.xboxlive.com/user/authenticate): connection closed".to_string(),
        guidance: Some(network_guidance(NetworkFailure::Generic)),
    });
    out.push(AuthErrorSample {
        label: "TLS certificate rejected",
        message: "error sending request for url (https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode): invalid peer certificate: UnknownIssuer".to_string(),
        guidance: Some(network_guidance(NetworkFailure::Certificate)),
    });
    out.push(AuthErrorSample {
        label: "TLS handshake failed",
        message: "error sending request for url (https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode): received fatal alert: HandshakeFailure".to_string(),
        guidance: Some(network_guidance(NetworkFailure::Handshake)),
    });
    out.push(AuthErrorSample {
        label: "DNS lookup failed",
        message: "error sending request for url (https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode): dns error: failed to lookup address information".to_string(),
        guidance: Some(network_guidance(NetworkFailure::Dns)),
    });
    out
}

fn xerr(code: u64) -> MinecraftAuthError {
    MinecraftAuthError::XboxError {
        step: MinecraftAuthStep::XstsAuthorize,
        error_code: code,
        message: friendly_xbox_error(code).unwrap_or("Xbox rejected sign-in.").to_string(),
        redirect: None,
    }
}

fn network_guidance(failure: NetworkFailure) -> AuthErrorGuidance {
    match failure {
        NetworkFailure::Certificate => AuthErrorGuidance::new(
            "OneClient reached the Microsoft sign-in service, but could not verify its security certificate. Something on this machine or network is intercepting the encrypted connection - usually antivirus HTTPS scanning, a school or workplace filter, or a VPN.",
            &[
                "Turn off HTTPS/SSL scanning in your antivirus (often called \"encrypted connection scanning\", \"web shield\", or \"SSL interception\")",
                "Temporarily disable your VPN or proxy and try signing in again",
                "If you are on a school or workplace network, try a home network or phone hotspot to confirm",
                "Check that your system date, time, and time zone are correct",
            ],
        ),
        NetworkFailure::Handshake => AuthErrorGuidance::new(
            "OneClient reached the Microsoft sign-in service, but the two could not agree on a secure connection. This normally means a proxy, filter, or antivirus in between is using outdated encryption settings.",
            &[
                "Turn off HTTPS/SSL scanning in your antivirus and try signing in again",
                "Temporarily disable your VPN or proxy software",
                "If you are on a school or workplace network, try a home network or phone hotspot to confirm",
                "Make sure OneClient is up to date",
            ],
        ),
        NetworkFailure::Dns => AuthErrorGuidance::new(
            "OneClient could not look up the address of the Microsoft sign-in service. This is a DNS problem on this machine or network, not a problem with your account.",
            &[
                "Check that your internet connection is working",
                "Temporarily disable your VPN or proxy and try signing in again",
                "Check your hosts file for entries blocking Microsoft or Minecraft domains",
                "Try a different DNS server, such as Cloudflare (1.1.1.1) or Google (8.8.8.8)",
                "Restart OneClient and try signing in again",
            ],
        ),
        NetworkFailure::Generic => AuthErrorGuidance::new(
            "OneClient could not connect to a Microsoft, Xbox, or Minecraft service needed for sign-in. This is usually a local network, DNS, proxy, firewall, hosts file, VPN, or antivirus issue.",
            &[
                "Restart OneClient and try signing in again",
                "Check that your internet connection is working",
                "Allow OneClient through your firewall, antivirus, proxy, VPN, and hosts file rules",
                "Try a different network, or temporarily disable VPN/proxy software if you use one",
                "If routing or DNS is the issue, a service like Cloudflare WARP can sometimes help",
            ],
        ),
    }
}
