use interpulse::api::minecraft::{Os, OsRule, Rule, RuleAction};
use regex::Regex;

use crate::utils::os_ext::OsExt;

#[tracing::instrument(level = "debug")]
pub fn validate_rules(rules: &[Rule], java_arch: &str, updated: bool) -> bool {
	let mut rule = rules
		.iter()
		.map(|r| validate_rule(r, java_arch, updated))
		.collect::<Vec<Option<bool>>>();
	if rules
		.iter()
		.all(|r| matches!(r.action, RuleAction::Disallow))
	{
		rule.push(Some(true));
	}

	!(rule.iter().any(|r| r == &Some(false)) || rule.iter().all(Option::is_none))
}

/// Parses a Minecraft library feature or OS rule.
/// Is disallowed -> Don't include it
/// Is not allowed -> Don't include it
/// Is allowed -> Include it
#[tracing::instrument(level = "debug")]
pub fn validate_rule(rule: &Rule, java_arch: &str, updated: bool) -> Option<bool> {
	let result = match rule {
		Rule { os: Some(os), .. } => validate_os_rule(os, java_arch, updated),
		Rule {
			features: Some(features),
			..
		} => {
			!features.is_demo_user.unwrap_or(true)
				|| features.has_custom_resolution.unwrap_or(false)
				|| !features.has_quick_plays_support.unwrap_or(true)
				|| !features.is_quick_play_multiplayer.unwrap_or(true)
				|| !features.is_quick_play_realms.unwrap_or(true)
				|| !features.is_quick_play_singleplayer.unwrap_or(true)
		}
		_ => return Some(true),
	};

	match rule.action {
		RuleAction::Allow => {
			if result {
				Some(true)
			} else {
				Some(false)
			}
		}
		RuleAction::Disallow => {
			if result {
				Some(false)
			} else {
				None
			}
		}
	}
}

#[must_use]
pub fn validate_os_rule(rule: &OsRule, java_arch: &str, updated: bool) -> bool {
	let mut rule_match = true;

	if let Some(ref arch) = rule.arch {
		rule_match &= !matches!(arch.as_str(), "x86" | "arm");
	}

	if let Some(name) = &rule.name {
		if updated && (name != &Os::LinuxArm64 || name != &Os::LinuxArm32) {
			rule_match &= &Os::native() == name || &Os::native_arch(java_arch) == name;
		} else {
			rule_match &= &Os::native_arch(java_arch) == name;
		}
	}

	if let Some(version) = &rule.version
		&& let Ok(regex) = Regex::new(version.as_str())
	{
		rule_match &= regex.is_match(&sysinfo::System::os_version().unwrap_or_default());
	}

	rule_match
}
