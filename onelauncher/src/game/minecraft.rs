use core::fmt;
use std::{collections::HashMap, marker::PhantomData};

use serde::{de::{self, SeqAccess, Visitor}, Deserialize, Deserializer, Serialize};
use serde_json::Value;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftManifest {
	pub asset_index: AssetIndex,
	pub downloads: Downloads,
	pub java_version: JavaVersion,
	pub libraries: Vec<Library>,
	pub logging: Logging,
	pub main_class: String,
	pub release_time: String,
	#[serde(rename = "id")]
	pub version: String,
	#[serde(rename = "type")]
	pub release_type: ReleaseType,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub minecraft_arguments
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum Arguments {
	// TODO: https://serde.rs/string-or-struct.html
	MinecraftArguments(String),
	Arguments {
		game: Vec<ModernArgumentsItem>,
		jvm: Vec<ModernArgumentsItem>,
	},
}

impl Default for Arguments {
	fn default() -> Self {
		Arguments::MinecraftArguments(String::new())
	}
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum ModernArgumentsItem {
	Simple(String),
	Rule {
		rules: Vec<Rule>,
		value: ArgumentRuleValue,
	},
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum ArgumentRuleValue {
	String(String),
	List(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReleaseType {
	Snapshot,
	Release,
	OldBeta,
	OldAlpha,
}

impl Default for ReleaseType {
	fn default() -> Self {
		ReleaseType::Release
	}
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndex {
	pub id: String,
	pub sha1: String,
	pub url: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Downloads {
	pub client: Client,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Client {
	pub sha1: String,
	pub url: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
	pub major_version: u8,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Library {
	pub downloads: Downloads2,
	pub name: String,
	#[serde(default)]
	pub rules: Vec<Rule>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub natives: Option<Natives>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Downloads2 {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub artifact: Option<Artifact>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub classifiers: Option<Classifiers>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
	pub path: String,
	pub sha1: String,
	pub url: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Classifiers {
	#[serde(skip_serializing_if = "Option::is_none")]
	#[serde(rename = "natives-osx")]
	pub natives_osx: Option<Native>,

	#[serde(skip_serializing_if = "Option::is_none")]
	#[serde(rename = "natives-linux")]
	pub natives_linux: Option<Native>,

	#[serde(skip_serializing_if = "Option::is_none")]
	#[serde(rename = "natives-windows")]
	pub natives_windows: Option<Native>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Native {
	pub path: String,
	pub sha1: String,
	pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
	pub action: RuleAction,

	#[serde(skip_serializing_if = "Option::is_none")]
	pub os: Option<Os>,

	#[serde(skip_serializing_if = "Option::is_none")]
	pub features: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum RuleAction {
	Allow,
	Disallow,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Os {
	pub name: Option<String>,
	pub arch: Option<String>,
	pub version: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Natives {
	pub windows: Option<String>,
	pub linux: Option<String>,
	pub osx: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Logging {
	pub client: Client2,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Client2 {
	pub argument: String,
	pub file: File,
	#[serde(rename = "type")]
	pub type_field: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct File {
	pub id: String,
	pub sha1: String,
	pub url: String,
}
