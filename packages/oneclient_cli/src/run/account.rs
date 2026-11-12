use std::sync::Arc;

use clap::Subcommand;
use oneclient_core::LauncherState;
use serde::{Deserialize, Serialize};

use super::find_account;
use crate::Output;
use crate::output::{accent, bold, dim, good, table, when};

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum AccountCommand {
    /// List accounts
    #[command(visible_alias = "ls")]
    List,
    /// Sign in with Microsoft, or add an offline account
    Login {
        /// Add an offline account with this username instead
        #[arg(long, value_name = "USERNAME")]
        offline: Option<String>,
    },
    /// Make an account the default for launching
    Use { account: String },
    /// Renew an account's sign-in, or every account's
    Refresh {
        #[arg(required_unless_present = "all")]
        account: Option<String>,
        #[arg(long, conflicts_with = "account")]
        all: bool,
    },
    /// Remove an account
    #[command(visible_alias = "rm")]
    Remove { account: String },
}

pub(super) async fn run(
    state: &Arc<LauncherState>,
    command: AccountCommand,
    out: &Output,
) -> anyhow::Result<()> {
    match command {
        AccountCommand::List => {
            let accounts = state.auth.list_accounts().await;
            if accounts.is_empty() {
                out.line("No accounts yet, add one with `oneclient_cli account login`");
                return Ok(());
            }
            let default = state.auth.default_account().await.ok().flatten();
            let rows: Vec<Vec<String>> = accounts
                .iter()
                .map(|account| {
                    let is_default = default.as_ref().is_some_and(|d| d.id == account.id);
                    vec![
                        if is_default {
                            format!("{} {}", bold(&account.username), good("(default)"))
                        } else {
                            account.username.clone()
                        },
                        if account.is_offline() {
                            "offline".to_string()
                        } else {
                            "microsoft".to_string()
                        },
                        dim(account.id),
                        if account.is_offline() {
                            String::new()
                        } else {
                            when(account.expires)
                        },
                    ]
                })
                .collect();
            out.line(table(&["NAME", "KIND", "UUID", "TOKEN EXPIRES"], &rows));
            Ok(())
        }
        AccountCommand::Login {
            offline: Some(username),
        } => {
            let account = state.auth.add_offline_account(username).await?;
            out.line(format!("Added offline account {}", bold(&account.username)));
            Ok(())
        }
        AccountCommand::Login { offline: None } => {
            let session = state.auth.begin_microsoft_login().await?;
            out.line(format!(
                "To sign in, open {} and enter the code {}",
                accent(&session.device.verification_uri),
                bold(&session.device.user_code)
            ));
            out.line(dim(format!(
                "On this computer you can also sign in at {}",
                session.auth_url()
            )));
            let state_token = session.dedupe_key().to_string();

            let login = state.auth.finish_microsoft_login(session);
            tokio::pin!(login);
            let account = loop {
                tokio::select! {
                    result = &mut login => break result?,
                    () = tokio::time::sleep(std::time::Duration::from_millis(500)) => {
                        if out.is_closed() {
                            state.auth.cancel_microsoft_login(&state_token).await;
                        }
                    }
                }
            };
            out.line(format!(
                "{} Signed in as {}",
                good("✓"),
                bold(&account.username)
            ));
            Ok(())
        }
        AccountCommand::Use { account } => {
            let account = find_account(state, &account).await?;
            state.auth.set_default_account(Some(account.id)).await?;
            out.line(format!("Now playing as {}", bold(&account.username)));
            Ok(())
        }
        AccountCommand::Refresh { account, .. } => {
            let refreshed = match account {
                Some(query) => {
                    let account = find_account(state, &query).await?;
                    vec![state.auth.refresh_account(account.id).await?]
                }
                None => state.auth.refresh_all_accounts().await?,
            };
            for account in refreshed {
                out.line(format!("Refreshed {}", bold(&account.username)));
            }
            Ok(())
        }
        AccountCommand::Remove { account } => {
            let account = find_account(state, &account).await?;
            state.auth.remove_account(account.id).await?;
            out.line(format!("Removed {}", bold(&account.username)));
            Ok(())
        }
    }
}
