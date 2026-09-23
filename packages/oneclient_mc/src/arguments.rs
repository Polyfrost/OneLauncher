use std::collections::{HashMap, HashSet};
use std::path::Path;

use interfrost::api::minecraft::{Argument, ArgumentValue, Library, VersionType};
use interfrost::api::modded::SidedDataEntry;
use interfrost::utils::get_path_from_artifact;

use crate::error::McError;
use crate::error::McResult;
use crate::rules::validate_rules;
use oneclient_common::Resolution;
use oneclient_common::constants::{self, DUMMY_REPLACE_NEWLINE};
use oneclient_common::paths;

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
    }

    if !parsed
        .iter()
        .any(|arg| arg.starts_with("-Djava.library.path="))
    {
        parsed.push(format!(
            "-Djava.library.path={}",
            polyio::canonicalize(natives_path)
                .map_err(|_| McError::LibraryPath(natives_path.display().to_string()))?
                .display()
        ));
    }
    if !parsed.iter().any(|arg| arg == "-cp" || arg == "-classpath") {
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
    let mut parsed = Vec::new();

    // a legacy version states its arguments as a string and a loader merged onto
    // one adds a list on top the game needs both, in that order
    if let Some(legacy_args) = legacy_args {
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
    }

    if let Some(args) = args {
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
    }

    drop_repeated_arguments(&mut parsed);

    Ok(parsed)
}

fn drop_repeated_arguments(args: &mut Vec<String>) {
    let mut seen: HashSet<(String, Option<String>)> = HashSet::new();
    let mut kept: Vec<String> = Vec::with_capacity(args.len());
    let mut index = 0;

    while index < args.len() {
        let token = &args[index];
        if !token.starts_with("--") {
            kept.push(token.clone());
            index += 1;
            continue;
        }

        let value = args
            .get(index + 1)
            .filter(|next| !next.starts_with("--"))
            .cloned();

        if seen.insert((token.clone(), value.clone())) {
            kept.push(token.clone());
            if let Some(value) = &value {
                kept.push(value.clone());
            }
        }

        index += if value.is_some() { 2 } else { 1 };
    }

    *args = kept;
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
        let arg = arg.as_ref();

        if let Some(coordinate) = arg.strip_prefix('[').and_then(|a| a.strip_suffix(']')) {
            parsed.push(get_library(libraries_path, coordinate, true)?);
            continue;
        }

        parsed.push(expand_data_placeholders(libraries_path, arg, data)?);
    }

    Ok(parsed)
}

fn expand_data_placeholders<S: std::hash::BuildHasher>(
    libraries_path: &Path,
    arg: &str,
    data: &HashMap<String, SidedDataEntry, S>,
) -> McResult<String> {
    let mut out = String::with_capacity(arg.len());
    let mut rest = arg;

    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}').map(|at| open + at) else {
            break;
        };

        out.push_str(&rest[..open]);
        let key = &rest[open + 1..close];

        match data.get(key) {
            Some(entry) => {
                match entry
                    .client
                    .strip_prefix('[')
                    .and_then(|c| c.strip_suffix(']'))
                {
                    Some(coordinate) => {
                        out.push_str(&get_library(libraries_path, coordinate, true)?);
                    }
                    None => out.push_str(&entry.client),
                }
            }
            None => out.push_str(&rest[open..=close]),
        }

        rest = &rest[close + 1..];
    }

    out.push_str(rest);
    Ok(out)
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
        .replace(
            "${clientid}",
            oneclient_common::constants::MICROSOFT_CLIENT_ID,
        )
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
            &polyio::canonicalize(assets_directory)?
                .display()
                .to_string(),
        )
        .replace("${game_assets}", &legacy_assets_path()?)
        .replace("${version_type}", version_type.as_str())
        .replace("${resolution_width}", &resolution.width.to_string())
        .replace("${resolution_height}", &resolution.height.to_string()))
}

fn legacy_assets_path() -> McResult<String> {
    let dir = paths::legacy_assets_dir()?;

    Ok(polyio::canonicalize(&dir)
        .unwrap_or(dir)
        .display()
        .to_string())
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
    let mut chosen: HashMap<String, (ClasspathRank, &str)> = HashMap::new();
    for lib in libraries {
        if let Some(rules) = &lib.rules
            && !validate_rules(rules, java_arch, updated)
        {
            continue;
        }
        if !lib.include_in_classpath {
            continue;
        }
        // natives-only libraries (classifiers, no artifact) have no jar on disk
        if !crate::install::has_main_artifact(lib) {
            continue;
        }

        let (artifact, version) = split_artifact_version(&lib.name);
        let bundled = is_legacy_asm_bundle(&artifact);
        let slot = classpath_slot(artifact);
        let rank = (!bundled, version_key(version));

        match chosen.get(&slot) {
            Some((existing, existing_name)) if *existing >= rank => {
                tracing::debug!(
                    skipped = %lib.name,
                    kept = %existing_name,
                    "classpath: dropping superseded library"
                );
            }
            _ => {
                chosen.insert(slot, (rank, &lib.name));
            }
        }
    }

    let mut classpaths = chosen
        .values()
        .map(|(_, name)| get_library(libraries_path, name, false))
        .collect::<Result<HashSet<_>, _>>()?;

    classpaths.insert(polyio::canonicalize(client_path)?.display().to_string());

    tracing::debug!(entries = classpaths.len(), "classpath resolved");

    Ok(classpaths
        .into_iter()
        .collect::<Vec<_>>()
        .join(constants::CLASSPATH_SEPARATOR))
}

