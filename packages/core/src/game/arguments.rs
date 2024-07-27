//! Programatic Minecraft argument parser which replaces variables in arguments

use std::collections::{HashMap, HashSet};
use std::io::prelude::BufRead;
use std::io::BufReader;
use std::path::Path;

use interpulse::api::minecraft::{Argument, ArgumentValue, Library, VersionType};
use interpulse::api::modded::SidedDataEntry;
use interpulse::utils::get_path_from_artifact;

use crate::constants::DUMMY_REPLACE_NEWLINE;
use crate::store::{Memory, MinecraftCredentials, Resolution};
use crate::utils::io::IOError;
use crate::utils::platform::classpath_separator;

#[allow(clippy::too_many_arguments)]
pub fn java_arguments(
	arguments: Option<&[Argument]>,
	natives_path: &Path,
	libraries_path: &Path,
	classpaths: &str,
	version: &str,
	memory: Memory,
	custom_args: Vec<String>,
	java_arch: &str,
) -> crate::Result<Vec<String>> {
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
					java_arch,
				)
			},
			java_arch,
		)?;
	} else {
		parsed.push(format!(
			"-Djava.library.path={}",
			dunce::canonicalize(natives_path)
				.map_err(|_| anyhow::anyhow!(
					"specified natives path {} not found",
					natives_path.to_string_lossy()
				))?
				.to_string_lossy()
		));
		parsed.push("-cp".to_string());
		parsed.push(classpaths.to_string());
	}

	parsed.push(format!("-Xmx{}M", memory.maximum));
	parsed.push(format!("-Xms{}M", memory.minimum));

	for arg in custom_args {
		if !arg.is_empty() {
			parsed.push(arg);
		}
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
	version_type: &VersionType,
	resolution: Resolution,
	java_arch: &str,
) -> crate::Result<Vec<String>> {
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

pub fn processor_arguments<T: AsRef<str>>(
	libraries_path: &Path,
	args: &[T],
	data: &HashMap<String, SidedDataEntry>,
) -> crate::Result<Vec<String>> {
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
				})
			}
		} else if arg.as_ref().starts_with('[') {
			parsed.push(get_library(libraries_path, a, true)?)
		} else {
			parsed.push(arg.as_ref().to_string())
		}
	}

	Ok(parsed)
}

pub async fn main_class(path: String) -> crate::Result<Option<String>> {
	let main_class = tokio::task::spawn_blocking(move || {
		let zipfs = std::fs::File::open(&path).map_err(|e| IOError::with_path(e, &path))?;
		let mut archive = zip::ZipArchive::new(zipfs)
			.map_err(|_| anyhow::anyhow!("failed to read main class {}", path))?;
		let file = archive
			.by_name("META-INF/MANIFEST.MF")
			.map_err(|_| anyhow::anyhow!("failed to read manifest {}", path))?;

		let zip_reader = BufReader::new(file);
		for line in zip_reader.lines() {
			let mut line = line.map_err(IOError::from)?;
			line.retain(|c| !c.is_whitespace());

			if line.starts_with("Main-Class:") {
				if let Some(class) = line.split(':').nth(1) {
					return Ok(Some(class.to_string()));
				}
			}
		}

		Ok::<Option<String>, crate::Error>(None)
	})
	.await??;

	Ok(main_class)
}

#[allow(clippy::too_many_arguments)]
fn parse_minecraft_argument(
	argument: &str,
	access_token: &str,
	username: &str,
	uuid: uuid::Uuid,
	version: &str,
	asset_index: &str,
	game_directory: &Path,
	assets_directory: &Path,
	version_type: &VersionType,
	resolution: Resolution,
) -> crate::Result<String> {
	Ok(argument
		.replace("${accessToken}", access_token)
		.replace("${auth_access_token}", access_token)
		.replace("${auth_session}", access_token)
		.replace("${auth_player_name}", username)
		.replace("${auth_xuid}", "10")
		.replace("${auth_uuid}", &uuid.simple().to_string())
		.replace("${uuid}", &uuid.simple().to_string())
		.replace("${clientid}", crate::constants::MICROSOFT_CLIENT_ID)
		.replace("${user_properties}", "{}")
		.replace("${user_type}", "msa")
		.replace("${version_name}", version)
		.replace("${assets_index_name}", asset_index)
		.replace(
			"${game_directory}",
			&dunce::canonicalize(game_directory)
				.map_err(|_| {
					anyhow::anyhow!(
						"game directory {} doesn't exist",
						game_directory.to_string_lossy()
					)
				})?
				.to_string_lossy(),
		)
		.replace(
			"${assets_root}",
			&dunce::canonicalize(assets_directory)
				.map_err(|_| {
					anyhow::anyhow!(
						"assets directory {} doesn't exist",
						assets_directory.to_string_lossy()
					)
				})?
				.to_string_lossy(),
		)
		.replace(
			"${game_assets}",
			&dunce::canonicalize(assets_directory)
				.map_err(|_| {
					anyhow::anyhow!(
						"assets directory {} doesn't exist",
						assets_directory.to_string_lossy()
					)
				})?
				.to_string_lossy(),
		)
		.replace("${version_type}", version_type.as_str())
		.replace("${resolution_width}", &resolution.0.to_string())
		.replace("${resolution_height}", &resolution.1.to_string()))
}

