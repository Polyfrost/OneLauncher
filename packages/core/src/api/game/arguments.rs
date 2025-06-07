//! Programatic Minecraft argument parser which replaces variables in arguments

use std::collections::{HashMap, HashSet};
use std::path::Path;

use futures::{pin_mut, AsyncReadExt, TryStreamExt};
use interpulse::api::minecraft::{Argument, ArgumentValue, Library, VersionType};
use interpulse::api::modded::SidedDataEntry;
use interpulse::utils::get_path_from_artifact;
use onelauncher_entity::resolution::Resolution;

use crate::constants::{self, DUMMY_REPLACE_NEWLINE};
use crate::error::LauncherResult;
use crate::store::credentials::MinecraftCredentials;
use crate::store::Core;
use crate::utils::io;

#[allow(clippy::too_many_arguments)]
pub fn java_arguments(
	arguments: Option<&[Argument]>,
	natives_path: &Path,
	libraries_path: &Path,
	classpaths: &str,
	version: &str,
	mem_max: u32,
	custom_args: String,
	java_arch: &str,
) -> LauncherResult<Vec<String>> {
	let mut parsed = Vec::new();
	if let Some(args) = arguments {
		parse_arguments(
			args,
			&mut parsed,
			|a| {
				parse_java_argument(
					a.to_string(),
					natives_path,
					libraries_path,
					classpaths,
					version,
				)
			},
			java_arch,
		)?;
	} else {
		parsed.push(format!(
			"-Djava.library.path={}",
			io::canonicalize(natives_path)
				.map_err(|_| anyhow::anyhow!(
					"specified natives path {} not found",
					natives_path.to_string_lossy()
				))?
				.to_string_lossy()
		));
		parsed.push("-cp".to_string());
		parsed.push(classpaths.to_string());
	}

	parsed.push(format!("-Xmx{mem_max}M"));

	if !custom_args.is_empty() {
		parsed.push(custom_args);
	}

	Ok(parsed)
}

#[allow(clippy::too_many_arguments)]
pub fn minecraft_arguments(
	args: Option<&[Argument]>,
	legacy_args: Option<&str>,
	creds: &MinecraftCredentials,
	version: &str,
	asset_index: &str,
	game_directory: &Path,
	assets_directory: &Path,
	version_type: VersionType,
	resolution: Resolution,
	java_arch: &str,
) -> LauncherResult<Vec<String>> {
	if let Some(args) = args {
		let mut parsed = Vec::new();
		parse_arguments(
			args,
			&mut parsed,
			|arg| {
				parse_minecraft_argument(
					arg,
					&creds.access_token,
					&creds.username,
					creds.id,
					version,
					asset_index,
					game_directory,
					assets_directory,
					version_type,
					resolution,
				)
			},
			java_arch,
		)?;

		Ok(parsed)
	} else if let Some(legacy_args) = legacy_args {
		let mut parsed = Vec::new();
		for arg in legacy_args.split(' ') {
			parsed.push(parse_minecraft_argument(
				&arg.replace(' ', DUMMY_REPLACE_NEWLINE),
				&creds.access_token,
				&creds.username,
				creds.id,
				version,
				asset_index,
				game_directory,
				assets_directory,
				version_type,
				resolution,
			)?);
		}

		Ok(parsed)
	} else {
		Ok(Vec::new())
	}
}

pub fn processor_arguments<T: AsRef<str>, S: ::std::hash::BuildHasher>(
	libraries_path: &Path,
	args: &[T],
	data: &HashMap<String, SidedDataEntry, S>,
) -> LauncherResult<Vec<String>> {
	let mut parsed = Vec::new();

	for arg in args {
		let a = &arg.as_ref()[1..arg.as_ref().len() - 1];
		if arg.as_ref().starts_with('{') {
			if let Some(entry) = data.get(a) {
				parsed.push(if entry.client.starts_with('[') {
					get_library(
						libraries_path,
						&entry.client[1..entry.client.len() - 1],
						true,
					)?
				} else {
					entry.client.clone()
				});
			}
		} else if arg.as_ref().starts_with('[') {
			parsed.push(get_library(libraries_path, a, true)?);
		} else {
			parsed.push(arg.as_ref().to_string());
		}
	}

	Ok(parsed)
}

pub async fn main_class(path: String) -> LauncherResult<Option<String>> {
	let data = io::read(path).await?;
	let mut class_name: Option<String> = None;
	let reader = io::stream_zip_entries_bytes(data);

	pin_mut!(reader);
	while let Ok(item) = reader.try_next().await {
		let Some((index, entry, reader)) = item else {
			continue;
		};

		if entry.dir().unwrap_or(true) {
			continue;
		}

		if entry.filename().as_str().unwrap_or_default() != "META-INF/MANIFEST.MF" {
			continue;
		}

		let mut buf = String::new();
		let mut entry_reader = match reader.reader_without_entry(index).await {
			Ok(reader) => reader,
			Err(err) => {
				tracing::error!("failed to get entry reader for {}: {}", entry.filename().as_str().unwrap_or_default(), err);
				continue;
			}
		};

		if let Err(err) = entry_reader.read_to_string(&mut buf).await {
			tracing::error!("failed to read entry {}: {}", entry.filename().as_str().unwrap_or_default(), err);
			continue;
		}

		for line in buf.lines() {
			let line = line.trim();
			if line.starts_with("Main-Class:") {
				if let Some(class) = line.split(':').nth(1) {
					class_name = Some(class.trim().to_string());
					break;
				}
			}
		}
	}

	Ok(class_name)
}

