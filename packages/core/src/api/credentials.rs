//! Authentication flow manager

use crate::{error::LauncherResult, store::{credentials::{MinecraftCredentials, MinecraftLogin}, State}};

/// Begin a Microsoft authentication flow.
#[tracing::instrument]
pub async fn begin() -> LauncherResult<MinecraftLogin> {
	let state = State::get().await?;
	let mut store = state.credentials.write().await;

	store.begin().await
}

/// Complete a Microsoft authentication flow to recieve [`MinecraftCredentials`].
#[tracing::instrument]
pub async fn finish(code: &str, flow: MinecraftLogin) -> LauncherResult<MinecraftCredentials> {
	let state = State::get().await?;
	let mut store = state.credentials.write().await;

	store.finish(code, flow).await
}

/// Get the current default user if it exists by [`uuid::Uuid`].
#[tracing::instrument]
pub async fn get_default_user() -> LauncherResult<Option<uuid::Uuid>> {
	let state = State::get().await?;
	let store = state.credentials.read().await;

	Ok(store.default_user)
}

/// Set the current default user by [`uuid::Uuid`].
#[tracing::instrument]
pub async fn set_default_user(user: Option<uuid::Uuid>) -> LauncherResult<()> {
	let user = match user {
		Some(user) => get_user(user).await?.map(|user| user.id),
		None => None,
	};

	let state = State::get().await?;
	let mut store = state.credentials.write().await;

	store.default_user = user;
	store.save().await?;

	Ok(())
}

/// Remove a user account by its [`uuid::Uuid`] from the global store.
#[tracing::instrument]
pub async fn remove_user(user: uuid::Uuid) -> LauncherResult<()> {
	let state = State::get().await?;
	let mut store = state.credentials.write().await;

	store.remove(user).await?;

	if store.default_user == Some(user) {
		set_default_user(Some(user)).await?;
	}

	Ok(())
}

/// Get a list of user [`MinecraftCredentials`].
#[tracing::instrument]
pub async fn get_users() -> LauncherResult<Vec<MinecraftCredentials>> {
	let state = State::get().await?;
	let store = state.credentials.read().await;

	Ok(store.users.values().cloned().collect())
}

/// Get a specifc user's [`MinecraftCredentials`] by their [`uuid::Uuid`].
/// Use [`crate::store::MinecraftState#refresh`] instead.
#[tracing::instrument]
pub async fn get_user(user: uuid::Uuid) -> LauncherResult<Option<MinecraftCredentials>> {
	let state = State::get().await?;
	let store = state.credentials.read().await;

	let user = store
		.users
		.get(&user)
		.cloned();

	Ok(user)
}

/// Get a fake user for testing or offline mode.
pub fn get_fake_user() -> MinecraftCredentials {
	MinecraftCredentials {
		id: uuid::Uuid::new_v4(),
		username: "Player".to_string(),
		access_token: "0".to_string(),
		refresh_token: "0".to_string(),
		expires: chrono::Utc::now() + chrono::Duration::days(1),
	}
}