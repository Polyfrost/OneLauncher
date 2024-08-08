//! Authentication flow manager

use crate::store::{MinecraftCredentials, MinecraftLogin, State};

/// Begin a Microsoft authentication flow.
#[tracing::instrument]
pub async fn begin() -> crate::Result<MinecraftLogin> {
	let state = State::get().await?;
	let mut users = state.users.write().await;

	users.begin().await
}

/// Complete a Microsoft authentication flow to recieve [`MinecraftCredentials`].
#[tracing::instrument]
pub async fn finish(code: &str, flow: MinecraftLogin) -> crate::Result<MinecraftCredentials> {
	let state = State::get().await?;
	let mut users = state.users.write().await;

	users.finish(code, flow).await
}

/// Get the current default user if it exists by [`uuid::Uuid`].
#[tracing::instrument]
pub async fn get_default_user() -> crate::Result<Option<uuid::Uuid>> {
	let state = State::get().await?;
	let users = state.users.read().await;

	Ok(users.default_user)
}

/// Set the current default user by [`uuid::Uuid`].
#[tracing::instrument]
pub async fn set_default_user(user: Option<uuid::Uuid>) -> crate::Result<()> {
	let user = match user {
		Some(user) => Some(get_user(user).await?.id),
		None => None,
	};

	let state = State::get().await?;
	let mut users = state.users.write().await;

	users.default_user = user;
	users.save().await?;

	Ok(())
}

/// Remove a user account by its [`uuid::Uuid`] from the global store.
#[tracing::instrument]
pub async fn remove_user(user: uuid::Uuid) -> crate::Result<()> {
	let state = State::get().await?;
	let mut users = state.users.write().await;

	users.remove(user).await?;

	if users.default_user == Some(user) {
		set_default_user(Some(user)).await?;
	}

	Ok(())
}

// TODO: Store this securely in stronghold
/// Get a list of user [`MinecraftCredentials`].
#[tracing::instrument]
pub async fn users() -> crate::Result<Vec<MinecraftCredentials>> {
	let state = State::get().await?;
	let users = state.users.read().await;

	Ok(users.users.values().cloned().collect())
}

/// Get a specifc user's [`MinecraftCredentials`] by their [`uuid::Uuid`].
/// Use [`crate::store::MinecraftState#refresh`] instead.
#[tracing::instrument]
pub async fn get_user(user: uuid::Uuid) -> crate::Result<MinecraftCredentials> {
	let state = State::get().await?;
	let users = state.users.read().await;

	let user = users
		.users
		.get(&user)
		.ok_or_else(|| anyhow::anyhow!("failed to get nonexistent user with uuid {user}"))?
		.clone();

	Ok(user)
}
