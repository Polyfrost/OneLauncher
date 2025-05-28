use onelauncher_entity::setting_profiles;
use sea_orm::ActiveValue::Set;

use crate::error::LauncherResult;
use crate::store::State;

pub mod dao {
	use onelauncher_entity::setting_profiles;
	use sea_orm::IntoActiveModel;
	use sea_orm::{ActiveModelTrait, EntityTrait};

	use crate::error::{DaoError, LauncherResult};
	use crate::store::State;

	pub async fn get_all_profiles() -> LauncherResult<Vec<setting_profiles::Model>> {
		let state = State::get().await?;
		let db = &state.db;

		Ok(setting_profiles::Entity::find().all(db).await?)
	}

	pub async fn get_profile_or_default(
		name: Option<&String>,
	) -> LauncherResult<setting_profiles::Model> {
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

	pub async fn get_profile_by_name(
		name: &str,
	) -> LauncherResult<Option<setting_profiles::Model>> {
		let state = State::get().await?;
		let db = &state.db;

		Ok(setting_profiles::Entity::find_by_id(name).one(db).await?)
	}

	pub async fn insert_profile(
		profile: setting_profiles::ActiveModel,
	) -> LauncherResult<setting_profiles::Model> {
		if let Some(name) = profile.name.try_as_ref()
			&& (name.is_empty() || profile.is_global())
		{
			return Err(DaoError::InvalidValue(name.to_string(), "name".into()).into());
		}

		let state = State::get().await?;
		let db = &state.db;

		Ok(profile.insert(db).await?)
	}

	pub async fn update_profile_by_name<B>(
        name: &str,
        block: B,
    ) -> LauncherResult<setting_profiles::Model>
    where B: AsyncFnOnce(setting_profiles::ActiveModel) -> LauncherResult<setting_profiles::ActiveModel> {
        let state = State::get().await?;
        let db = &state.db;

        let model = get_profile_by_name(name).await?.ok_or(DaoError::NotFound)?;
        let model = block(model.into_active_model()).await?;
        let model = model.update(db).await?;

        Ok(model)
    }

    /// Updates an existing profile in the database.
    pub async fn update_profile<B>(
        profile: &mut setting_profiles::Model,
        block: B,
    ) -> LauncherResult<&mut setting_profiles::Model>
    where B: AsyncFnOnce(setting_profiles::ActiveModel) -> LauncherResult<setting_profiles::ActiveModel> {
        let state = State::get().await?;
        let db = &state.db;

        let model = profile.clone().into_active_model();
        let model = block(model).await?;
        let model = model.update(db).await?;

        *profile = model;

        Ok(profile)
    }

	pub async fn delete_profile_by_name(name: &str) -> LauncherResult<()> {
		let state = State::get().await?;
		let db = &state.db;

		let deleted = setting_profiles::Entity::delete_by_id(name)
			.exec(db)
			.await?;

		if deleted.rows_affected == 0 {
			return Err(DaoError::NotFound.into());
		}
		Ok(())
	}
}

/// Returns the global settings profile
pub async fn get_global_profile() -> setting_profiles::Model {
	let Ok(state) = State::get().await else {
		return setting_profiles::Model::default_global_profile();
	};

	let settings = state.settings.read().await;
	settings.global_game_settings.clone()
}

/// Creates a new setting profile and inserts it into the database. Returns the inserted entry
pub async fn create_profile<F>(name: &str, block: F) -> LauncherResult<setting_profiles::Model>
where
	F: AsyncFnOnce(setting_profiles::ActiveModel) -> LauncherResult<setting_profiles::ActiveModel>,
{
	let model = setting_profiles::ActiveModel {
		name: Set(name.to_string()),
		..Default::default()
	};

	let model = block(model).await?;

	dao::insert_profile(model).await
}