type ClasspathRank = (bool, Vec<u64>);

const ASM_SLOT: &str = "org.ow2.asm:asm";

fn is_legacy_asm_bundle(artifact: &str) -> bool {
    matches!(
        artifact,
        "org.ow2.asm:asm-all" | "org.ow2.asm:asm-debug-all" | "asm:asm-all" | "asm:asm-debug-all"
    )
}

fn classpath_slot(artifact: String) -> String {
    if is_legacy_asm_bundle(&artifact) {
        return ASM_SLOT.to_string();
    }

    artifact
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

pub fn get_library(libraries_path: &Path, library: &str, allow_missing: bool) -> McResult<String> {
    let mut path = libraries_path.to_path_buf();
    path.push(
        get_path_from_artifact(library).map_err(|_| McError::LibraryPath(library.to_string()))?,
    );

    if !path.exists() {
        if allow_missing {
            return Ok(path.display().to_string());
        }

        return Err(McError::MissingLibrary {
            library: library.to_string(),
            path: path.display().to_string(),
        });
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
    use super::{
        HashMap, Library, Path, SidedDataEntry, ZGC_MIN_HEAP_MB, classpaths,
        drop_repeated_arguments, get_library, is_collector_flag, java_arguments,
        minecraft_arguments, performance_flags, processor_arguments, split_custom_args,
    };
    use oneclient_common::Resolution;

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
            vec!["-Xms512M", "-XX:+UseZGC", "-XX:+UseCompactObjectHeaders"]
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
            vec!["-Xms256M", "-XX:+UseG1GC", "-XX:+ParallelRefProcEnabled",],
            "a start size above the maximum is refused by the JVM outright"
        );
    }

    #[test]
    fn a_java_7_runtime_is_left_untouched() {
        assert_eq!(flags(7, "amd64", 4096), vec!["-Xms512M"]);
    }

    fn install_jars(tag: &str, jars: &[&str]) -> (std::path::PathBuf, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(tag);
        for jar in jars {
            let path = dir.join(jar);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"").unwrap();
        }
        let client = dir.join("client.jar");
        std::fs::write(&client, b"").unwrap();
        (dir, client)
    }

    #[test]
    fn a_legacy_asm_bundle_loses_to_the_split_modules() {
        let libraries: Vec<Library> = serde_json::from_str(
            r#"[
                { "name": "org.ow2.asm:asm-all:4.1" },
                { "name": "org.ow2.asm:asm:9.10.1" },
                { "name": "org.ow2.asm:asm-tree:9.10.1" },
                { "name": "net.minecraft:launchwrapper:1.5" }
            ]"#,
        )
        .unwrap();

        let (dir, client) = install_jars(
            "oneclient-classpath-asm-all",
            &[
                "org/ow2/asm/asm-all/4.1/asm-all-4.1.jar",
                "org/ow2/asm/asm/9.10.1/asm-9.10.1.jar",
                "org/ow2/asm/asm-tree/9.10.1/asm-tree-9.10.1.jar",
                "net/minecraft/launchwrapper/1.5/launchwrapper-1.5.jar",
            ],
        );

        let cp = classpaths(&dir, &libraries, &client, "x86", false).unwrap();

        assert!(
            !cp.contains("asm-all"),
            "fabric aborts on two copies of org/objectweb/asm/ClassReader.class: {cp}"
        );
        assert!(cp.contains("asm-9.10.1.jar"), "{cp}");
        assert!(cp.contains("asm-tree-9.10.1.jar"), "{cp}");
        assert!(cp.contains("launchwrapper-1.5.jar"), "{cp}");
    }

    #[test]
    fn a_legacy_asm_bundle_stays_when_nothing_supersedes_it() {
        let libraries: Vec<Library> = serde_json::from_str(
            r#"[
                { "name": "org.ow2.asm:asm-all:4.1" },
                { "name": "net.minecraft:launchwrapper:1.5" }
            ]"#,
        )
        .unwrap();

        let (dir, client) = install_jars(
            "oneclient-classpath-asm-only",
            &[
                "org/ow2/asm/asm-all/4.1/asm-all-4.1.jar",
                "net/minecraft/launchwrapper/1.5/launchwrapper-1.5.jar",
            ],
        );

        let cp = classpaths(&dir, &libraries, &client, "x86", false).unwrap();

        assert!(cp.contains("asm-all-4.1.jar"), "{cp}");
    }

    #[test]
    fn a_newer_bundle_does_not_outrank_an_older_split_module() {
        let libraries: Vec<Library> = serde_json::from_str(
            r#"[
                { "name": "org.ow2.asm:asm-debug-all:5.2" },
                { "name": "org.ow2.asm:asm:5.0.3" }
            ]"#,
        )
        .unwrap();

        let (dir, client) = install_jars(
            "oneclient-classpath-asm-debug",
            &[
                "org/ow2/asm/asm-debug-all/5.2/asm-debug-all-5.2.jar",
                "org/ow2/asm/asm/5.0.3/asm-5.0.3.jar",
            ],
        );

        let cp = classpaths(&dir, &libraries, &client, "x86", false).unwrap();

        assert!(!cp.contains("asm-debug-all"), "{cp}");
        assert!(cp.contains("asm-5.0.3.jar"), "{cp}");
    }

    #[test]
    fn a_missing_classpath_jar_names_the_library_it_could_not_find() {
        let libraries: Vec<Library> = serde_json::from_str(
            r#"[
                { "name": "net.fabricmc:fabric-loader:0.19.5" },
                { "name": "net.fabricmc:intermediary:1.8.9" }
            ]"#,
        )
        .unwrap();

        let dir = std::env::temp_dir().join("oneclient-classpath-missing");
        let present = dir.join("net/fabricmc/fabric-loader/0.19.5/fabric-loader-0.19.5.jar");
        std::fs::create_dir_all(present.parent().unwrap()).unwrap();
        std::fs::write(&present, b"").unwrap();
        let client = dir.join("client.jar");
        std::fs::write(&client, b"").unwrap();

        let err = classpaths(&dir, &libraries, &client, "x86", false)
            .expect_err("a jar that never downloaded must not reach the jvm as a silent gap");
        let message = err.to_string();

        assert!(
            message.contains("net.fabricmc:intermediary:1.8.9"),
            "{message}"
        );
        assert!(message.contains("intermediary-1.8.9.jar"), "{message}");
    }

    #[test]
    fn a_tolerated_missing_library_keeps_its_expected_path() {
        let dir = std::env::temp_dir().join("oneclient-library-tolerated");
        let path = get_library(&dir, "net.fabricmc:intermediary:1.8.9", true)
            .expect("allow_missing hands back the path it would have used");

        assert!(path.ends_with("intermediary-1.8.9.jar"), "{path}");
    }

    #[test]
    fn a_natives_only_library_is_not_a_classpath_entry() {
        let libraries: Vec<Library> = serde_json::from_str(
            r#"[
                { "name": "net.fabricmc:fabric-loader:0.19.5" },
                { "name": "org.lwjgl.lwjgl:lwjgl:2.9.4+legacyfabric.15",
                  "downloads": { "artifact": { "sha1": "a", "size": 1, "url": "https://maven.legacyfabric.net/lwjgl.jar" } } },
                { "name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.4+legacyfabric.15",
                  "downloads": { "classifiers": { "natives-osx": { "sha1": "b", "size": 2, "url": "https://maven.legacyfabric.net/natives-osx.jar" } } },
                  "natives": { "osx": "natives-osx" } }
            ]"#,
        )
        .unwrap();

        let dir = std::env::temp_dir().join("oneclient-classpath-test");
        let client = dir.join("client.jar");
        for lib in [
            "net/fabricmc/fabric-loader/0.19.5/fabric-loader-0.19.5.jar",
            "org/lwjgl/lwjgl/lwjgl/2.9.4+legacyfabric.15/lwjgl-2.9.4+legacyfabric.15.jar",
        ] {
            let path = dir.join(lib);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"").unwrap();
        }
        std::fs::write(&client, b"").unwrap();

        let cp = classpaths(&dir, &libraries, &client, "x86", false)
            .expect("a jar that was never downloaded must not fail the launch");
        assert!(!cp.contains("lwjgl-platform"));
        assert!(cp.contains("fabric-loader") && cp.contains("lwjgl-2.9.4"));
    }

    #[test]
    fn a_loader_on_a_legacy_version_still_gets_a_classpath() {
        // ornithe's manifest declares two -D flags and nothing else
        // 1.8.9 has no `arguments` block to supply -cp or the natives path
        let loader_jvm: Vec<interfrost::api::minecraft::Argument> = serde_json::from_str(
            r#"["-Dfabric.fixPackageAccess=true", "-Dfabric.gameVersion=1.8.9"]"#,
        )
        .unwrap();
        let natives = std::env::temp_dir();

        let args = java_arguments(
            false,
            Some(&loader_jvm),
            &natives,
            &natives,
            "/libs/fabric-loader.jar",
            "1.8.9",
            2048,
            String::new(),
            "arm64",
            21,
        )
        .unwrap();

        assert!(args.contains(&"-Dfabric.fixPackageAccess=true".to_string()));
        let cp = args.iter().position(|a| a == "-cp").expect("no classpath");
        assert_eq!(args[cp + 1], "/libs/fabric-loader.jar");
        assert!(args.iter().any(|a| a.starts_with("-Djava.library.path=")));
    }

    #[test]
    fn a_loader_game_argument_does_not_replace_the_legacy_string() {
        let loader_game: Vec<interfrost::api::minecraft::Argument> =
            serde_json::from_str(r#"["--fabric"]"#).unwrap();
        let dir = std::env::temp_dir();

        let args = minecraft_arguments(
            false,
            Some(&loader_game),
            Some("--username ${auth_player_name} --gameDir ${game_directory}"),
            "token",
            "player",
            uuid::Uuid::nil(),
            "1.8.9",
            "1.8",
            &dir,
            &dir,
            interfrost::api::minecraft::VersionType::Release,
            Resolution::default(),
            "arm64",
        )
        .unwrap();

        assert_eq!(args.first().unwrap(), "--username");
        assert_eq!(args[1], "player");
        assert_eq!(args.last().unwrap(), "--fabric");
    }

    #[test]
    fn a_loader_repeating_the_legacy_string_is_not_passed_twice() {
        let loader_game: Vec<interfrost::api::minecraft::Argument> = serde_json::from_str(
            r#"["--username", "${auth_player_name}", "--gameDir", "${game_directory}",
                "--tweakClass", "net.minecraftforge.fml.common.launcher.FMLTweaker"]"#,
        )
        .unwrap();
        let dir = std::env::temp_dir();

        let args = minecraft_arguments(
            false,
            Some(&loader_game),
            Some(
                "--username ${auth_player_name} --gameDir ${game_directory}                  --tweakClass net.minecraftforge.fml.common.launcher.FMLTweaker",
            ),
            "token",
            "player",
            uuid::Uuid::nil(),
            "1.11",
            "1.11",
            &dir,
            &dir,
            interfrost::api::minecraft::VersionType::Release,
            Resolution::default(),
            "arm64",
        )
        .unwrap();

        assert_eq!(args.iter().filter(|a| *a == "--gameDir").count(), 1);
        assert_eq!(args.iter().filter(|a| *a == "--username").count(), 1);
        assert_eq!(args.iter().filter(|a| *a == "--tweakClass").count(), 1);
    }

    #[test]
    fn an_embedded_placeholder_keeps_its_surrounding_text() {
        let mut data = HashMap::new();
        data.insert(
            "ROOT".to_string(),
            SidedDataEntry {
                client: "/meta".to_string(),
                server: String::new(),
            },
        );
        data.insert(
            "BINPATCH".to_string(),
            SidedDataEntry {
                client: "/meta/libraries/patch.lzma".to_string(),
                server: String::new(),
            },
        );

        let args = [
            "--task",
            "PROCESS_MINECRAFT_JAR",
            "--extract-libraries-to",
            "{ROOT}/libraries/",
            "--apply-patches",
            "{BINPATCH}",
        ];

        let parsed = processor_arguments(Path::new("/libs"), &args, &data).unwrap();

        assert_eq!(parsed.len(), args.len());
        assert_eq!(parsed[3], "/meta/libraries/");
        assert_eq!(parsed[4], "--apply-patches");
        assert_eq!(parsed[5], "/meta/libraries/patch.lzma");
    }

    #[test]
    fn an_unknown_placeholder_is_left_alone_rather_than_dropped() {
        let data: HashMap<String, SidedDataEntry> = HashMap::new();
        let args = ["--flag", "{NOPE}", "--after"];

        let parsed = processor_arguments(Path::new("/libs"), &args, &data).unwrap();

        assert_eq!(parsed, vec!["--flag", "{NOPE}", "--after"]);
    }

    #[test]
    fn distinct_tweakers_both_survive() {
        let mut args = vec![
            "--tweakClass".to_string(),
            "forge.FMLTweaker".to_string(),
            "--tweakClass".to_string(),
            "optifine.OptiFineTweaker".to_string(),
            "--gameDir".to_string(),
            "/games".to_string(),
            "--gameDir".to_string(),
            "/games".to_string(),
        ];

        drop_repeated_arguments(&mut args);

        assert_eq!(args.iter().filter(|a| *a == "--tweakClass").count(), 2);
        assert_eq!(args.iter().filter(|a| *a == "--gameDir").count(), 1);
    }
}
