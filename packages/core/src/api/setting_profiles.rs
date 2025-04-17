use onelauncher_entity::setting_profiles;

use crate::store::State;

pub mod dao {
	use onelauncher_entity::setting_profiles;
	use sea_orm::{ActiveModelTrait, EntityTrait};

	use crate::{error::{DaoError, LauncherResult}, store::State};

	pub async fn get_all_profiles() -> LauncherResult<Vec<setting_profiles::Model>> {
		let state = State::get().await?;
		let db = &state.db;

		Ok(setting_profiles::Entity::find()
			.all(db)
			.await?)
	}

	pub async fn get_profile_or_default(name: Option<&String>) -> LauncherResult<setting_profiles::Model> {
		if let Some(name) = name {
			let profile = get_profile_by_name(name).await?;
			if let Some(profile) = profile {
				return Ok(profile);
			}
		}

		let state = State::get().await?;
		let settings = &state.settings.read().await;
		Ok(settings.global_game_settings.clone())
	}

	pub async fn get_profile_by_name(name: &str) -> LauncherResult<Option<setting_profiles::Model>> {
		let state = State::get().await?;
		let db = &state.db;

		Ok(setting_profiles::Entity::find_by_id(name)
			.one(db)
			.await?)
	}

	pub async fn insert_profile(profile: setting_profiles::ActiveModel) -> LauncherResult<setting_profiles::Model> {
		if let Some(name) = profile.name.try_as_ref() {
			if name.is_empty() || profile.is_global() {
				return Err(DaoError::InvalidValue(name.to_string(), "name".into()).into());
			}
		}

		let state = State::get().await?;
		let db = &state.db;

		Ok(profile.insert(db).await?)
	}

	pub async fn delete_profile_by_name(name: &str) -> LauncherResult<()> {
		let state = State::get().await?;
		let db = &state.db;

		let deleted = setting_profiles::Entity::delete_by_id(name).exec(db).await?;

		if deleted.rows_affected == 0 {
			return Err(DaoError::NotFound.into());
		}
		Ok(())
	}
}

pub async fn get_global_profile() -> setting_profiles::Model {
	let Ok(state) = State::get().await else {
		return setting_profiles::Model::default_global_profile();
	};

	let settings = state.settings.read().await;
	settings.global_game_settings.clone()
}
