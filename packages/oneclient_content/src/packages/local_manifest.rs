use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

const MAX_DESCRIPTION: usize = 600;

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

		let parsed = match wanted {
			FABRIC => parse_fabric(text),
			QUILT => parse_quilt(text),
			NEOFORGE | FORGE => parse_forge(text),
			_ => parse_legacy_forge(text),
		};

		match parsed {
			Some(parsed) => out.fill_from(parsed),
			None => tracing::debug!("{wanted} in {} did not parse", jar.display()),
		}
	}

	out
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
		metadata: Option<QuiltMetadata>,
	}

	#[derive(Deserialize)]
	struct QuiltMetadata {
		name: Option<String>,
		description: Option<String>,
		#[serde(default)]
		contributors: BTreeMap<String, serde_json::Value>,
		icon: Option<Icon>,
	}

	let parsed: QuiltMod = serde_json::from_str(text).ok()?;
	let metadata = parsed.quilt_loader.metadata?;

	Some(JarManifest {
		name: clean(metadata.name),
		description: clean_description(metadata.description),
		authors: metadata
			.contributors
			.into_keys()
			.filter_map(|name| clean(Some(name)))
			.collect(),
		icon_entry: metadata.icon.and_then(Icon::path).and_then(entry_path),
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
			Self::Many(list) => list.into_iter().filter_map(|name| clean(Some(name))).collect(),
		}
	}
}

fn clean(value: Option<String>) -> Option<String> {
	let value = value?.trim().to_string();
	(!value.is_empty()).then_some(value)
}

fn clean_description(value: Option<String>) -> Option<String> {
	let mut text = clean(value)?.split_whitespace().collect::<Vec<_>>().join(" ");
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

#[cfg(test)]
mod tests {
	use super::*;

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
		assert_eq!(parsed.icon_entry.as_deref(), Some("assets/example/icon.png"));
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
		assert_eq!(parsed.description.as_deref(), Some("A mod that spans lines"));
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
}
