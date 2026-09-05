use std::collections::{HashMap, HashSet};
use std::path::Path;

use interfrost::api::minecraft::{Argument, ArgumentValue, Library, VersionType};
use interfrost::api::modded::SidedDataEntry;
use interfrost::utils::get_path_from_artifact;

use oneclient_common::constants::{self, DUMMY_REPLACE_NEWLINE};
use crate::rules::validate_rules;
use crate::error::McError;
use oneclient_common::Resolution;
use crate::error::McResult;

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
    java_major: u32,
) -> McResult<Vec<String>> {
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
                .map_err(|_| McError::LibraryPath(natives_path.display().to_string()))?
                .display()
        ));
        parsed.push("-cp".to_string());
        parsed.push(classpaths.to_string());
    }

    let custom = split_custom_args(&custom_args);

    parsed.extend(performance_flags(java_major, java_arch, mem_max, &custom));

    parsed.push(format!("-Xmx{mem_max}M"));
    parsed.extend(custom);

    Ok(parsed)
}

const ZGC_MIN_HEAP_MB: u32 = 8192;

const INITIAL_HEAP_MB: u32 = 512;

#[must_use]
pub fn performance_flags(
    java_major: u32,
    java_arch: &str,
    mem_max: u32,
    custom_args: &[String],
) -> Vec<String> {
    let mut flags = vec![format!("-Xms{}M", mem_max.min(INITIAL_HEAP_MB))];

    let collector_chosen = custom_args.iter().any(|arg| is_collector_flag(arg));

    let use_zgc =
        !collector_chosen && java_major >= 21 && is_64_bit(java_arch) && mem_max >= ZGC_MIN_HEAP_MB;
    let use_g1 = !collector_chosen && !use_zgc && java_major >= 8;

    if (use_zgc || use_g1) && java_major == 24 {
        flags.push("-XX:+UnlockExperimentalVMOptions".to_string());
    }

    if use_zgc {
        flags.push("-XX:+UseZGC".to_string());
        if java_major <= 22 {
            flags.push("-XX:+ZGenerational".to_string());
        }
    } else if use_g1 {
        flags.push("-XX:+UseG1GC".to_string());
        flags.push("-XX:+ParallelRefProcEnabled".to_string());
    }

    if (use_zgc || use_g1) && java_major >= 24 && is_64_bit(java_arch) {
        flags.push("-XX:+UseCompactObjectHeaders".to_string());
    }

    flags
}

fn is_collector_flag(arg: &str) -> bool {
    const COLLECTORS: [&str; 8] = [
        "UseG1GC",
        "UseZGC",
        "UseShenandoahGC",
        "UseParallelGC",
        "UseParallelOldGC",
        "UseConcMarkSweepGC",
        "UseSerialGC",
        "UseEpsilonGC",
    ];

    arg.starts_with("-XX:") && COLLECTORS.iter().any(|name| arg.contains(name))
}

fn is_64_bit(java_arch: &str) -> bool {
    matches!(
        java_arch,
        "amd64" | "x86_64" | "x64" | "aarch64" | "arm64" | "ppc64le" | "s390x" | "riscv64"
    )
}

