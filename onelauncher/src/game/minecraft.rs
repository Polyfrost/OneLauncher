use core::fmt;
use std::{collections::HashMap, marker::PhantomData};

use serde::{de::{self, SeqAccess, Visitor}, Deserialize, Deserializer, Serialize};

use crate::constants;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftVersion {
	pub id: String,
	pub url: String,
	#[serde(default)]
	pub release_type: ReleaseType,
	#[serde(default)]
	pub release_time: chrono::DateTime<chrono::Utc>,
}

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
    pub minecraft_arguments: Option<String>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<ModernArguments>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModernArguments {
    pub game: Vec<ModernArgumentsItem>,
    pub jvm: Vec<ModernArgumentsItem>,
}

pub trait ModernArgumentsExt {
    fn build(&self) -> String;
}

impl ModernArgumentsExt for ModernArguments {
    fn build(&self) -> String {
        // TODO: Implement the custom rules
        let mut builder = String::new();

        for item in &self.game {
            match item {
                ModernArgumentsItem::Simple(s) => {
                    builder.push_str(s);
                    builder.push(' ');
                },

                ModernArgumentsItem::Rule(_) => {}
            }
        }

        for item in &self.jvm {
            match item {
                ModernArgumentsItem::Simple(s) => {
                    builder.push_str(s);
                    builder.push(' ');
                },

                ModernArgumentsItem::Rule(_) => {}
            }
        }

        builder
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum ModernArgumentsItem {
    Simple(String),
    Rule(ModernArgumentRuleItem),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModernArgumentRuleItem {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<Rule>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(deserialize_with = "string_or_seq")]
    pub value: Vec<String>,
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
    fn default() -> Self { ReleaseType::Release }
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
    pub downloads: LibraryDownload,
    pub name: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub rules: Vec<Rule>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub natives: Option<HashMap<String, String>>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryDownload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact: Option<Artifact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
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
    pub features: Option<HashMap<String, bool>>,
}

pub trait RuleExt {
    fn check(&self) -> bool;
}

pub trait RuleListExt {
    fn check(&self) -> bool;
}

impl RuleListExt for Vec<Rule> {
    fn check(&self) -> bool {
        for rule in self {
            if !rule.check() {
                return false;
            }
        }

        true
    }
}

impl RuleExt for Rule {
    fn check(&self) -> bool {
        if let Some(os) = &self.os {
            match &self.action {
                RuleAction::Allow => {
                    // os name check
                    if let Some(name) = &os.name {
                        if name != constants::TARGET_OS {
                            return false;
                        }
                    }

                    // TODO: os version check
                    // os version check

                    // TODO: os arch check
                    // os arch check
                },

                RuleAction::Disallow => {
                    // os name check
                    if let Some(name) = &os.name {
                        if name == constants::TARGET_OS {
                            return false;
                        }
                    }
                }
            }
        }
        
        true
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RuleAction {
    #[serde(rename = "allow")]
    Allow,
    #[serde(rename = "disallow")]
    Disallow,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Os {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
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

fn string_or_seq<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringOrVec(PhantomData<Vec<String>>);

    impl<'de> Visitor<'de> for StringOrVec {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("string or list of strings")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(vec![value.into()])
        }

        fn visit_seq<S>(self, seq: S) -> Result<Self::Value, S::Error>
        where
            S: SeqAccess<'de>,
        {
            Deserialize::deserialize(de::value::SeqAccessDeserializer::new(seq))
        }
    }

    deserializer.deserialize_any(StringOrVec(PhantomData))
}