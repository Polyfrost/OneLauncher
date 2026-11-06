use std::collections::BTreeMap;
use std::path::Path;

use oneclient_common::domain::GameLoader;
use oneclient_events::VersionRange;
use serde::Deserialize;

use crate::packages::dependencies::compare_semver;

const MAX_DESCRIPTION: usize = 600;
const MAX_DESCRIPTION_WINDOW: usize = MAX_DESCRIPTION * 8;

const FABRIC: &str = "fabric.mod.json";
const QUILT: &str = "quilt.mod.json";
const NEOFORGE: &str = "META-INF/neoforge.mods.toml";
const FORGE: &str = "META-INF/mods.toml";
const LEGACY_FORGE: &str = "mcmod.info";

const MANIFESTS: [&str; 5] = [FABRIC, QUILT, NEOFORGE, FORGE, LEGACY_FORGE];

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct JarManifest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub icon_entry: Option<String>,
    pub mod_id: Option<String>,
    pub version: Option<String>,
}

impl JarManifest {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.description.is_none()
            && self.authors.is_empty()
            && self.icon_entry.is_none()
    }

    #[must_use]
    pub fn author_line(&self) -> String {
        self.authors.join(", ")
    }

    fn fill_from(&mut self, other: Self) {
        if self.name.is_none() {
            self.name = other.name;
        }
        if self.description.is_none() {
            self.description = other.description;
        }
        if self.authors.is_empty() {
            self.authors = other.authors;
        }
        if self.icon_entry.is_none() {
            self.icon_entry = other.icon_entry;
        }
        if self.mod_id.is_none() {
            self.mod_id = other.mod_id;
        }
        if self.version.is_none() {
            self.version = other.version;
        }
    }
}

#[tracing::instrument(level = "debug", fields(jar = %jar.display()))]
pub async fn read_jar_manifest(jar: &Path) -> JarManifest {
    let entries = match polyio::read_zip_file_entries(jar, |name| MANIFESTS.contains(&name)).await {
        Ok(entries) => entries,
        Err(err) => {
            tracing::debug!("could not read {}: {err}", jar.display());
            return JarManifest::default();
        }
    };

    let mut out = JarManifest::default();
    for wanted in MANIFESTS {
        let Some((_, bytes)) = entries.iter().find(|(name, _)| name == wanted) else {
            continue;
        };
        let Ok(text) = std::str::from_utf8(bytes) else {
            continue;
        };

        match parse_manifest(wanted, text) {
            Some(parsed) => out.fill_from(parsed),
            None => tracing::debug!("{wanted} in {} did not parse", jar.display()),
        }
    }

    out
}

