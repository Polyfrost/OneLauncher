use std::collections::HashMap;

use chrono::Utc;
use uuid::Uuid;

use super::error::AuthError;
use super::offline::{offline_account, validate_offline_username};
use super::data::{AccountKind, MinecraftAccount};
use crate::paths;
use crate::state::LauncherServices;
use crate::LauncherResult;

#[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct CredentialsStore {
    pub users: HashMap<Uuid, MinecraftAccount>,
    pub default_user: Option<Uuid>,
}

impl CredentialsStore {
    pub async fn load() -> LauncherResult<Self> {
        let path = paths::auth_file()?;
        if !path.exists() {
            return Ok(Self::default());
        }

        match polyio::read_json(&path).await {
            Ok(store) => Ok(store),
            Err(err) => {
                tracing::warn!("failed to read auth file: {err}");
                Ok(Self::default())
            }
        }
    }

    pub async fn save(&self) -> LauncherResult<()> {
        let path = paths::auth_file()?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        polyio::write_json(&path, self).await?;
        Ok(())
    }

    pub fn has_microsoft_account(&self) -> bool {
        self.users
            .values()
            .any(|account| account.kind == AccountKind::Microsoft)
    }

    pub fn list_accounts(&self) -> Vec<MinecraftAccount> {
        self.users.values().cloned().collect()
    }

    pub fn get_account(&self, id: Uuid) -> Option<&MinecraftAccount> {
        self.users.get(&id)
    }

    pub async fn commit_account(
        &mut self,
        account: MinecraftAccount,
        services: &LauncherServices,
    ) -> LauncherResult<MinecraftAccount> {
        self.users.insert(account.id, account.clone());

        if self.default_user.is_none() {
            self.default_user = Some(account.id);
        }

        self.save().await?;
        services
            .notifier
            .send_info("Account added", &format!("Signed in as {}", account.username));
        Ok(account)
    }

    pub fn add_offline_account(&mut self, username: String) -> LauncherResult<MinecraftAccount> {
        let account = self.insert_offline_account(username)?;
        Ok(account)
    }

    pub async fn add_offline_account_and_save(
        &mut self,
        username: String,
    ) -> LauncherResult<MinecraftAccount> {
        let account = self.insert_offline_account(username)?;
        self.save().await?;
        Ok(account)
    }

    fn insert_offline_account(&mut self, username: String) -> LauncherResult<MinecraftAccount> {
        if !self.has_microsoft_account() {
            return Err(AuthError::OfflineRequiresMicrosoft.into());
        }

        validate_offline_username(&username)?;

        if self
            .users
            .values()
            .any(|u| u.username.eq_ignore_ascii_case(&username))
        {
            return Err(AuthError::DuplicateUsername { username }.into());
        }

        let account = offline_account(username);
        self.users.insert(account.id, account.clone());
        Ok(account)
    }

    pub async fn refresh_microsoft_account(
        &mut self,
        id: Uuid,
        services: &LauncherServices,
    ) -> LauncherResult<Option<MinecraftAccount>> {
        let Some(existing) = self.users.get(&id).cloned() else {
            return Ok(None);
        };

        if existing.kind != AccountKind::Microsoft {
            return Ok(Some(existing));
        }

        let client = services.requester.http();
        let account = super::msa::refresh_microsoft_account(client, &existing)
            .await
            .map_err(AuthError::from)?;

        self.users.insert(id, account.clone());
        self.save().await?;

        Ok(Some(account))
    }

    pub async fn remove_account(&mut self, id: Uuid) -> LauncherResult<Option<MinecraftAccount>> {
        let removed = self.users.remove(&id);

        if self.default_user == Some(id) {
            self.default_user = self.users.keys().copied().next();
        }

        self.save().await?;
        Ok(removed)
    }

    pub async fn set_default_user(&mut self, id: Option<Uuid>) -> LauncherResult<()> {
        if let Some(id) = id
            && !self.users.contains_key(&id)
        {
            return Err(AuthError::AccountNotFound(id).into());
        }

        self.default_user = id;
        self.save().await?;
        Ok(())
    }

    pub async fn default_account(
        &mut self,
        services: &LauncherServices,
    ) -> LauncherResult<Option<MinecraftAccount>> {
        let id = self
            .default_user
            .or_else(|| self.users.keys().copied().next());

        let Some(id) = id else {
            return Ok(None);
        };

        let Some(account) = self.users.get(&id).cloned() else {
            return Ok(None);
        };

        if self.default_user != Some(id) {
            self.default_user = Some(id);
            self.save().await?;
        }

        if account.kind == AccountKind::Offline {
            if !self.has_microsoft_account() {
                return Err(AuthError::OfflineRequiresMicrosoft.into());
            }
            return Ok(Some(account));
        }

        if account.expires >= Utc::now() {
            return Ok(Some(account));
        }

        let old = account.clone();
        match self.refresh_microsoft_account(id, services).await {
            Ok(Some(updated)) => Ok(Some(updated)),
            Ok(None) => Ok(None),
            Err(err) => {
                if is_transient_auth_error(&err) {
                    Ok(Some(old))
                } else {
                    Err(err)
                }
            }
        }
    }

    pub async fn account_for_launch(
        &mut self,
        id: Uuid,
        services: &LauncherServices,
    ) -> LauncherResult<MinecraftAccount> {
        if !self.users.contains_key(&id) {
            return Err(AuthError::AccountNotFound(id).into());
        }

        let previous_default = self.default_user;
        self.default_user = Some(id);

        let account = self
            .default_account(services)
            .await?
            .ok_or(AuthError::AccountNotFound(id))?;

        self.default_user = previous_default;

        if account.kind == AccountKind::Offline && !self.has_microsoft_account() {
            return Err(AuthError::OfflineRequiresMicrosoft.into());
        }

        Ok(account)
    }
}

fn is_transient_auth_error(err: &crate::LauncherError) -> bool {
    match err {
        crate::LauncherError::AuthError(super::error::AuthError::Minecraft(
            super::error::MinecraftAuthError::RequestError { source, .. },
        )) => source.is_connect() || source.is_timeout(),
        _ => false,
    }
}