#[allow(clippy::too_many_arguments)]
fn parse_java_argument(
	mut argument: String,
	natives_path: &Path,
	libraries_path: &Path,
	classpaths: &str,
	version: &str,
	java_arch: &str,
) -> crate::Result<String> {
	argument.retain(|c| !c.is_whitespace());
	Ok(argument
		.replace(
			"${natives_directory}",
			&dunce::canonicalize(natives_path)
				.map_err(|_| {
					anyhow::anyhow!(
						"natives path {} doesn't exist",
						natives_path.to_string_lossy()
					)
				})?
				.to_string_lossy(),
		)
		.replace(
			"${library_directory}",
			&dunce::canonicalize(libraries_path)
				.map_err(|_| {
					anyhow::anyhow!(
						"libraries path {} doesn't eixst",
						libraries_path.to_string_lossy()
					)
				})?
				.to_string_lossy(),
		)
		.replace("${classpath_seperator}", classpath_separator(java_arch))
		.replace("${launcher_name}", crate::constants::NAME)
		.replace("${launcher_version}", crate::constants::VERSION)
		.replace("${version_name}", version)
		.replace("${classpath}", classpaths))
}

pub fn classpaths(
	libraries_path: &Path,
	libraries: &[Library],
	client_path: &Path,
	java_arch: &str,
	updated: bool,
) -> crate::Result<String> {
	let mut classpaths = libraries
		.iter()
		.filter_map(|lib| {
			if let Some(rules) = &lib.rules {
				if !crate::game::rules(rules, java_arch, updated) {
					return None;
				}
			}

			if !lib.include_in_classpath {
				return None;
			}

			Some(get_library(libraries_path, &lib.name, false))
		})
		.collect::<Result<HashSet<_>, _>>()?;

	classpaths.insert(
		dunce::canonicalize(client_path)
			.map_err(|_| {
				anyhow::anyhow!(
					"specified classpath {} not found",
					client_path.to_string_lossy()
				)
			})?
			.to_string_lossy()
			.to_string(),
	);

	Ok(classpaths
		.into_iter()
		.collect::<Vec<_>>()
		.join(classpath_separator(java_arch)))
}

pub fn get_classpath_library<T: AsRef<str>>(
	libraries_path: &Path,
	libraries: &[T],
	java_arch: &str,
) -> crate::Result<String> {
	let classpaths = libraries
		.iter()
		.map(|lib| get_library(libraries_path, lib.as_ref(), false))
		.collect::<Result<Vec<_>, _>>()?;

	Ok(classpaths.join(classpath_separator(java_arch)))
}

pub fn get_library(
	libraries_path: &Path,
	library: &str,
	error_exist: bool,
) -> crate::Result<String> {
	let mut path = libraries_path.to_path_buf();
	path.push(get_path_from_artifact(library)?);

	if !path.exists() && error_exist {
		return Ok(path.to_string_lossy().to_string());
	}

	let path = &dunce::canonicalize(&path)
		.map_err(|_| anyhow::anyhow!("library file {} not found", path.to_string_lossy()))?;

	Ok(path.to_string_lossy().to_string())
}

fn parse_arguments<ParseFn>(
	args: &[Argument],
	parsed: &mut Vec<String>,
	parse_function: ParseFn,
	java_arch: &str,
) -> crate::Result<()>
where
	ParseFn: Fn(&str) -> crate::Result<String>,
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
				if super::rules(rules, java_arch, true) {
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