fn parse_manifest(manifest: &str, text: &str) -> Option<JarManifest> {
    match manifest {
        FABRIC => parse_fabric(text),
        QUILT => parse_quilt(text),
        NEOFORGE | FORGE => parse_forge(text),
        _ => parse_legacy_forge(text),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JarModId {
    pub id: String,
    pub version: Option<String>,
}

#[tracing::instrument(level = "debug", fields(jar = %jar.display()))]
pub async fn read_jar_mod_id(jar: &Path) -> Option<JarModId> {
    let entries = polyio::read_zip_file_entries(jar, |name| MANIFESTS.contains(&name))
        .await
        .inspect_err(|err| tracing::debug!("could not read {}: {err}", jar.display()))
        .ok()?;

    MANIFESTS.into_iter().find_map(|wanted| {
        let (_, bytes) = entries.iter().find(|(name, _)| name == wanted)?;
        parse_mod(wanted, std::str::from_utf8(bytes).ok()?)
    })
}

const MAX_BUNDLED_JARS: usize = 64;

#[tracing::instrument(level = "debug", fields(jar = %jar.display()))]
pub async fn read_jar_bundled_mod_ids(jar: &Path) -> Vec<JarModId> {
    let Ok(entries) = polyio::read_zip_file_entries(jar, |name| name == FABRIC).await else {
        return Vec::new();
    };

    let Some(json) = entries
        .first()
        .and_then(|(_, bytes)| serde_json::from_slice::<serde_json::Value>(bytes).ok())
    else {
        return Vec::new();
    };

    let bundled: Vec<String> = json
        .pointer("/jars")
        .and_then(serde_json::Value::as_array)
        .map(|jars| {
            jars.iter()
                .filter_map(|entry| entry.get("file")?.as_str().map(str::to_owned))
                .take(MAX_BUNDLED_JARS)
                .collect()
        })
        .unwrap_or_default();

    if bundled.is_empty() {
        return Vec::new();
    }

    let Ok(mut cursor) =
        polyio::ZipEntryCursor::open(jar, |name| bundled.iter().any(|file| file == name)).await
    else {
        return Vec::new();
    };

    let mut found = Vec::new();
    for file in &bundled {
        let Ok(bytes) = cursor.read(file).await else {
            continue;
        };
        let Ok(manifest) =
            polyio::try_read_zip_entry_bytes(std::io::Cursor::new(bytes), FABRIC).await
        else {
            continue;
        };

        if let Some(id) = std::str::from_utf8(&manifest)
            .ok()
            .and_then(|text| parse_mod(FABRIC, text))
        {
            found.push(id);
        }
    }

    found
}

fn parse_mod(manifest: &str, text: &str) -> Option<JarModId> {
    let (id, version) = match manifest {
        NEOFORGE | FORGE => {
            let id = toml::from_str::<toml::Value>(text)
                .ok()?
                .get("mods")?
                .get(0)?
                .get("modId")?
                .as_str()?
                .to_owned();
            (id, None)
        }
        _ => {
            let json: serde_json::Value = serde_json::from_str(text).ok()?;
            let pointers: &[&str] = match manifest {
                FABRIC => &["/id"],
                QUILT => &["/quilt_loader/id"],
                _ => &["/0/modid", "/modList/0/modid"],
            };
            let id = pointers
                .iter()
                .find_map(|pointer| json.pointer(pointer)?.as_str())?
                .to_owned();
            let version = (manifest == FABRIC)
                .then(|| json.pointer("/version")?.as_str().map(str::to_owned))
                .flatten();
            (id, version)
        }
    };

    Some(JarModId {
        id: clean(Some(id)).filter(|id| !is_placeholder_id(id))?,
        version: clean(version).filter(|version| !version.contains('$')),
    })
}

fn is_placeholder_id(id: &str) -> bool {
    id.contains('$') || id.eq_ignore_ascii_case("examplemod")
}

#[tracing::instrument(level = "debug", fields(jar = %jar.display()))]
pub async fn read_jar_loader(jar: &Path) -> Option<GameLoader> {
    let entries = polyio::read_zip_file_entries(jar, |name| MANIFESTS.contains(&name))
        .await
        .inspect_err(|err| tracing::debug!("could not read {}: {err}", jar.display()))
        .ok()?;
    let has = |wanted: &str| entries.iter().any(|(name, _)| name == wanted);

    let fabric_family = has(FABRIC) || has(QUILT);
    let forge_family = has(FORGE) || has(LEGACY_FORGE) || has(NEOFORGE);
    if fabric_family && forge_family {
        return None;
    }

    [
        (FABRIC, GameLoader::Fabric),
        (QUILT, GameLoader::Quilt),
        (FORGE, GameLoader::Forge),
        (LEGACY_FORGE, GameLoader::Forge),
        (NEOFORGE, GameLoader::NeoForge),
    ]
    .into_iter()
    .find(|(wanted, _)| has(wanted))
    .map(|(_, loader)| loader)
}

#[tracing::instrument(level = "debug", fields(jar = %jar.display()))]
pub async fn read_jar_icon(jar: &Path, entry: &str) -> Option<Vec<u8>> {
    let entry = entry.to_string();
    let found = polyio::read_zip_file_entries(jar, |name| name == entry)
        .await
        .inspect_err(|err| tracing::debug!("no icon in {}: {err}", jar.display()))
        .ok()?;

    found.into_iter().next().map(|(_, bytes)| bytes)
}

#[must_use]
pub fn parse_fabric(text: &str) -> Option<JarManifest> {
    #[derive(Deserialize)]
    struct FabricMod {
        id: Option<String>,
        version: Option<String>,
        name: Option<String>,
        description: Option<String>,
        #[serde(default)]
        authors: Vec<serde_json::Value>,
        icon: Option<Icon>,
    }

    let parsed: FabricMod = serde_json::from_str(text).ok()?;

    Some(JarManifest {
        name: clean(parsed.name),
        description: clean_description(parsed.description),
        authors: people(parsed.authors),
        icon_entry: parsed.icon.and_then(Icon::path).and_then(entry_path),
        mod_id: clean(parsed.id),
        version: real_version(parsed.version),
    })
}

#[must_use]
pub fn parse_quilt(text: &str) -> Option<JarManifest> {
    #[derive(Deserialize)]
    struct QuiltMod {
        quilt_loader: QuiltLoader,
    }

    #[derive(Deserialize)]
    struct QuiltLoader {
        id: Option<String>,
        version: Option<String>,
        metadata: Option<QuiltMetadata>,
    }

    #[derive(Deserialize, Default)]
    struct QuiltMetadata {
        name: Option<String>,
        description: Option<String>,
        #[serde(default)]
        contributors: BTreeMap<String, serde_json::Value>,
        icon: Option<Icon>,
    }

    let parsed: QuiltMod = serde_json::from_str(text).ok()?;
    let mod_id = clean(parsed.quilt_loader.id);
    let version = real_version(parsed.quilt_loader.version);
    let metadata = parsed.quilt_loader.metadata.unwrap_or_default();

    Some(JarManifest {
        name: clean(metadata.name),
        description: clean_description(metadata.description),
        authors: metadata
            .contributors
            .into_keys()
            .filter_map(|name| clean(Some(name)))
            .collect(),
        icon_entry: metadata.icon.and_then(Icon::path).and_then(entry_path),
        mod_id,
        version,
    })
}

#[must_use]
pub fn parse_forge(text: &str) -> Option<JarManifest> {
    #[derive(Deserialize)]
    struct ForgeManifest {
        #[serde(default)]
        mods: Vec<ForgeMod>,
        /// Some older jars put it above the mod list rather than inside it
        #[serde(rename = "logoFile")]
        logo_file: Option<String>,
    }

    #[derive(Deserialize)]
    struct ForgeMod {
        #[serde(rename = "modId")]
        mod_id: Option<String>,
        version: Option<String>,
        #[serde(rename = "displayName")]
        display_name: Option<String>,
        description: Option<String>,
        authors: Option<StringOrList>,
        credits: Option<StringOrList>,
        #[serde(rename = "logoFile")]
        logo_file: Option<String>,
    }

    let parsed: ForgeManifest = toml::from_str(text).ok()?;
    let first = parsed.mods.into_iter().next()?;

    Some(JarManifest {
        name: clean(first.display_name),
        description: clean_description(first.description),
        authors: first
            .authors
            .or(first.credits)
            .map(StringOrList::into_people)
            .unwrap_or_default(),
        icon_entry: first.logo_file.or(parsed.logo_file).and_then(entry_path),
        mod_id: clean(first.mod_id),
        version: real_version(first.version),
    })
}

#[must_use]
pub fn parse_legacy_forge(text: &str) -> Option<JarManifest> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum McModInfo {
        Bare(Vec<LegacyMod>),
        Wrapped {
            #[serde(rename = "modList")]
            mod_list: Vec<LegacyMod>,
        },
    }

    #[derive(Deserialize)]
    struct LegacyMod {
        modid: Option<String>,
        version: Option<String>,
        name: Option<String>,
        description: Option<String>,
        #[serde(rename = "authorList", default)]
        author_list: Vec<String>,
        credits: Option<String>,
        #[serde(rename = "logoFile")]
        logo_file: Option<String>,
    }

    let mods = match serde_json::from_str::<McModInfo>(text).ok()? {
        McModInfo::Bare(mods) | McModInfo::Wrapped { mod_list: mods } => mods,
    };
    let first = mods.into_iter().next()?;

    let authors: Vec<String> = first
        .author_list
        .into_iter()
        .filter_map(|name| clean(Some(name)))
        .collect();

    Some(JarManifest {
        name: clean(first.name),
        description: clean_description(first.description),
        authors: if authors.is_empty() {
            clean(first.credits).into_iter().collect()
        } else {
            authors
        },
        icon_entry: first.logo_file.and_then(entry_path),
        mod_id: clean(first.modid),
        version: real_version(first.version),
    })
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Icon {
    Path(String),
    Sizes(BTreeMap<String, String>),
}

impl Icon {
    fn path(self) -> Option<String> {
        match self {
            Self::Path(path) => Some(path),
            Self::Sizes(sizes) => sizes
                .into_iter()
                .max_by_key(|(size, _)| size.parse::<u32>().unwrap_or(0))
                .map(|(_, path)| path),
        }
    }
}

fn people(list: Vec<serde_json::Value>) -> Vec<String> {
    list.into_iter()
        .filter_map(|person| match person {
            serde_json::Value::String(name) => clean(Some(name)),
            serde_json::Value::Object(mut fields) => match fields.remove("name") {
                Some(serde_json::Value::String(name)) => clean(Some(name)),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StringOrList {
    One(String),
    Many(Vec<String>),
}

impl StringOrList {
    fn into_people(self) -> Vec<String> {
        match self {
            Self::One(text) => clean(Some(text)).into_iter().collect(),
            Self::Many(list) => list
                .into_iter()
                .filter_map(|name| clean(Some(name)))
                .collect(),
        }
    }
}

fn real_version(value: Option<String>) -> Option<String> {
    clean(value).filter(|version| !version.contains("${"))
}

fn clean(value: Option<String>) -> Option<String> {
    let value = value?.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn clean_description(value: Option<String>) -> Option<String> {
    let raw = clean(value)?;

    // Collapsing whitespace only shortens so a window this wide still holds
    // words to spare and a megabyte long blurb is never copied whole
    let end = raw
        .char_indices()
        .nth(MAX_DESCRIPTION_WINDOW)
        .map_or(raw.len(), |(idx, _)| idx);

    let mut text = raw[..end].split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return None;
    }

    if text.chars().count() > MAX_DESCRIPTION {
        let cut = text
            .char_indices()
            .nth(MAX_DESCRIPTION)
            .map_or(text.len(), |(idx, _)| idx);
        text.truncate(cut);
        text.push('…');
    } else if end < raw.len() {
        text.push('…');
    }

    Some(text)
}

fn entry_path(raw: String) -> Option<String> {
    let path = raw
        .trim()
        .replace('\\', "/")
        .trim_start_matches('/')
        .trim_start_matches("./")
        .to_string();

    if path.is_empty() || path.split('/').any(|part| part == "..") {
        return None;
    }

    Some(path)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JarDependencies {
    /// The jar's own mod ids
    pub provides: Vec<String>,
    pub nested: Vec<JarDependencies>,
    pub requires: Vec<String>,
    pub version: Option<String>,
    pub semver: bool,
    pub multi_loader: bool,
    pub ranges: BTreeMap<String, Vec<VersionRange>>,
    pub alias_versions: BTreeMap<String, String>,
    pub breaks: BTreeMap<String, Vec<VersionRange>>,
    pub server_only: bool,
}

impl JarDependencies {
    #[must_use]
    pub fn bundled(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .nested
            .iter()
            .flat_map(|nested| &nested.provides)
            .filter(|id| !self.provides.contains(id))
            .cloned()
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }

    pub fn all_ids(&self) -> impl Iterator<Item = &String> {
        let nested = self.nested.iter().flat_map(|nested| &nested.provides);
        self.provides.iter().chain(nested)
    }

    pub fn all_ranges(&self) -> impl Iterator<Item = (&String, &Vec<VersionRange>)> {
        let nested = self.nested.iter().flat_map(|nested| &nested.ranges);
        self.ranges.iter().chain(nested)
    }

    pub fn all_breaks(&self) -> impl Iterator<Item = (&String, &Vec<VersionRange>)> {
        let nested = self.nested.iter().flat_map(|nested| &nested.breaks);
        self.breaks.iter().chain(nested)
    }
}

fn loader_manifests(loader: GameLoader) -> &'static [&'static str] {
    match loader {
        GameLoader::Fabric | GameLoader::Ornithe => &[FABRIC],
        GameLoader::Quilt => &[QUILT, FABRIC],
        GameLoader::Forge => &[FORGE, LEGACY_FORGE],
        GameLoader::NeoForge => &[NEOFORGE, FORGE],
        GameLoader::Vanilla => &[],
    }
}

fn manifests_read<'a>(present: Vec<&'a str>, loader: Option<GameLoader>) -> Vec<&'a str> {
    let active = loader.map_or(&[][..], loader_manifests);
    match active.iter().find(|wanted| present.contains(wanted)) {
        Some(wanted) => vec![*wanted],
        None => present,
    }
}

const JAR_IN_JAR: &str = "META-INF/jarjar/metadata.json";

const MAX_NESTING: usize = 4;

fn declared_jars(manifest: &str, text: &str) -> Vec<String> {
    let list = match manifest {
        FABRIC | JAR_IN_JAR => "/jars",
        QUILT => "/quilt_loader/jars",
        _ => return Vec::new(),
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };

    json.pointer(list)
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|jar| {
            let path = jar
                .get("file")
                .or(jar.get("path"))
                .unwrap_or(jar)
                .as_str()?;
            entry_path(path.to_owned())
        })
        .collect()
}

fn reads_jar_in_jar(read: &[&str]) -> bool {
    read.iter().any(|name| matches!(*name, NEOFORGE | FORGE))
}

#[tracing::instrument(level = "debug", fields(jar = %jar.display()))]
pub async fn read_jar_dependencies(jar: &Path, loader: Option<GameLoader>) -> JarDependencies {
    let entries = match polyio::read_zip_file_entries(jar, |name| {
        MANIFESTS.contains(&name) || name == JAR_IN_JAR
    })
    .await
    {
        Ok(entries) => entries,
        Err(err) => {
            tracing::debug!("could not read {}: {err}", jar.display());
            return JarDependencies::default();
        }
    };

    let present = entries
        .iter()
        .map(|(name, _)| name.as_str())
        .filter(|name| MANIFESTS.contains(name))
        .collect();
    let read = manifests_read(present, loader);

    let mut out = JarDependencies::default();
    let mut manifests = 0;
    let mut declared = Vec::new();
    for (name, bytes) in &entries {
        let wanted =
            read.contains(&name.as_str()) || (name == JAR_IN_JAR && reads_jar_in_jar(&read));
        if !wanted {
            continue;
        }
        let Ok(text) = std::str::from_utf8(bytes) else {
            continue;
        };
        declared.extend(declared_jars(name, text));
        if let Some(parsed) = parse_dependencies(name, text) {
            manifests += 1;
            if out.version.is_none() {
                out.version = parsed.version;
                out.semver = parsed.semver;
            }
            out.provides.extend(parsed.provides);
            out.requires.extend(parsed.requires);
            out.alias_versions.extend(parsed.alias_versions);
            out.server_only |= parsed.server_only;
            for (id, ranges) in parsed.ranges {
                out.ranges.entry(id).or_default().extend(ranges);
            }
            for (id, ranges) in parsed.breaks {
                out.breaks.entry(id).or_default().extend(ranges);
            }
        }
    }

    if !declared.is_empty() && !out.server_only {
        match polyio::read_zip_file_entries(jar, |name| declared.iter().any(|file| file == name))
            .await
        {
            Ok(jars) => out.nested = nested_mods(jars, loader).await,
            Err(err) => tracing::debug!("could not read {}: {err}", jar.display()),
        }
    }

    out.provides.sort();
    out.provides.dedup();
    out.multi_loader = manifests > 1;
    let provides = &out.provides;
    out.requires.sort();
    out.requires.dedup();
    out.requires
        .retain(|id| provides.binary_search(id).is_err());
    let requires = &out.requires;
    out.ranges
        .retain(|id, _| requires.binary_search(id).is_ok());
    let nested = &out.nested;
    out.requires
        .retain(|id| !nested.iter().any(|n| n.provides.contains(id)));
    out
}

async fn nested_mods(
    jars: Vec<(String, Vec<u8>)>,
    loader: Option<GameLoader>,
) -> Vec<JarDependencies> {
    let mut mods = Vec::new();
    let mut pending: Vec<(Vec<u8>, usize)> =
        jars.into_iter().map(|(_, bytes)| (bytes, 1)).collect();

    while let Some((bytes, depth)) = pending.pop() {
        let mut found = Vec::new();
        for wanted in MANIFESTS.into_iter().chain([JAR_IN_JAR]) {
            if let Ok(data) =
                polyio::try_read_zip_entry_bytes(std::io::Cursor::new(&bytes), wanted).await
            {
                found.push((wanted, data));
            }
        }

        let present = found
            .iter()
            .map(|(name, _)| *name)
            .filter(|name| *name != JAR_IN_JAR)
            .collect();
        let read = manifests_read(present, loader);
        let multi_loader = read.len() > 1;

        let mut declared = Vec::new();
        for (name, data) in &found {
            let Ok(text) = std::str::from_utf8(data) else {
                continue;
            };
            if *name == JAR_IN_JAR {
                if reads_jar_in_jar(&read) {
                    declared.extend(declared_jars(name, text));
                }
            } else if read.contains(name) {
                let parsed = parse_dependencies(name, text);
                if parsed.as_ref().is_some_and(|parsed| parsed.server_only) {
                    continue;
                }
                declared.extend(declared_jars(name, text));
                mods.extend(parsed.map(|parsed| JarDependencies {
                    multi_loader,
                    ..parsed
                }));
            }
        }

        if depth == MAX_NESTING {
            continue;
        }
        for file in declared {
            if let Ok(jar) =
                polyio::try_read_zip_entry_bytes(std::io::Cursor::new(&bytes), &file).await
            {
                pending.push((jar, depth + 1));
            }
        }
    }

    mods
}

fn own_versions<'a>(
    mut mods: impl Iterator<Item = (&'a str, &'a str)>,
) -> BTreeMap<String, String> {
    let Some((_, first)) = mods.next() else {
        return BTreeMap::new();
    };
    mods.filter(|(_, version)| *version != first)
        .map(|(id, version)| (id.trim().to_ascii_lowercase(), version.to_owned()))
        .collect()
}

fn parse_dependencies(manifest: &str, text: &str) -> Option<JarDependencies> {
    let mut alias_versions = BTreeMap::new();
    let mut breaks = BTreeMap::new();
    let mut server_only = false;
    let (provides, requires, ranges) = match manifest {
        FABRIC => {
            let json: serde_json::Value = serde_json::from_str(text).ok()?;
            server_only = json
                .get("environment")
                .and_then(|v| v.as_str())
                .is_some_and(|side| side.eq_ignore_ascii_case("server"));
            breaks = json
                .get("breaks")
                .and_then(|v| v.as_object())
                .into_iter()
                .flatten()
                .filter_map(|(id, banned)| {
                    Some((id.trim().to_ascii_lowercase(), semver_ranges(banned)?))
                })
                .collect();
            let provides = json
                .get("id")
                .into_iter()
                .chain(
                    json.get("provides")
                        .and_then(|v| v.as_array())
                        .into_iter()
                        .flatten(),
                )
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect();
            let depends = json.get("depends").and_then(|v| v.as_object());
            let requires = depends
                .map(|map| map.keys().cloned().collect())
                .unwrap_or_default();
            let ranges = depends
                .into_iter()
                .flatten()
                .filter_map(|(id, wanted)| Some((id.clone(), semver_ranges(wanted)?)))
                .collect();
            (provides, requires, ranges)
        }
        QUILT => {
            let json: serde_json::Value = serde_json::from_str(text).ok()?;
            let loader = json.get("quilt_loader")?;
            let aliases = loader
                .get("provides")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten();
            alias_versions = aliases.clone().filter_map(quilt_alias_version).collect();
            let provides = loader
                .get("id")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .into_iter()
                .chain(aliases.filter_map(quilt_id))
                .collect();
            let depends: Vec<&serde_json::Value> = loader
                .get("depends")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
                .filter(|dep| dep.get("optional").and_then(|v| v.as_bool()) != Some(true))
                .filter(|dep| {
                    dep.get("environment").and_then(|v| v.as_str()) != Some("dedicated_server")
                })
                .collect();
            let requires = depends.iter().copied().filter_map(quilt_id).collect();
            let ranges = depends
                .iter()
                .filter_map(|dep| Some((quilt_id(dep)?, semver_ranges(dep.get("versions")?)?)))
                .collect();
            (provides, requires, ranges)
        }
        NEOFORGE | FORGE => {
            let toml: toml::Value = toml::from_str(text).ok()?;
            let provides = toml
                .get("mods")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
                .filter_map(|m| m.get("modId")?.as_str().map(str::to_owned))
                .collect();
            alias_versions = own_versions(
                toml.get("mods")
                    .and_then(|v| v.as_array())
                    .into_iter()
                    .flatten()
                    .filter_map(|m| {
                        let version = m.get("version").and_then(|v| v.as_str()).unwrap_or("1");
                        Some((m.get("modId")?.as_str()?, version))
                    }),
            );
            let dependencies: Vec<(&str, &toml::Value)> = toml
                .get("dependencies")
                .and_then(|v| v.as_table())
                .into_iter()
                .flat_map(|table| table.values())
                .filter_map(|v| v.as_array())
                .flatten()
                .filter_map(|dep| Some((dep.get("modId")?.as_str()?, dep)))
                .collect();
            for (id, dep) in dependencies.iter().filter(|(_, dep)| forge_banned(dep)) {
                breaks
                    .entry(id.trim().to_ascii_lowercase())
                    .or_default()
                    .extend(forge_banned_ranges(dep));
            }
            let depends: Vec<(&str, &toml::Value)> = dependencies
                .into_iter()
                .filter(|(_, dep)| forge_required(dep))
                .collect();
            let requires = depends.iter().map(|(id, _)| (*id).to_owned()).collect();
            let ranges = depends
                .iter()
                .filter_map(|(id, dep)| {
                    let spec = dep.get("versionRange")?.as_str()?;
                    Some(((*id).to_owned(), maven_ranges(spec)?))
                })
                .collect();
            (provides, requires, ranges)
        }
        LEGACY_FORGE => {
            let json: serde_json::Value = serde_json::from_str(text).ok()?;
            let mods = json
                .as_array()
                .or_else(|| json.get("modList")?.as_array())?;
            let provides = mods
                .iter()
                .filter_map(|m| m.get("modid")?.as_str().map(str::to_owned))
                .collect();
            alias_versions = own_versions(mods.iter().filter_map(|m| {
                let version = m
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                Some((m.get("modid")?.as_str()?, version))
            }));
            let depends: Vec<&str> = mods
                .iter()
                .filter_map(|m| m.get("requiredMods")?.as_array())
                .flatten()
                .filter_map(|v| v.as_str())
                .collect();
            let requires = depends
                .iter()
                .map(|spec| spec.split('@').next().unwrap_or(spec).to_owned())
                .collect();
            let ranges = depends
                .iter()
                .filter_map(|spec| {
                    let (id, range) = spec.split_once('@')?;
                    Some((id.to_owned(), maven_ranges(range)?))
                })
                .collect();
            (provides, requires, ranges)
        }
        _ => return None,
    };

    let ranges: Vec<(String, Vec<VersionRange>)> = ranges;
    Some(JarDependencies {
        version: parse_manifest(manifest, text).and_then(|parsed| parsed.version),
        semver: matches!(manifest, FABRIC | QUILT),
        multi_loader: false,
        provides: normalize_ids(provides),
        nested: Vec::new(),
        requires: normalize_ids(requires),
        ranges: ranges
            .into_iter()
            .map(|(id, ranges)| (id.trim().to_ascii_lowercase(), ranges))
            .collect(),
        alias_versions,
        breaks,
        server_only,
    })
}

fn semver_ranges(wanted: &serde_json::Value) -> Option<Vec<VersionRange>> {
    match wanted {
        serde_json::Value::String(spec) => Some(vec![semver_range(spec)?]),
        serde_json::Value::Array(specs) if !specs.is_empty() => specs
            .iter()
            .map(|spec| semver_range(spec.as_str()?))
            .collect(),
        serde_json::Value::Object(map) => semver_ranges(map.get("any")?),
        _ => None,
    }
}

fn semver_range(spec: &str) -> Option<VersionRange> {
    use std::cmp::Ordering;

    let mut range = VersionRange::default();
    for clause in spec.split_whitespace() {
        let parsed = semver_clause(clause)?;
        if let Some(min) = parsed.min
            && narrows(
                (&min, parsed.min_inclusive),
                (range.min.as_deref(), range.min_inclusive),
                Ordering::Greater,
            )
        {
            range.min = Some(min);
            range.min_inclusive = parsed.min_inclusive;
        }
        if let Some(max) = parsed.max
            && narrows(
                (&max, parsed.max_inclusive),
                (range.max.as_deref(), range.max_inclusive),
                Ordering::Less,
            )
        {
            range.max = Some(max);
            range.max_inclusive = parsed.max_inclusive;
        }
    }
    Some(range)
}

fn narrows(
    (new, new_inclusive): (&str, bool),
    (old, old_inclusive): (Option<&str>, bool),
    tighter: std::cmp::Ordering,
) -> bool {
    let Some(old) = old else {
        return true;
    };
    compare_semver(new, old).is_some_and(|ordering| {
        ordering == tighter || (ordering.is_eq() && old_inclusive && !new_inclusive)
    })
}

fn semver_clause(clause: &str) -> Option<VersionRange> {
    if matches!(clause, "*" | "x" | "X") {
        return Some(VersionRange::default());
    }

    let at_least = |min: &str, min_inclusive, max: Option<String>| VersionRange {
        min: Some(min.to_string()),
        min_inclusive,
        max,
        max_inclusive: false,
    };
    let at_most = |max: &str, max_inclusive| VersionRange {
        max: Some(max.to_string()),
        max_inclusive,
        ..VersionRange::default()
    };

    if let Some(min) = clause.strip_prefix(">=") {
        return Some(at_least(min, true, None));
    }
    if let Some(max) = clause.strip_prefix("<=") {
        return Some(at_most(max, true));
    }
    if let Some(min) = clause.strip_prefix('>') {
        return Some(at_least(min, false, None));
    }
    if let Some(max) = clause.strip_prefix('<') {
        return Some(at_most(max, false));
    }
    if let Some(min) = clause.strip_prefix('^') {
        return Some(at_least(min, true, bump(min, 0)));
    }
    if let Some(min) = clause.strip_prefix('~') {
        return Some(at_least(min, true, bump(min, 1)));
    }

    let exact = clause.strip_prefix('=').unwrap_or(clause);
    let stem = [".x", ".X", ".*"]
        .iter()
        .find_map(|wildcard| exact.strip_suffix(wildcard));
    match stem {
        Some(stem) => {
            let last = stem.split('.').count() - 1;
            Some(at_least(&format!("{stem}-"), true, Some(bump(stem, last)?)))
        }
        None => Some(VersionRange::exactly(exact)),
    }
}

fn bump(version: &str, index: usize) -> Option<String> {
    let core = version.split(['-', '+']).next()?;
    let mut parts: Vec<u64> = core
        .split('.')
        .map(|part| part.parse().ok())
        .collect::<Option<_>>()?;
    parts.resize(index + 1, 0);
    parts[index] += 1;
    let bumped: Vec<String> = parts.iter().map(u64::to_string).collect();
    Some(format!("{}-", bumped.join(".")))
}

fn maven_ranges(spec: &str) -> Option<Vec<VersionRange>> {
    let mut rest = spec.trim();
    if !rest.starts_with(['[', '(']) {
        return None;
    }

    let mut ranges = Vec::new();
    while let Some(end) = rest.find([')', ']']) {
        let (group, tail) = rest.split_at(end + 1);
        ranges.push(maven_range(group)?);
        rest = tail.trim_start_matches([',', ' ']);
    }

    (rest.is_empty() && !ranges.is_empty()).then_some(ranges)
}

fn maven_range(group: &str) -> Option<VersionRange> {
    let inner = group
        .strip_prefix(['[', '('])?
        .strip_suffix([')', ']'])?
        .trim();
    let bound = |text: &str| Some(text.trim().to_string()).filter(|text| !text.is_empty());

    Some(match inner.split_once(',') {
        Some((low, high)) => VersionRange {
            min: bound(low),
            min_inclusive: group.starts_with('['),
            max: bound(high),
            max_inclusive: group.ends_with(']'),
        },
        None => VersionRange::exactly(inner),
    })
}

fn quilt_id(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(id) => Some(id.clone()),
        other => other.get("id")?.as_str().map(str::to_owned),
    }
}

fn quilt_alias_version(value: &serde_json::Value) -> Option<(String, String)> {
    let version = real_version(value.get("version")?.as_str().map(str::to_owned))?;
    Some((quilt_id(value)?.trim().to_ascii_lowercase(), version))
}

fn forge_on_client(dep: &toml::Value) -> bool {
    let side = dep.get("side").and_then(|v| v.as_str());
    !side.is_some_and(|side| side.eq_ignore_ascii_case("SERVER"))
}

fn forge_banned(dep: &toml::Value) -> bool {
    let kind = dep.get("type").and_then(|v| v.as_str());
    forge_on_client(dep) && kind.is_some_and(|kind| kind.eq_ignore_ascii_case("incompatible"))
}

fn forge_banned_ranges(dep: &toml::Value) -> Vec<VersionRange> {
    dep.get("versionRange")
        .and_then(|v| v.as_str())
        .and_then(maven_ranges)
        .unwrap_or_else(|| vec![VersionRange::default()])
}

fn forge_required(dep: &toml::Value) -> bool {
    if !forge_on_client(dep) {
        return false;
    }

    match dep.get("type").and_then(|v| v.as_str()) {
        Some(kind) => kind.eq_ignore_ascii_case("required"),
        None => dep.get("mandatory").and_then(|v| v.as_bool()) == Some(true),
    }
}

fn normalize_ids(ids: Vec<String>) -> Vec<String> {
    ids.into_iter()
        .map(|id| id.trim().to_ascii_lowercase())
        .filter(|id| !id.is_empty() && !is_placeholder_id(id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_mod_id(manifest: &str, text: &str) -> Option<String> {
        parse_mod(manifest, text).map(|parsed| parsed.id)
    }

    fn parse_mod_version(manifest: &str, text: &str) -> Option<String> {
        parse_mod(manifest, text)?.version
    }

    #[test]
    fn every_loader_names_its_mod_id() {
        assert_eq!(
            parse_mod_id(FABRIC, r#"{"id":"argentum","name":"Argentum"}"#).as_deref(),
            Some("argentum")
        );
        assert_eq!(
            parse_mod_id(QUILT, r#"{"quilt_loader":{"id":"qsl"}}"#).as_deref(),
            Some("qsl")
        );
        assert_eq!(
            parse_mod_id(FORGE, "[[mods]]\nmodId = \"jei\"\n").as_deref(),
            Some("jei")
        );
        assert_eq!(
            parse_mod_id(LEGACY_FORGE, r#"[{"modid":"patcher"}]"#).as_deref(),
            Some("patcher")
        );
        assert_eq!(
            parse_mod_id(LEGACY_FORGE, r#"{"modList":[{"modid":"oneconfig"}]}"#).as_deref(),
            Some("oneconfig")
        );
        assert_eq!(parse_mod_id(FABRIC, r#"{"id":"  "}"#), None);

        assert_eq!(
            parse_mod_version(FABRIC, r#"{"id":"fabric-api","version":"0.141.6+1.21.11"}"#)
                .as_deref(),
            Some("0.141.6+1.21.11")
        );
        assert_eq!(
            parse_mod_version(QUILT, r#"{"quilt_loader":{"id":"qsl","version":"1.2.3"}}"#),
            None
        );
        assert_eq!(
            parse_mod_version(FABRIC, r#"{"id":"a","version":"${version}"}"#),
            None
        );
        assert_eq!(
            parse_mod_version(FORGE, "[[mods]]\nmodId = \"jei\"\nversion = \"1\"\n"),
            None
        );
        assert_eq!(
            parse_mod_id(LEGACY_FORGE, r#"[{"modid":"examplemod"}]"#),
            None
        );
        assert_eq!(
            parse_mod_id(LEGACY_FORGE, r#"[{"modid":"${modid}"}]"#),
            None
        );
    }

    #[test]
    fn fabric_reads_every_field() {
        let parsed = parse_fabric(
            r#"{
				"schemaVersion": 1,
				"id": "sodium",
				"name": "Sodium",
				"description": "A modern\n  rendering engine",
				"authors": ["JellySquid", {"name": "Someone", "contact": {}}],
				"icon": "assets/sodium/icon.png"
			}"#,
        )
        .expect("valid fabric manifest");

        assert_eq!(parsed.name.as_deref(), Some("Sodium"));
        assert_eq!(
            parsed.description.as_deref(),
            Some("A modern rendering engine"),
            "hard wrapping collapses so the card lays out its own lines"
        );
        assert_eq!(parsed.author_line(), "JellySquid, Someone");
        assert_eq!(parsed.icon_entry.as_deref(), Some("assets/sodium/icon.png"));
    }

    #[test]
    fn fabric_icon_map_takes_the_largest() {
        let parsed = parse_fabric(
            r#"{"id": "x", "icon": {"32": "small.png", "128": "big.png", "64": "mid.png"}}"#,
        )
        .expect("valid fabric manifest");

        assert_eq!(parsed.icon_entry.as_deref(), Some("big.png"));
    }

    #[test]
    fn fabric_survives_an_author_shaped_oddly() {
        let parsed = parse_fabric(r#"{"id": "x", "name": "X", "authors": [["nested"], "Real"]}"#)
            .expect("one odd author must not sink the manifest");

        assert_eq!(parsed.author_line(), "Real");
    }

    #[test]
    fn quilt_reads_contributor_names() {
        let parsed = parse_quilt(
            r#"{
				"schema_version": 1,
				"quilt_loader": {
					"id": "example",
					"metadata": {
						"name": "Example",
						"description": "Does things",
						"contributors": {"Alice": "Owner", "Bob": "Author"},
						"icon": "assets/example/icon.png"
					}
				}
			}"#,
        )
        .expect("valid quilt manifest");

        assert_eq!(parsed.name.as_deref(), Some("Example"));
        assert_eq!(parsed.author_line(), "Alice, Bob");
        assert_eq!(
            parsed.icon_entry.as_deref(),
            Some("assets/example/icon.png")
        );
    }

    #[test]
    fn quilt_keeps_the_mod_id_without_metadata() {
        let parsed = parse_quilt(
            r#"{"schema_version": 1, "quilt_loader": {"id": "example", "version": "1.2.0"}}"#,
        )
        .expect("metadata is optional in quilt");

        assert_eq!(parsed.mod_id.as_deref(), Some("example"));
        assert_eq!(parsed.version.as_deref(), Some("1.2.0"));
        assert!(parsed.is_empty());
    }

    #[test]
    fn forge_reads_the_first_mod_entry() {
        let parsed = parse_forge(
            r#"
modLoader="javafml"
loaderVersion="[47,)"
license="MIT"

[[mods]]
modId="examplemod"
version="1.0.0"
displayName="Example Mod"
authors="Someone"
logoFile="examplemod.png"
description='''
A mod that
spans lines
'''

[[dependencies.examplemod]]
modId="forge"
mandatory=true
"#,
        )
        .expect("valid forge manifest");

        assert_eq!(parsed.name.as_deref(), Some("Example Mod"));
        assert_eq!(
            parsed.description.as_deref(),
            Some("A mod that spans lines")
        );
        assert_eq!(parsed.author_line(), "Someone");
        assert_eq!(parsed.icon_entry.as_deref(), Some("examplemod.png"));
    }

    #[test]
    fn forge_falls_back_to_credits_and_a_root_logo() {
        let parsed = parse_forge(
            r#"
logoFile="logo.png"

[[mods]]
modId="x"
displayName="X"
credits="Thanks to everyone"
"#,
        )
        .expect("valid forge manifest");

        assert_eq!(parsed.author_line(), "Thanks to everyone");
        assert_eq!(parsed.icon_entry.as_deref(), Some("logo.png"));
    }

    #[test]
    fn legacy_forge_reads_both_shapes() {
        let bare = parse_legacy_forge(
			r#"[{"modid":"x","name":"X","description":"Old","authorList":["A","B"],"logoFile":"/logo.png"}]"#,
		)
		.expect("valid mcmod.info");

        assert_eq!(bare.name.as_deref(), Some("X"));
        assert_eq!(bare.author_line(), "A, B");
        assert_eq!(
            bare.icon_entry.as_deref(),
            Some("logo.png"),
            "the leading slash is jar relative not absolute"
        );

        let wrapped = parse_legacy_forge(
            r#"{"modListVersion":2,"modList":[{"modid":"y","name":"Y","authorList":["C"]}]}"#,
        )
        .expect("valid v2 mcmod.info");

        assert_eq!(wrapped.name.as_deref(), Some("Y"));
    }

    #[test]
    fn reads_mod_id_and_version_but_not_placeholders() {
        let fabric =
            parse_fabric(r#"{"id":"sodium-extra","version":"0.9.4+mc26.2"}"#).expect("fabric");
        assert_eq!(fabric.mod_id.as_deref(), Some("sodium-extra"));
        assert_eq!(fabric.version.as_deref(), Some("0.9.4+mc26.2"));

        let forge = parse_forge("[[mods]]\nmodId = \"jei\"\nversion = \"${file.jarVersion}\"\n")
            .expect("forge");
        assert_eq!(forge.mod_id.as_deref(), Some("jei"));
        assert_eq!(forge.version, None);
    }

    #[test]
    fn a_second_manifest_only_fills_gaps() {
        let mut fabric = JarManifest {
            name: Some("Fabric Name".into()),
            ..JarManifest::default()
        };
        fabric.fill_from(JarManifest {
            name: Some("Forge Name".into()),
            description: Some("From forge".into()),
            authors: vec!["Someone".into()],
            icon_entry: Some("logo.png".into()),
            ..JarManifest::default()
        });

        assert_eq!(fabric.name.as_deref(), Some("Fabric Name"));
        assert_eq!(fabric.description.as_deref(), Some("From forge"));
        assert_eq!(fabric.author_line(), "Someone");
        assert_eq!(fabric.icon_entry.as_deref(), Some("logo.png"));
    }

    #[test]
    fn traversing_icon_paths_are_refused() {
        assert_eq!(entry_path("../../etc/passwd".into()), None);
        assert_eq!(entry_path("   ".into()), None);
        assert_eq!(entry_path("./icon.png".into()).as_deref(), Some("icon.png"));
    }

    #[test]
    fn long_descriptions_are_cut_on_a_char_boundary() {
        let long = "ż".repeat(MAX_DESCRIPTION + 50);
        let cut = clean_description(Some(long)).expect("non empty");

        assert_eq!(
            cut.chars().count(),
            MAX_DESCRIPTION + 1,
            "the ellipsis is the extra one"
        );
        assert!(cut.ends_with('…'));
    }

    #[test]
    fn garbage_is_not_a_manifest() {
        assert!(parse_fabric("not json").is_none());
        assert!(parse_forge("[[mods").is_none());
        assert!(
            parse_forge("modLoader=\"javafml\"").is_none(),
            "no mod entry to describe"
        );
    }

    fn range(
        min: Option<&str>,
        min_inclusive: bool,
        max: Option<&str>,
        max_inclusive: bool,
    ) -> VersionRange {
        VersionRange {
            min: min.map(str::to_string),
            min_inclusive,
            max: max.map(str::to_string),
            max_inclusive,
        }
    }

    #[test]
    fn semver_specs_turn_into_ranges() {
        for (spec, expected) in [
            ("*", VersionRange::default()),
            (">=1.2", VersionRange::at_least("1.2")),
            (">1.2", range(Some("1.2"), false, None, false)),
            ("<=2", range(None, false, Some("2"), true)),
            ("<2", range(None, false, Some("2"), false)),
            ("^1.2.3", range(Some("1.2.3"), true, Some("2-"), false)),
            ("~1.2.3", range(Some("1.2.3"), true, Some("1.3-"), false)),
            ("1.2.x", range(Some("1.2-"), true, Some("1.3-"), false)),
            ("=1.2", VersionRange::exactly("1.2")),
            ("1.2+build", VersionRange::exactly("1.2+build")),
            (">=1 <2", range(Some("1"), true, Some("2"), false)),
            (">=2.0 >=1.0", VersionRange::at_least("2.0")),
            (">=1.0 >=2.0", VersionRange::at_least("2.0")),
            (">2 >=2", range(Some("2"), false, None, false)),
            ("<3 <=2 <4", range(None, false, Some("2"), true)),
            (">=1 =1.5", VersionRange::exactly("1.5")),
        ] {
            assert_eq!(semver_range(spec), Some(expected), "{spec}");
        }
    }

    #[test]
    fn a_caret_on_text_only_takes_that_text() {
        use crate::packages::dependencies::semver_rules_out;

        let ranges = [semver_range("^beta").unwrap()];
        assert!(!semver_rules_out(&ranges, "beta"));
        assert!(semver_rules_out(&ranges, "2.0"));
    }

    #[test]
    fn maven_specs_turn_into_ranges() {
        assert_eq!(
            maven_ranges("[1.0,2.0)"),
            Some(vec![range(Some("1.0"), true, Some("2.0"), false)])
        );
        assert_eq!(
            maven_ranges("[1.0,)"),
            Some(vec![VersionRange::at_least("1.0")])
        );
        assert_eq!(
            maven_ranges("(,2.0]"),
            Some(vec![range(None, false, Some("2.0"), true)])
        );
        assert_eq!(
            maven_ranges("[1.0]"),
            Some(vec![VersionRange::exactly("1.0")])
        );
        assert_eq!(
            maven_ranges("[1,2),[3,)"),
            Some(vec![
                range(Some("1"), true, Some("2"), false),
                VersionRange::at_least("3"),
            ])
        );
    }

    #[test]
    fn a_bare_maven_version_limits_nothing() {
        assert_eq!(maven_ranges("1.0"), None);
        assert_eq!(maven_ranges("*"), None);
    }

    #[test]
    fn manifests_keep_the_versions_they_require() {
        let fabric = parse_dependencies(
            FABRIC,
            r#"{"id": "app", "depends": {"Lib": [">=2", "1.0"], "any": "*", "odd": 1}}"#,
        )
        .unwrap();
        assert_eq!(
            fabric.ranges.get("lib"),
            Some(&vec![
                VersionRange::at_least("2"),
                VersionRange::exactly("1.0"),
            ])
        );
        assert_eq!(
            fabric.ranges.get("any"),
            Some(&vec![VersionRange::default()])
        );
        assert_eq!(fabric.ranges.get("odd"), None);

        let quilt = parse_dependencies(
            QUILT,
            r#"{"quilt_loader": {"id": "app", "depends": [
                {"id": "lib", "versions": ">=2"},
                {"id": "alt", "versions": {"any": [">=1"]}},
                {"id": "opt", "versions": ">=9", "optional": true}
            ]}}"#,
        )
        .unwrap();
        assert_eq!(
            quilt.ranges.get("lib"),
            Some(&vec![VersionRange::at_least("2")])
        );
        assert_eq!(
            quilt.ranges.get("alt"),
            Some(&vec![VersionRange::at_least("1")])
        );
        assert_eq!(quilt.ranges.get("opt"), None);

        let forge = parse_dependencies(
            NEOFORGE,
            r#"
            [[mods]]
            modId = "app"
            [[dependencies.app]]
            modId = "lib"
            type = "required"
            versionRange = "[2,)"
            [[dependencies.app]]
            modId = "opt"
            type = "optional"
            versionRange = "[9,)"
            "#,
        )
        .unwrap();
        assert_eq!(
            forge.ranges.get("lib"),
            Some(&vec![VersionRange::at_least("2")])
        );
        assert_eq!(forge.ranges.get("opt"), None);

        let legacy = parse_dependencies(
            LEGACY_FORGE,
            r#"[{"modid": "app", "requiredMods": ["lib@[2,)", "plain"]}]"#,
        )
        .unwrap();
        assert_eq!(legacy.requires, ["lib", "plain"]);
        assert_eq!(
            legacy.ranges.get("lib"),
            Some(&vec![VersionRange::at_least("2")])
        );
    }

    #[test]
    fn each_forge_mod_keeps_its_own_version() {
        let parsed = parse_dependencies(
            FORGE,
            r#"
            [[mods]]
            modId = "app"
            version = "2.0"
            [[mods]]
            modId = "Library"
            version = "1.0"
            [[mods]]
            modId = "same"
            version = "2.0"
            [[mods]]
            modId = "filled"
            version = "${file.jarVersion}"
            [[mods]]
            modId = "bare"
            "#,
        )
        .unwrap();

        assert_eq!(parsed.version.as_deref(), Some("2.0"));
        assert_eq!(
            parsed.alias_versions,
            BTreeMap::from([
                ("bare".to_string(), "1".to_string()),
                ("filled".to_string(), "${file.jarVersion}".to_string()),
                ("library".to_string(), "1.0".to_string()),
            ])
        );
    }

    #[test]
    fn each_legacy_forge_mod_keeps_its_own_version() {
        let parsed = parse_dependencies(
            LEGACY_FORGE,
            r#"[{"modid": "app", "version": "2.0"}, {"modid": "library", "version": "1.0"}]"#,
        )
        .unwrap();

        assert_eq!(
            parsed.alias_versions,
            BTreeMap::from([("library".to_string(), "1.0".to_string())])
        );
    }

    #[test]
    fn only_a_loaders_own_list_declares_nested_jars() {
        let fabric =
            r#"{"jars": [{"file": "custom/a.jar"}, {"file": "./b.jar"}, {"file": "../c.jar"}]}"#;
        assert_eq!(declared_jars(FABRIC, fabric), ["custom/a.jar", "b.jar"]);

        let quilt = r#"{"quilt_loader": {"jars": ["custom/a.jar"]}}"#;
        assert_eq!(declared_jars(QUILT, quilt), ["custom/a.jar"]);

        let jar_in_jar = r#"{"jars": [{"path": "META-INF/jarjar/a.jar"}]}"#;
        assert_eq!(
            declared_jars(JAR_IN_JAR, jar_in_jar),
            ["META-INF/jarjar/a.jar"]
        );

        assert!(declared_jars(FORGE, "[[mods]]").is_empty());
    }

    #[test]
    fn quilt_keeps_the_version_of_a_provided_id() {
        let parsed = parse_dependencies(
            QUILT,
            r#"{"quilt_loader": {"id": "app", "version": "5.0", "provides": [
                {"id": "Library", "version": "2.0"},
                {"id": "built", "version": "${version}"},
                {"id": "plain"},
                "bare"
            ]}}"#,
        )
        .unwrap();

        assert_eq!(
            parsed.provides,
            ["app", "library", "built", "plain", "bare"]
        );
        assert_eq!(
            parsed.alias_versions,
            BTreeMap::from([("library".to_string(), "2.0".to_string())])
        );
    }

    #[test]
    fn a_loader_reads_only_its_own_manifest() {
        let all = [FABRIC, QUILT, NEOFORGE, FORGE];
        for (loader, expected) in [
            (GameLoader::Fabric, FABRIC),
            (GameLoader::Ornithe, FABRIC),
            (GameLoader::Quilt, QUILT),
            (GameLoader::NeoForge, NEOFORGE),
            (GameLoader::Forge, FORGE),
        ] {
            assert_eq!(manifests_read(all.to_vec(), Some(loader)), [expected]);
        }

        assert_eq!(
            manifests_read(vec![FABRIC, FORGE], Some(GameLoader::Quilt)),
            [FABRIC],
            "Quilt loads Fabric mods"
        );
        assert_eq!(
            manifests_read(vec![LEGACY_FORGE, FORGE], Some(GameLoader::Forge)),
            [FORGE],
            "the priority is the loader's, not the jar's order"
        );
        assert_eq!(
            manifests_read(vec![FABRIC, QUILT], Some(GameLoader::Forge)),
            [FABRIC, QUILT]
        );
        assert_eq!(manifests_read(all.to_vec(), None), all);
    }

    #[test]
    fn fabric_dependencies_list_depends_keys() {
        let parsed = parse_dependencies(
            FABRIC,
            r#"{"id":"Sodium-Extra","provides":["extras"],"depends":{"sodium":"*","fabric-api":"*"},"recommends":{"modmenu":"*"}}"#,
        )
        .unwrap();
        assert_eq!(parsed.provides, ["sodium-extra", "extras"]);
        // `depends` is a JSON object, so its keys come back in whatever order
        // serde_json's map keeps, which depends on whether `preserve_order` is
        // enabled anywhere in the build. The caller sorts them anyway.
        let mut requires = parsed.requires;
        requires.sort();
        assert_eq!(requires, ["fabric-api", "sodium"]);
    }

    #[test]
    fn quilt_skips_optional_dependencies() {
        let parsed = parse_dependencies(
            QUILT,
            r#"{"quilt_loader":{"id":"a","depends":["b",{"id":"c"},{"id":"d","optional":true}]}}"#,
        )
        .unwrap();
        assert_eq!(parsed.provides, ["a"]);
        assert_eq!(parsed.requires, ["b", "c"]);
    }

    #[test]
    fn forge_reads_mandatory_and_typed_dependencies() {
        let parsed = parse_dependencies(
            FORGE,
            r#"
            [[mods]]
            modId = "examplemod2"

            [[dependencies.examplemod2]]
            modId = "forge"
            mandatory = true

            [[dependencies.examplemod2]]
            modId = "jei"
            mandatory = false

            [[dependencies.examplemod2]]
            modId = "cloth_config"
            type = "required"

            [[dependencies.examplemod2]]
            modId = "emi"
            type = "optional"
            "#,
        )
        .unwrap();
        assert_eq!(parsed.provides, ["examplemod2"]);
        assert_eq!(parsed.requires, ["forge", "cloth_config"]);
    }

    #[test]
    fn neoforge_reads_incompatible_dependencies_as_bans() {
        let parsed = parse_dependencies(
            NEOFORGE,
            r#"
            [[mods]]
            modId = "app"

            [[dependencies.app]]
            modId = "Library"
            type = "incompatible"
            versionRange = "[2.0,3.0),[4.0,)"

            [[dependencies.app]]
            modId = "library"
            type = "INCOMPATIBLE"
            versionRange = "[1.0]"

            [[dependencies.app]]
            modId = "anyversion"
            type = "incompatible"
            versionRange = "*"

            [[dependencies.app]]
            modId = "norange"
            type = "incompatible"

            [[dependencies.app]]
            modId = "serveronly"
            type = "incompatible"
            side = "SERVER"

            [[dependencies.app]]
            modId = "warned"
            type = "discouraged"
            "#,
        )
        .unwrap();

        assert_eq!(parsed.requires, Vec::<String>::new());
        assert_eq!(parsed.breaks["library"].len(), 3);
        assert_eq!(parsed.breaks["anyversion"], [VersionRange::default()]);
        assert_eq!(parsed.breaks["norange"], [VersionRange::default()]);
        assert_eq!(parsed.breaks.len(), 3);
    }

    #[test]
    fn legacy_forge_strips_version_from_required_mods() {
        let parsed = parse_dependencies(
            LEGACY_FORGE,
            r#"[{"modid":"skyblockaddons","requiredMods":["oneconfig@[1.0,)"]}]"#,
        )
        .unwrap();
        assert_eq!(parsed.provides, ["skyblockaddons"]);
        assert_eq!(parsed.requires, ["oneconfig"]);
    }
}