#[allow(clippy::too_many_arguments)]
pub fn parse_minecraft_argument(
	argument: &str,
	access_token: &str,
	username: &str,
	uuid: uuid::Uuid,
	version: &str,
	asset_index: &str,
	game_directory: &Path,
	assets_directory: &Path,
	version_type: VersionType,
	resolution: Resolution,
) -> LauncherResult<String> {
	#[allow(clippy::literal_string_with_formatting_args)]
	Ok(argument
		.replace("${accessToken}", access_token)
		.replace("${auth_access_token}", access_token)
		.replace("${auth_session}", access_token)
		.replace("${auth_player_name}", username)
		.replace("${auth_xuid}", "0") // TODO: add auth xuid
		.replace("${auth_uuid}", &uuid.simple().to_string())
		.replace("${uuid}", &uuid.simple().to_string())
		.replace("${clientid}", &Core::get().msa_client_id)
		.replace("${user_properties}", "{}")
		.replace("${user_type}", "msa")
		.replace("${version_name}", version)
		.replace("${assets_index_name}", asset_index)
		.replace(
			"${game_directory}",
			&io::canonicalize(game_directory)?.to_string_lossy(),
		)
		.replace(
			"${assets_root}",
			&io::canonicalize(assets_directory)?.to_string_lossy(),
		)
		.replace(
			"${game_assets}",
			&io::canonicalize(assets_directory)?.to_string_lossy(),
		)
		.replace("${version_type}", version_type.as_str())
		.replace("${resolution_width}", &resolution.width.to_string())
		.replace("${resolution_height}", &resolution.height.to_string()))
}

#[allow(clippy::too_many_arguments)]
fn parse_java_argument(
	mut argument: String,
	natives_path: &Path,
	libraries_path: &Path,
	classpaths: &str,
	version: &str,
) -> LauncherResult<String> {
	argument.retain(|c| !c.is_whitespace());
	Ok(argument
		.replace(
			"${natives_directory}",
			&io::canonicalize(natives_path)?.to_string_lossy(),
		)
		.replace(
			"${library_directory}",
			&io::canonicalize(libraries_path)?.to_string_lossy(),
		)
		.replace("${classpath_separator}", constants::CLASSPATH_SEPARATOR)
		.replace("${launcher_name}", &Core::get().launcher_name)
		.replace("${launcher_version}", &Core::get().launcher_version)
		.replace("${version_name}", version)
		.replace("${classpath}", classpaths))
}

pub fn classpaths(
	libraries_path: &Path,
	libraries: &[Library],
	client_path: &Path,
	java_arch: &str,
	updated: bool,
) -> LauncherResult<String> {
	let mut classpaths = libraries
		.iter()
		.filter_map(|lib| {
			if let Some(rules) = &lib.rules
				&& !super::rules::validate_rules(rules, java_arch, updated) {
					return None;
				}

			if !lib.include_in_classpath {
				return None;
			}

			Some(get_library(libraries_path, &lib.name, false))
		})
		.collect::<Result<HashSet<_>, _>>()?;

	classpaths.insert(
		io::canonicalize(client_path)?
			.to_string_lossy()
			.to_string(),
	);

	Ok(classpaths
		.into_iter()
		.collect::<Vec<_>>()
		.join(constants::CLASSPATH_SEPARATOR))
}

pub fn get_classpath_library<T: AsRef<str>>(
	libraries_path: &Path,
	libraries: &[T],
) -> LauncherResult<String> {
	let classpaths = libraries
		.iter()
		.map(|lib| get_library(libraries_path, lib.as_ref(), false))
		.collect::<Result<Vec<_>, _>>()?;

	Ok(classpaths.join(constants::CLASSPATH_SEPARATOR))
}

pub fn get_library(
	libraries_path: &Path,
	library: &str,
	error_exist: bool,
) -> LauncherResult<String> {
	let mut path = libraries_path.to_path_buf();
	path.push(get_path_from_artifact(library)?);

	if !path.exists() && error_exist {
		return Ok(path.to_string_lossy().to_string());
	}

	let path = &io::canonicalize(&path)?;

	Ok(path.to_string_lossy().to_string())
}

fn parse_arguments<ParseFn>(
	args: &[Argument],
	parsed: &mut Vec<String>,
	parse_function: ParseFn,
	java_arch: &str,
) -> LauncherResult<()>
where
	ParseFn: Fn(&str) -> LauncherResult<String>,
{
	for arg in args {
		match arg {
			Argument::Normal(a) => {
				let p = parse_function(&a.replace(' ', DUMMY_REPLACE_NEWLINE))?;
				for split_p in p.split(DUMMY_REPLACE_NEWLINE) {
					parsed.push(split_p.to_string());
				}
			}
			Argument::Ruled { rules, value } => {
				if super::rules::validate_rules(rules, java_arch, true) {
					match value {
						ArgumentValue::Single(arg) => {
							parsed.push(parse_function(&arg.replace(' ', DUMMY_REPLACE_NEWLINE))?);
						}
						ArgumentValue::Many(args) => {
							for arg in args {
								parsed.push(parse_function(
									&arg.replace(' ', DUMMY_REPLACE_NEWLINE),
								)?);
							}
						}
					}
				}
			}
		}
	}

	Ok(())
}