/// Whitespace-separated double quotes for values containing spaces
/// Must not be passed as one argv entry or the JVM sees one unrecognised option
fn split_custom_args(raw: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut started = false;

    for ch in raw.chars() {
        match ch {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    args.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            c => {
                current.push(c);
                started = true;
            }
        }
    }

    if started {
        args.push(current);
    }

    args
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
) -> McResult<Vec<String>> {
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
) -> McResult<Vec<String>> {
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
pub async fn main_class(path: impl AsRef<std::path::Path>) -> McResult<Option<String>> {
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
) -> McResult<String> {
    Ok(argument
        .replace("${accessToken}", access_token)
        .replace("${auth_access_token}", access_token)
        .replace("${auth_session}", access_token)
        .replace("${auth_player_name}", username)
        .replace("${auth_xuid}", "0")
        .replace("${auth_uuid}", &uuid.simple().to_string())
        .replace("${uuid}", &uuid.simple().to_string())
        .replace("${clientid}", oneclient_common::constants::MICROSOFT_CLIENT_ID)
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
) -> McResult<String> {
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
) -> McResult<String> {
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
) -> McResult<String> {
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
) -> McResult<String> {
    let mut path = libraries_path.to_path_buf();
    path.push(get_path_from_artifact(library).map_err(|_| {
        McError::LibraryPath(library.to_string())
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
) -> McResult<()>
where
    ParseFn: Fn(&str) -> McResult<String>,
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

#[cfg(test)]
mod tests {
    use super::{ZGC_MIN_HEAP_MB, is_collector_flag, performance_flags, split_custom_args};

    #[test]
    fn blank_input_contributes_nothing() {
        assert!(split_custom_args("").is_empty());
        assert!(split_custom_args("   \t ").is_empty());
    }

    #[test]
    fn each_flag_becomes_its_own_argument() {
        assert_eq!(
            split_custom_args("-XX:+UseG1GC  -Xms2G"),
            vec!["-XX:+UseG1GC", "-Xms2G"]
        );
    }

    #[test]
    fn quotes_hold_a_value_with_spaces_together() {
        assert_eq!(
            split_custom_args(r#"-Dname="My Server" -Xss1M"#),
            vec!["-Dname=My Server", "-Xss1M"]
        );
    }

    #[test]
    fn an_empty_quoted_value_survives() {
        assert_eq!(split_custom_args(r#"-Dempty="""#), vec!["-Dempty="]);
    }

    fn flags(java_major: u32, java_arch: &str, mem_max: u32) -> Vec<String> {
        performance_flags(java_major, java_arch, mem_max, &[])
    }

    fn custom(args: &str) -> Vec<String> {
        split_custom_args(args)
    }

    #[test]
    fn a_java_8_runtime_gets_g1() {
        let j8 = flags(8, "amd64", 4096);
        assert_eq!(j8.first().unwrap(), "-Xms512M");
        assert!(j8.contains(&"-XX:+UseG1GC".to_string()));
    }

    #[test]
    fn the_versions_told_to_upgrade_still_get_a_tuned_collector() {
        for major in [9, 11, 17, 20] {
            let flags = flags(major, "amd64", 4096);
            assert!(
                flags.contains(&"-XX:+UseG1GC".to_string()),
                "java {major} should still be tuned, it is what old packs run on"
            );
            assert!(
                !flags.iter().any(|f| f.contains("UseZGC")),
                "java {major} predates the generational ZGC we would want"
            );
        }
    }

    #[test]
    fn a_32_bit_runtime_never_gets_zgc() {
        for arch in ["x86", "arm", "i386", "unknown"] {
            let flags = flags(25, arch, 16384);
            assert!(
                !flags.iter().any(|f| f.contains("UseZGC")),
                "{arch} must not select a collector it cannot load"
            );
            assert!(
                !flags.contains(&"-XX:+UseCompactObjectHeaders".to_string()),
                "{arch} has no compact headers to enable"
            );
            assert!(flags.contains(&"-XX:+UseG1GC".to_string()));
        }
    }

    #[test]
    fn a_small_heap_gets_g1_rather_than_zgc() {
        for mem in [2048, 4096, ZGC_MIN_HEAP_MB - 1] {
            let flags = flags(21, "amd64", mem);
            assert!(
                !flags.iter().any(|f| f.contains("UseZGC")),
                "{mem}M is not enough heap for ZGC to stay ahead of a modded client"
            );
            assert!(flags.contains(&"-XX:+UseG1GC".to_string()));
        }
    }

    #[test]
    fn a_large_heap_still_gets_zgc() {
        for mem in [ZGC_MIN_HEAP_MB, 12288, 16384] {
            let flags = flags(21, "amd64", mem);
            assert!(flags.contains(&"-XX:+UseZGC".to_string()));
            assert!(
                !flags.contains(&"-XX:+UseG1GC".to_string()),
                "only one collector may be selected"
            );
        }
    }

    #[test]
    fn the_collector_is_selected_but_not_sized() {
        for mem in [2048, 4096, 8192, 16384] {
            let flags = flags(17, "amd64", mem);
            assert_eq!(
                flags,
                vec![
                    "-Xms512M".to_string(),
                    "-XX:+UseG1GC".to_string(),
                    "-XX:+ParallelRefProcEnabled".to_string(),
                ],
                "{mem}M should get no hand-picked sizing"
            );
        }
    }

    #[test]
    fn a_collector_in_the_custom_arguments_replaces_ours() {
        for arg in [
            "-XX:+UseG1GC",
            "-XX:+UseZGC",
            "-XX:+UseSerialGC",
            "-XX:+UseShenandoahGC",
            "-XX:-UseG1GC",
        ] {
            let flags = performance_flags(21, "amd64", 16384, &custom(arg));
            assert!(
                !flags.iter().any(|f| is_collector_flag(f)),
                "{arg} should leave the collector entirely to the user"
            );
            assert!(
                !flags.iter().any(|f| f.starts_with("-XX:G1")),
                "{arg} should not be tuned for a collector we did not select"
            );
            assert_eq!(
                flags,
                vec!["-Xms512M"],
                "{arg} should leave only the collector-independent flags"
            );
        }
    }

    #[test]
    fn unrelated_custom_arguments_leave_the_collector_alone() {
        let flags = performance_flags(21, "amd64", 4096, &custom("-Dfoo=bar -Xss1M"));
        assert!(flags.contains(&"-XX:+UseG1GC".to_string()));
    }

    #[test]
    fn zgc_is_asked_for_a_young_generation_only_before_23() {
        assert!(flags(21, "amd64", 16384).contains(&"-XX:+ZGenerational".to_string()));
        assert!(flags(22, "aarch64", 16384).contains(&"-XX:+ZGenerational".to_string()));
        assert!(!flags(23, "amd64", 16384).contains(&"-XX:+ZGenerational".to_string()));
        assert!(!flags(25, "amd64", 16384).contains(&"-XX:+ZGenerational".to_string()));
    }

    #[test]
    fn compact_headers_arrive_on_24_whichever_collector_is_chosen() {
        for major in [21, 23] {
            assert!(
                !flags(major, "amd64", 16384).contains(&"-XX:+UseCompactObjectHeaders".to_string()),
                "java {major} predates compact object headers"
            );
        }

        for mem in [4096, 16384] {
            let j24 = flags(24, "amd64", mem);
            let unlock = j24
                .iter()
                .position(|f| f == "-XX:+UnlockExperimentalVMOptions")
                .expect("24 needs the unlock flag");
            let compact = j24
                .iter()
                .position(|f| f == "-XX:+UseCompactObjectHeaders")
                .expect("24 supports compact headers");
            assert!(
                unlock < compact,
                "the unlock flag must precede the option it unlocks"
            );
        }

        let j25_zgc = flags(25, "amd64", 16384);
        assert!(j25_zgc.contains(&"-XX:+UseCompactObjectHeaders".to_string()));
        assert!(!j25_zgc.contains(&"-XX:+UnlockExperimentalVMOptions".to_string()));

        assert!(flags(25, "amd64", 4096).contains(&"-XX:+UseCompactObjectHeaders".to_string()));
    }

    #[test]
    fn every_experimental_option_sits_after_the_unlock() {
        const EXPERIMENTAL: [&str; 1] = ["-XX:+UseCompactObjectHeaders"];

        for major in [8, 17, 21, 24, 25] {
            for mem in [4096, 16384] {
                let flags = flags(major, "amd64", mem);
                let Some(unlock) = flags
                    .iter()
                    .position(|f| f == "-XX:+UnlockExperimentalVMOptions")
                else {
                    assert!(
                        !flags
                            .iter()
                            .any(|f| EXPERIMENTAL.iter().any(|e| f.starts_with(e))
                                && !(major >= 25 && f == "-XX:+UseCompactObjectHeaders")),
                        "java {major} at {mem}M emits an experimental option with no unlock"
                    );
                    continue;
                };

                for (i, flag) in flags.iter().enumerate() {
                    if EXPERIMENTAL.iter().any(|e| flag.starts_with(e)) {
                        assert!(
                            unlock < i,
                            "java {major} at {mem}M puts {flag} before the unlock"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn string_deduplication_is_never_added() {
        for major in [7, 8, 17, 21, 24, 25] {
            assert!(
                !flags(major, "amd64", 4096).contains(&"-XX:+UseStringDeduplication".to_string()),
                "java {major} should not be told to deduplicate strings"
            );
        }
    }

    #[test]
    fn a_modern_runtime_gets_the_full_set() {
        assert_eq!(
            flags(25, "aarch64", 16384),
            vec![
                "-Xms512M",
                "-XX:+UseZGC",
                "-XX:+UseCompactObjectHeaders"
            ]
        );
    }

    #[test]
    fn every_tier_starts_small_and_grows_into_the_memory_setting() {
        for major in [8, 17, 21, 25] {
            assert_eq!(
                flags(major, "amd64", 8192).first().unwrap(),
                "-Xms512M",
                "java {major} should not commit the ceiling up front"
            );
        }
    }

    #[test]
    fn a_tiny_profile_never_starts_above_its_ceiling() {
        assert_eq!(
            flags(21, "amd64", 256),
            vec![
                "-Xms256M",
                "-XX:+UseG1GC",
                "-XX:+ParallelRefProcEnabled",
            ],
            "a start size above the maximum is refused by the JVM outright"
        );
    }

    #[test]
    fn a_java_7_runtime_is_left_untouched() {
        assert_eq!(flags(7, "amd64", 4096), vec!["-Xms512M"]);
    }
}

