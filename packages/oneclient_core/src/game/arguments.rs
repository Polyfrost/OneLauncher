use std::collections::{HashMap, HashSet};
use std::path::Path;

use interfrost::api::minecraft::{Argument, ArgumentValue, Library, VersionType};
use interfrost::api::modded::SidedDataEntry;
use interfrost::utils::get_path_from_artifact;

use crate::constants::{self, DUMMY_REPLACE_NEWLINE};
use crate::game::rules::validate_rules;
use crate::game::GameError;
use crate::settings::Resolution;
use crate::LauncherResult;

#[allow(clippy::too_many_arguments)]
pub fn java_arguments(
    version_updated: bool,
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
            version_updated,
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
            polyio::canonicalize(natives_path)
                .map_err(|_| GameError::LibraryPath(natives_path.display().to_string()))?
                .display()
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
    version_updated: bool,
    args: Option<&[Argument]>,
    legacy_args: Option<&str>,
    access_token: &str,
    username: &str,
    uuid: uuid::Uuid,
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
            version_updated,
            args,
            &mut parsed,
            |arg| {
                parse_minecraft_argument(
                    arg,
                    access_token,
                    username,
                    uuid,
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
                access_token,
                username,
                uuid,
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

pub fn append_profile_game_arguments(
    args: &mut Vec<String>,
    force_fullscreen: Option<bool>,
    launch_args: Option<&str>,
) {
    if force_fullscreen.unwrap_or(false) {
        args.push("--fullscreen".to_string());
    }

    if let Some(extra) = launch_args.map(str::trim).filter(|s| !s.is_empty()) {
        args.push(extra.to_string());
    }
}

pub fn processor_arguments<T: AsRef<str>, S: std::hash::BuildHasher>(
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

#[tracing::instrument(skip_all, level = "debug")]
pub async fn main_class(path: impl AsRef<std::path::Path>) -> LauncherResult<Option<String>> {
    let data = polyio::read(path.as_ref()).await?;
    let mut class_name = None;

    use futures_util::TryStreamExt;

    let stream = polyio::stream_zip_entries_bytes(data);
    let mut stream = std::pin::pin!(stream);

    while let Some(item) = stream.try_next().await? {
        let (index, entry, reader) = item;
        if entry.dir().map_err(polyio::IOError::from)? {
            continue;
        }

        if entry.filename().as_str().map_err(polyio::IOError::from)? != "META-INF/MANIFEST.MF" {
            continue;
        }

        let mut buf = String::new();
        let mut entry_reader = reader
            .reader_without_entry(index)
            .await
            .map_err(polyio::IOError::from)?;
        futures_util::AsyncReadExt::read_to_string(&mut entry_reader, &mut buf)
            .await
            .map_err(polyio::IOError::from)?;

        for line in buf.lines() {
            let line = line.trim();
            if line.starts_with("Main-Class:")
                && let Some(class) = line.split(':').nth(1)
            {
                class_name = Some(class.trim().to_string());
                break;
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
    Ok(argument
        .replace("${accessToken}", access_token)
        .replace("${auth_access_token}", access_token)
        .replace("${auth_session}", access_token)
        .replace("${auth_player_name}", username)
        .replace("${auth_xuid}", "0")
        .replace("${auth_uuid}", &uuid.simple().to_string())
        .replace("${uuid}", &uuid.simple().to_string())
        .replace("${clientid}", crate::constants::MICROSOFT_CLIENT_ID)
        .replace("${user_properties}", "{}")
        .replace("${user_type}", "msa")
        .replace("${version_name}", version)
        .replace("${assets_index_name}", asset_index)
        .replace(
            "${game_directory}",
            &polyio::canonicalize(game_directory)?.display().to_string(),
        )
        .replace(
            "${assets_root}",
            &polyio::canonicalize(assets_directory)?.display().to_string(),
        )
        .replace(
            "${game_assets}",
            &polyio::canonicalize(assets_directory)?.display().to_string(),
        )
        .replace("${version_type}", version_type.as_str())
        .replace("${resolution_width}", &resolution.width.to_string())
        .replace("${resolution_height}", &resolution.height.to_string()))
}

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
            &polyio::canonicalize(natives_path)?.display().to_string(),
        )
        .replace(
            "${library_directory}",
            &polyio::canonicalize(libraries_path)?.display().to_string(),
        )
        .replace("${classpath_separator}", constants::CLASSPATH_SEPARATOR)
        .replace("${launcher_name}", "OneClient")
        .replace("${launcher_version}", env!("CARGO_PKG_VERSION"))
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
    let mut chosen: HashMap<String, (Vec<u64>, &str)> = HashMap::new();
    for lib in libraries {
        if let Some(rules) = &lib.rules
            && !validate_rules(rules, java_arch, updated)
        {
            continue;
        }
        if !lib.include_in_classpath {
            continue;
        }

        let (artifact, version) = split_artifact_version(&lib.name);
        let ver_key = version_key(version);
        match chosen.get(&artifact) {
            Some((existing, existing_name)) if *existing >= ver_key => {
                tracing::debug!(
                    skipped = %lib.name,
                    kept = %existing_name,
                    "classpath: dropping older duplicate library"
                );
            }
            _ => {
                chosen.insert(artifact, (ver_key, &lib.name));
            }
        }
    }

    let mut classpaths = chosen
        .values()
        .map(|(_, name)| get_library(libraries_path, name, false))
        .collect::<Result<HashSet<_>, _>>()?;

    classpaths.insert(
        polyio::canonicalize(client_path)?
            .display()
            .to_string(),
    );

    tracing::debug!(entries = classpaths.len(), "classpath resolved");

    Ok(classpaths
        .into_iter()
        .collect::<Vec<_>>()
        .join(constants::CLASSPATH_SEPARATOR))
}

fn split_artifact_version(name: &str) -> (String, &str) {
    match name.rsplit_once(':') {
        Some((key, version)) if version.starts_with(|c: char| c.is_ascii_digit()) => {
            (key.to_string(), version)
        }
        _ => (name.to_string(), ""),
    }
}

fn version_key(version: &str) -> Vec<u64> {
    version
        .split(['.', '-', '+', '_'])
        .map(|seg| seg.parse::<u64>().unwrap_or(0))
        .collect()
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
    path.push(get_path_from_artifact(library).map_err(|_| {
        GameError::LibraryPath(library.to_string())
    })?);

    if !path.exists() && error_exist {
        return Ok(path.display().to_string());
    }

    Ok(polyio::canonicalize(&path)?.display().to_string())
}

fn parse_arguments<ParseFn>(
    version_updated: bool,
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
                if validate_rules(rules, java_arch, version_updated) {
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
