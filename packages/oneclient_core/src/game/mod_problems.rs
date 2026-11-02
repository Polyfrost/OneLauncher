use oneclient_events::{CrashMod, CrashModAction, CrashModIssue, CrashModIssueKind, VersionRange};

pub(crate) const MAX_PROBLEMS: usize = 256;
const MAX_DESCRIBED: usize = 4;

const MINECRAFT_NAME: &str = "Minecraft";

const PLATFORMS: [(&str, &str); 10] = [
    ("minecraft", "Minecraft"),
    ("mcp", "Minecraft"),
    ("java", "Java"),
    ("fabricloader", "Fabric Loader"),
    ("fabric loader", "Fabric Loader"),
    ("forge", "Forge"),
    ("minecraft forge", "Forge"),
    ("fml", "Forge"),
    ("javafml", "Forge"),
    ("neoforge", "NeoForge"),
];

pub(crate) const INSTALL_ADVICE: &str =
    "Install what they ask for, or remove the mods that need it.";
pub(crate) const PLATFORM_ADVICE: &str =
    "Use a version of the mod made for this cluster, or remove it from the cluster.";
pub(crate) const INCOMPATIBLE_ADVICE: &str =
    "Remove one mod from each incompatible pair, or update them to versions that work together.";

const MOD_LOAD_MARKERS: [&str; 11] = [
    "WrongMinecraftVersionException",
    "MissingModsException",
    "Missing or unsupported mandatory dependencies",
    "ModLoadingException",
    "net.fabricmc.loader.impl.FormattedException",
    "Incompatible mod set!",
    "Incompatible mods found!",
    "Mod resolution encountered an incompatible mod set",
    "Could not find required mod: ",
    "requires mods [",
    "Forge Mod Loader has found a problem with your minecraft installation",
];

const FABRIC_API_ID: &str = "fabric-api";
const FABRIC_API_NAME: &str = "Fabric API";
const FABRIC_API_LEGACY_ID: &str = "fabric";
const FABRIC_API_BASE: &str = "fabric-api-base";
const FABRIC_REQUIRES: &str = " requires ";
const FABRIC_MISSING_SUFFIX: &str = ", which is missing!";
const FABRIC_WRONG_VERSION: &str = ", but only the wrong version is present: ";
const FABRIC_INCOMPATIBLE: &str = " is incompatible with ";
const FABRIC_MATCHING_PRESENT: &str = ", but a matching version is present: ";
const FABRIC_LEGACY_MARKER: &str = "Could not find required mod: ";
const FORGE_LEGACY_LISTS: [&str; 2] = ["requires mods [", "requires ["];
const FORGE_MOD_ID: &str = "Mod ID: '";
const FORGE_REQUESTED_BY: &str = "Requested by: '";
const FORGE_EXPECTED_RANGE: &str = "Expected range: '";
const FORGE_ACTUAL_VERSION: &str = "Actual version: '";
const FORGE_MISSING_VERSION: &str = "[MISSING]";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModRef {
    pub name: String,
    pub id: Option<String>,
    pub version: Option<String>,
}

impl ModRef {
    #[must_use]
    pub fn named(name: &str) -> Self {
        let name = name.trim();

        Self {
            name: name.to_string(),
            id: (!name.contains(char::is_whitespace)).then(|| name.to_string()),
            version: None,
        }
    }

    #[must_use]
    pub fn with_id(name: &str, id: &str) -> Self {
        Self {
            name: name.trim().to_string(),
            id: Some(id.trim().to_string()),
            version: None,
        }
    }

    #[must_use]
    pub fn at(mut self, version: Option<&str>) -> Self {
        self.version = version
            .map(str::trim)
            .filter(|version| !version.is_empty())
            .map(str::to_string);
        self
    }

    fn platform(&self) -> Option<&'static str> {
        let matches = |text: &str| {
            let text = text.trim().to_ascii_lowercase();
            PLATFORMS
                .iter()
                .find(|(known, _)| *known == text)
                .map(|(_, name)| *name)
        };

        self.id
            .as_deref()
            .and_then(matches)
            .or_else(|| matches(&self.name))
    }

    fn display_name(&self) -> String {
        self.platform()
            .map_or_else(|| self.name.clone(), str::to_string)
    }

    fn key(&self) -> String {
        self.id
            .as_deref()
            .unwrap_or(&self.name)
            .trim()
            .to_ascii_lowercase()
    }

    fn row(&self, note: String) -> CrashMod {
        CrashMod {
            name: self.display_name(),
            id: self.id.clone(),
            version: self.version.clone(),
            note,
            hash: None,
            bundled_in: None,
            action: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModProblemKind {
    Missing,
    WrongVersion,
    Incompatible,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModProblem {
    pub kind: ModProblemKind,
    pub required_by: Option<ModRef>,
    pub dependency: ModRef,
    pub wanted: Option<String>,
}

impl ModProblem {
    fn is_platform(&self) -> bool {
        self.kind != ModProblemKind::Incompatible && self.dependency.platform().is_some()
    }

    fn needed(&self) -> String {
        let name = self.dependency.display_name();

        match &self.wanted {
            Some(wanted) => format!("{name} {wanted}"),
            None => name,
        }
    }

    fn requirer(&self) -> Option<String> {
        self.required_by.as_ref().map(|by| by.display_name())
    }

    fn describe(&self) -> String {
        let needed = self.needed();
        let present = self.dependency.version.as_deref();

        if self.kind == ModProblemKind::Incompatible {
            let other = match present {
                Some(version) => format!("{} {version}", self.dependency.display_name()),
                None => self.dependency.display_name(),
            };

            return match self.requirer() {
                Some(by) => format!("{by} does not work with {other}"),
                None => format!("{other} is incompatible with another mod"),
            };
        }

        if self.is_platform() {
            let present = present
                .map(|present| format!(", but this cluster has {present}"))
                .unwrap_or_default();

            return match self.requirer() {
                Some(by) => format!("{by} is made for {needed}{present}"),
                None => format!("{needed} is required{present}"),
            };
        }

        match (self.requirer(), present) {
            (Some(by), Some(present)) => format!("{by} needs {needed}, but {present} is installed"),
            (Some(by), None) => format!("{by} needs {needed}"),
            (None, Some(present)) => format!("{needed} is required, but {present} is installed"),
            (None, None) => format!("{needed} is missing"),
        }
    }

    pub(crate) fn same_as(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.dependency.key() == other.dependency.key()
            && self.required_by.as_ref().map(ModRef::key)
                == other.required_by.as_ref().map(ModRef::key)
    }

    fn mirrors(&self, other: &Self) -> bool {
        self.kind == ModProblemKind::Incompatible
            && other.kind == ModProblemKind::Incompatible
            && other
                .required_by
                .as_ref()
                .is_some_and(|by| by.key() == self.dependency.key())
            && self
                .required_by
                .as_ref()
                .is_some_and(|by| by.key() == other.dependency.key())
    }
}

#[must_use]
pub fn problems_title(problems: &[ModProblem]) -> String {
    let Some(first) = problems.first() else {
        return "A mod stopped the game from starting".to_string();
    };

    let name = first.dependency.display_name();

    if problems.len() > 1 {
        let shared = problems.iter().all(|other| {
            other.kind == first.kind && other.dependency.key() == first.dependency.key()
        });
        let all = |kind: ModProblemKind| problems.iter().all(|other| other.kind == kind);
        let any_incompatible = problems
            .iter()
            .any(|other| other.kind == ModProblemKind::Incompatible);

        return match (shared, first.kind) {
            (true, ModProblemKind::Incompatible) => {
                format!("{} mods are incompatible with {name}", problems.len())
            }
            (true, _) if first.is_platform() => format!(
                "{} mods are not made for this version of {name}",
                problems.len()
            ),
            (true, _) => format!("{} mods need {name}", problems.len()),
            (false, _) if all(ModProblemKind::Incompatible) => {
                "Some mods are incompatible with each other".to_string()
            }
            (false, _) if any_incompatible => "Some mods cannot be loaded together".to_string(),
            (false, _) if problems.iter().all(ModProblem::is_platform) => {
                "Some mods are not made for this cluster".to_string()
            }
            (false, _) => "Some mods are missing what they depend on".to_string(),
        };
    }

    let by = first.requirer();

    if first.kind == ModProblemKind::Incompatible {
        return match by {
            Some(by) => format!("{by} is incompatible with {name}"),
            None => format!("{name} is incompatible with another mod"),
        };
    }

    if first.is_platform() {
        return match by {
            Some(by) => format!("{by} is not made for this version of {name}"),
            None => format!("This version of {name} is not supported"),
        };
    }

    match (by, first.kind) {
        (Some(by), ModProblemKind::WrongVersion) => {
            format!("{by} needs a different version of {name}")
        }
        (Some(by), _) => format!("{by} needs {name}"),
        (None, ModProblemKind::WrongVersion) => format!("The wrong version of {name} is installed"),
        (None, _) => format!("{name} is missing"),
    }
}

#[must_use]
pub fn problems_body(problems: &[ModProblem]) -> String {
    let mut sentences: Vec<String> = problems
        .iter()
        .take(MAX_DESCRIBED)
        .map(|problem| format!("{}.", problem.describe()))
        .collect();

    let hidden = problems.len().saturating_sub(MAX_DESCRIBED);
    if hidden > 0 {
        sentences.push(format!("And {hidden} more."));
    }

    if problems
        .iter()
        .any(|problem| problem.kind != ModProblemKind::Incompatible && !problem.is_platform())
    {
        sentences.push(INSTALL_ADVICE.to_string());
    }
    if problems.iter().any(ModProblem::is_platform) {
        sentences.push(PLATFORM_ADVICE.to_string());
    }
    if problems
        .iter()
        .any(|problem| problem.kind == ModProblemKind::Incompatible)
    {
        sentences.push(INCOMPATIBLE_ADVICE.to_string());
    }

    sentences.join(" ")
}

#[must_use]
pub fn problem_issues(problems: &[ModProblem]) -> Vec<CrashModIssue> {
    let mut issues: Vec<(ModProblemKind, String, CrashModIssue)> = Vec::new();

    for (index, problem) in problems.iter().enumerate() {
        if problems[..index]
            .iter()
            .any(|earlier| earlier.mirrors(problem))
        {
            continue;
        }

        let key = problem.dependency.key();
        let source = problem.required_by.as_ref().map(|by| {
            let note = match problem.kind {
                ModProblemKind::Incompatible => {
                    format!("Incompatible with {}", problem.dependency.display_name())
                }
                _ if problem.is_platform() => format!("Made for {}", problem.needed()),
                _ => format!("Needs {}", problem.needed()),
            };
            by.row(note)
        });

        if let Some((_, _, issue)) = issues
            .iter_mut()
            .find(|(kind, known, _)| *kind == problem.kind && *known == key)
        {
            issue.sources.extend(source);
            continue;
        }

        let note = match problem.kind {
            ModProblemKind::Missing => "Not installed".to_string(),
            ModProblemKind::WrongVersion if problem.is_platform() => "This cluster".to_string(),
            ModProblemKind::WrongVersion => "Wrong version".to_string(),
            ModProblemKind::Incompatible => "Incompatible".to_string(),
        };

        issues.push((
            problem.kind,
            key,
            CrashModIssue {
                kind: match problem.kind {
                    ModProblemKind::Missing => CrashModIssueKind::Missing,
                    ModProblemKind::WrongVersion => CrashModIssueKind::WrongVersion,
                    ModProblemKind::Incompatible => CrashModIssueKind::Incompatible,
                },
                target: CrashMod {
                    action: (problem.kind == ModProblemKind::Missing && !problem.is_platform())
                        .then(|| CrashModAction::Install {
                            mod_id: problem.dependency.key(),
                            range: problem.wanted.as_deref().and_then(wanted_range),
                        }),
                    ..problem.dependency.row(note)
                },
                sources: source.into_iter().collect(),
            },
        ));
    }

    issues.into_iter().map(|(_, _, issue)| issue).collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstalledJar {
    pub hash: String,
    pub enabled: bool,
    pub name: Option<String>,
    pub id: Option<String>,
    pub version: Option<String>,
}

impl InstalledJar {
    fn is(&self, row: &CrashMod) -> bool {
        let same = |a: &str, b: &str| a.trim().eq_ignore_ascii_case(b.trim());

        match (&row.id, &self.id) {
            (Some(row_id), Some(id)) => same(row_id, id),
            _ => self
                .name
                .as_deref()
                .or(self.id.as_deref())
                .is_some_and(|name| same(name, &row.name)),
        }
    }
}

fn is_platform_row(row: &CrashMod) -> bool {
    ModRef {
        name: row.name.clone(),
        id: row.id.clone(),
        version: None,
    }
    .platform()
    .is_some()
}

fn is_unmatched(row: &CrashMod) -> bool {
    row.hash.is_none() && row.bundled_in.is_none() && !is_platform_row(row)
}

pub(crate) fn has_unmatched(issues: &[CrashModIssue]) -> bool {
    issues
        .iter()
        .any(|issue| is_unmatched(&issue.target) || issue.sources.iter().any(is_unmatched))
}

pub(crate) fn has_unmatched_missing(issues: &[CrashModIssue]) -> bool {
    issues
        .iter()
        .any(|issue| issue.kind == CrashModIssueKind::Missing && is_unmatched(&issue.target))
}

pub(crate) fn attach_jar(issues: &mut [CrashModIssue], jar: &InstalledJar) {
    let fill = |row: &mut CrashMod| {
        if !is_unmatched(row) || !jar.is(row) {
            return None;
        }

        row.hash = Some(jar.hash.clone());
        if row.version.is_none() {
            row.version.clone_from(&jar.version);
        }

        Some(jar.enabled)
    };

    for issue in issues {
        let enabled = fill(&mut issue.target);
        if issue.kind == CrashModIssueKind::Missing {
            match enabled {
                Some(false) => {
                    issue.target.note = "Disabled".to_string();
                    issue.target.action = Some(CrashModAction::Enable {
                        hash: jar.hash.clone(),
                    });
                }
                Some(true) => issue.target.action = None,
                None => {}
            }
        }

        for source in &mut issue.sources {
            fill(source);
        }
    }
}

pub(crate) fn attach_bundled(
    issues: &mut [CrashModIssue],
    container: &InstalledJar,
    bundled: &InstalledJar,
) {
    let container_name = container
        .name
        .clone()
        .or_else(|| container.id.clone())
        .unwrap_or_else(|| "another mod".to_string());

    let fill = |row: &mut CrashMod| {
        if !is_unmatched(row) || !bundled.is(row) {
            return false;
        }

        row.bundled_in = Some(container_name.clone());
        if row.version.is_none() {
            row.version.clone_from(&bundled.version);
        }

        true
    };

    for issue in issues {
        if fill(&mut issue.target) && issue.kind == CrashModIssueKind::Missing {
            if container.enabled {
                issue.target.note = format!("Inside {container_name}");
                issue.target.action = None;
            } else {
                issue.target.note = format!("Inside {container_name}, disabled");
                issue.target.action = Some(CrashModAction::Enable {
                    hash: container.hash.clone(),
                });
            }
        }

        for source in &mut issue.sources {
            fill(source);
        }
    }
}

#[cfg(test)]
pub(crate) fn attach_installed(issues: &mut [CrashModIssue], installed: &[InstalledJar]) {
    for jar in installed {
        attach_jar(issues, jar);
    }
}

pub(crate) fn is_mod_load_line(line: &str) -> bool {
    MOD_LOAD_MARKERS.iter().any(|marker| line.contains(marker))
        || line.contains(FORGE_MOD_ID)
        || is_fabric_requirement_line(line)
        || is_fabric_incompatibility_line(line)
}

fn is_fabric_requirement_line(line: &str) -> bool {
    line.contains(FABRIC_REQUIRES)
        && (line.contains(FABRIC_MISSING_SUFFIX) || line.contains(FABRIC_WRONG_VERSION))
}

fn is_fabric_incompatibility_line(line: &str) -> bool {
    line.contains(FABRIC_INCOMPATIBLE) && line.contains(FABRIC_MATCHING_PRESENT)
}

#[must_use]
pub fn problems_in(line: &str) -> Vec<ModProblem> {
    if !is_mod_load_line(line) {
        return Vec::new();
    }

    let single = fabric_requirement(line)
        .or_else(|| fabric_incompatibility(line))
        .or_else(|| fabric_legacy_requirement(line))
        .or_else(|| forge_requirement(line));

    match single {
        Some(found) => vec![found],
        None => forge_legacy_requirements(line),
    }
    .into_iter()
    .map(fold_fabric_api)
    .collect()
}

fn is_minecraft_version(problem: &ModProblem) -> bool {
    problem.kind != ModProblemKind::Incompatible
        && problem.dependency.platform() == Some(MINECRAFT_NAME)
}

pub(crate) fn without_rejected_builds(mut problems: Vec<ModProblem>) -> Vec<ModProblem> {
    if problems
        .iter()
        .any(|problem| !is_minecraft_version(problem))
    {
        problems.retain(|problem| !is_minecraft_version(problem));
    }

    problems
}

#[must_use]
pub fn wanted_range(wanted: &str) -> Option<VersionRange> {
    let wanted = wanted.trim();
    let open_ended = |min: Option<&str>, min_inclusive, max: Option<&str>, max_inclusive| {
        Some(VersionRange {
            min: min.map(str::to_string),
            min_inclusive,
            max: max.map(str::to_string),
            max_inclusive,
        })
    };

    if let Some(rest) = wanted.strip_prefix("between ") {
        let (low, high) = rest.split_once(" and ")?;
        let bound = |text: &str| {
            let inclusive = text.contains("(inclusive)");
            let version = text
                .replace("(inclusive)", "")
                .replace("(exclusive)", "")
                .trim()
                .to_string();
            (version, inclusive)
        };
        let ((low, low_inclusive), (high, high_inclusive)) = (bound(low), bound(high));
        return open_ended(Some(&low), low_inclusive, Some(&high), high_inclusive);
    }

    for (prefix, inclusive) in [(">=", true), (">", false), ("<=", true), ("<", false)] {
        if let Some(version) = wanted.strip_prefix(prefix) {
            let version = version.trim();
            return if prefix.starts_with('>') {
                open_ended(Some(version), inclusive, None, false)
            } else {
                open_ended(None, false, Some(version), inclusive)
            };
        }
    }

    if let Some(version) = wanted.strip_prefix("newer than ") {
        return open_ended(Some(version), false, None, false);
    }
    if let Some(version) = wanted.strip_prefix("older than ") {
        return open_ended(None, false, Some(version), false);
    }
    if let Some(version) = wanted.strip_suffix(" or later") {
        return open_ended(Some(version), true, None, false);
    }
    if let Some(version) = wanted.strip_suffix(" or earlier") {
        return open_ended(None, false, Some(version), true);
    }
    if let Some((low, high)) = wanted.split_once(" to below ") {
        return open_ended(Some(low), true, Some(high), false);
    }
    if let Some((low, high)) = wanted.split_once(" to ") {
        return open_ended(Some(low), true, Some(high), true);
    }

    let exact = wanted.trim_start_matches('=').trim();
    (!exact.is_empty() && !exact.contains(char::is_whitespace))
        .then(|| VersionRange::exactly(exact))
}

fn is_fabric_api_part(key: &str) -> bool {
    if key == FABRIC_API_LEGACY_ID || key == FABRIC_API_BASE {
        return true;
    }

    key.starts_with("fabric-")
        && key.rsplit_once("-v").is_some_and(|(_, revision)| {
            !revision.is_empty() && revision.chars().all(|c| c.is_ascii_digit())
        })
}

fn fold_fabric_api(mut problem: ModProblem) -> ModProblem {
    let key = problem.dependency.key();

    if key == FABRIC_API_ID {
        problem.dependency.name = FABRIC_API_NAME.to_string();
    } else if is_fabric_api_part(&key) {
        problem.dependency = ModRef::with_id(FABRIC_API_NAME, FABRIC_API_ID);
        problem.wanted = None;
    }

    problem
}

fn fabric_requirer(head: &str) -> Option<ModRef> {
    let quoted = match head.split_once("Mod '") {
        Some((_, quoted)) => quoted,
        None => head
            .trim_start_matches(|c: char| c == '-' || c.is_whitespace())
            .strip_prefix('\'')?,
    };

    fabric_mod(quoted)
}

fn fabric_mod(quoted: &str) -> Option<ModRef> {
    if let Some((name, rest)) = quoted.split_once("' (") {
        let (id, version) = rest.split_once(')').unwrap_or((rest, ""));
        let found = ModRef::with_id(name, id).at(Some(version));

        return (!found.name.is_empty()).then_some(found);
    }

    let (name, _) = quoted.split_once('\'')?;
    let name = name.trim();

    (!name.is_empty()).then(|| ModRef {
        name: name.to_string(),
        id: None,
        version: None,
    })
}

fn fabric_target(spec: &str) -> Option<(Option<String>, ModRef)> {
    let (version, dependency) = spec.split_once(" of ")?;
    let dependency = dependency.trim();
    let dependency = dependency.strip_prefix("mod ").unwrap_or(dependency);

    let dependency = match dependency.strip_prefix('\'') {
        Some(quoted) => fabric_mod(quoted)?.at(None),
        None => ModRef::named(dependency),
    };

    let version = version.trim();
    let wanted = (version != "any version").then(|| fabric_version(version));

    (!dependency.name.is_empty()).then_some((wanted, dependency))
}

fn fabric_present(text: &str) -> Option<String> {
    let present = text.trim().trim_end_matches('!').trim();

    (!present.is_empty()).then(|| present.to_string())
}

fn fabric_requirement(line: &str) -> Option<ModProblem> {
    let (head, rest) = line.split_once(FABRIC_REQUIRES)?;
    let required_by = fabric_requirer(head)?;

    let (spec, kind, present) =
        if let Some(spec) = rest.trim_end().strip_suffix(FABRIC_MISSING_SUFFIX) {
            (spec, ModProblemKind::Missing, None)
        } else {
            let (spec, present) = rest.split_once(FABRIC_WRONG_VERSION)?;
            (spec, ModProblemKind::WrongVersion, fabric_present(present))
        };

    let (wanted, dependency) = fabric_target(spec)?;

    Some(ModProblem {
        kind,
        required_by: Some(required_by),
        dependency: dependency.at(present.as_deref()),
        wanted,
    })
}

fn fabric_incompatibility(line: &str) -> Option<ModProblem> {
    let (head, rest) = line.split_once(FABRIC_INCOMPATIBLE)?;
    let required_by = fabric_requirer(head)?;
    let (spec, present) = rest.split_once(FABRIC_MATCHING_PRESENT)?;

    let (wanted, dependency) = match fabric_target(spec) {
        Some(found) => found,
        None => (None, ModRef::named(spec)),
    };

    Some(ModProblem {
        kind: ModProblemKind::Incompatible,
        required_by: Some(required_by),
        dependency: dependency.at(fabric_present(present).as_deref()),
        wanted,
    })
}

fn fabric_version(version: &str) -> String {
    if let Some(after) = version.strip_prefix("any version after ") {
        return format!("newer than {after}");
    }

    if let Some(before) = version.strip_prefix("any version before ") {
        return format!("older than {before}");
    }

    version
        .strip_prefix("any version ")
        .or_else(|| version.strip_prefix("version "))
        .unwrap_or(version)
        .to_string()
}

fn fabric_legacy_requirement(line: &str) -> Option<ModProblem> {
    let (_, rest) = line.split_once(FABRIC_LEGACY_MARKER)?;
    let (required_by, spec) = rest.split_once(FABRIC_REQUIRES)?;

    let spec = spec.trim().trim_start_matches('{');
    let spec = spec.split('}').next().unwrap_or(spec);
    let (dependency, range) = spec.split_once(" @ ").unwrap_or((spec, ""));

    let range = range
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim();

    let dependency = dependency.trim();
    let required_by = required_by.trim();
    if dependency.is_empty() {
        return None;
    }

    Some(ModProblem {
        kind: ModProblemKind::Missing,
        required_by: (!required_by.is_empty()).then(|| ModRef::with_id(required_by, required_by)),
        dependency: ModRef::with_id(dependency, dependency),
        wanted: (!range.is_empty() && range != "*").then(|| range.to_string()),
    })
}

fn forge_requirement(line: &str) -> Option<ModProblem> {
    let dependency = quoted_after(line, FORGE_MOD_ID)?;
    let present = quoted_after(line, FORGE_ACTUAL_VERSION).filter(|v| v != FORGE_MISSING_VERSION);

    Some(ModProblem {
        kind: if present.is_some() {
            ModProblemKind::WrongVersion
        } else {
            ModProblemKind::Missing
        },
        required_by: quoted_after(line, FORGE_REQUESTED_BY).map(|by| ModRef::with_id(&by, &by)),
        dependency: ModRef::with_id(&dependency, &dependency).at(present.as_deref()),
        wanted: quoted_after(line, FORGE_EXPECTED_RANGE).and_then(|range| maven_range(&range)),
    })
}

fn forge_legacy_requirements(line: &str) -> Vec<ModProblem> {
    let Some((head, list)) = FORGE_LEGACY_LISTS
        .iter()
        .find_map(|marker| line.split_once(marker))
    else {
        return Vec::new();
    };

    let Some(owner) = head.rfind("Mod ").or_else(|| head.rfind("mod ")) else {
        return Vec::new();
    };

    let owner = head[owner + 4..].trim();
    let required_by = match owner.split_once('(') {
        Some((id, name)) if !name.trim_end_matches(')').trim().is_empty() => {
            ModRef::with_id(name.trim_end_matches(')'), id)
        }
        Some((id, _)) => ModRef::with_id(id, id),
        None => ModRef::with_id(owner, owner),
    };

    top_level_entries(list)
        .into_iter()
        .filter_map(|entry| {
            let (dependency, range) = entry.split_once('@').unwrap_or((entry, ""));
            let dependency = dependency.trim();

            (!dependency.is_empty()).then(|| ModProblem {
                kind: ModProblemKind::Missing,
                required_by: (!required_by.name.is_empty()).then(|| required_by.clone()),
                dependency: ModRef::with_id(dependency, dependency),
                wanted: maven_range(range),
            })
        })
        .take(MAX_PROBLEMS)
        .collect()
}

fn quoted_after(text: &str, marker: &str) -> Option<String> {
    let (_, rest) = text.split_once(marker)?;
    let (value, _) = rest.split_once('\'')?;
    let value = value.trim();

    (!value.is_empty()).then(|| value.to_string())
}

fn top_level_entries(list: &str) -> Vec<&str> {
    let mut entries = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;

    for (index, ch) in list.char_indices() {
        match ch {
            '[' | '(' => depth += 1,
            ']' | ')' if depth == 0 => {
                entries.push(&list[start..index]);
                return entries
                    .into_iter()
                    .map(str::trim)
                    .filter(|entry| !entry.is_empty())
                    .collect();
            }
            ']' | ')' => depth -= 1,
            ',' if depth == 0 => {
                entries.push(&list[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }

    Vec::new()
}

fn maven_range(range: &str) -> Option<String> {
    let range = range.trim();
    if range.is_empty() || range == "*" {
        return None;
    }

    let inclusive_low = range.starts_with('[');
    let inclusive_high = range.ends_with(']');
    let inner = range
        .trim_start_matches(['[', '('])
        .trim_end_matches([']', ')']);

    let Some((low, high)) = inner.split_once(',') else {
        let exact = inner.trim();
        return (!exact.is_empty()).then(|| exact.to_string());
    };

    let (low, high) = (low.trim(), high.trim());

    Some(match (low.is_empty(), high.is_empty()) {
        (true, true) => return None,
        (false, true) if inclusive_low => format!("{low} or later"),
        (false, true) => format!("newer than {low}"),
        (true, false) if inclusive_high => format!("{high} or earlier"),
        (true, false) => format!("older than {high}"),
        (false, false) if inclusive_high => format!("{low} to {high}"),
        (false, false) => format!("{low} to below {high}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    type Brief = (
        ModProblemKind,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    );

    fn brief(problems: &[ModProblem]) -> Vec<Brief> {
        problems
            .iter()
            .map(|problem| {
                (
                    problem.kind,
                    problem.dependency.name.clone(),
                    problem.required_by.as_ref().map(|by| by.name.clone()),
                    problem.wanted.clone(),
                    problem.dependency.version.clone(),
                )
            })
            .collect()
    }

    fn expect(
        kind: ModProblemKind,
        dependency: &str,
        by: Option<&str>,
        wanted: Option<&str>,
        present: Option<&str>,
    ) -> Brief {
        (
            kind,
            dependency.to_string(),
            by.map(str::to_string),
            wanted.map(str::to_string),
            present.map(str::to_string),
        )
    }

    fn problem(
        kind: ModProblemKind,
        dependency: &str,
        by: Option<&str>,
        wanted: Option<&str>,
        present: Option<&str>,
    ) -> ModProblem {
        ModProblem {
            kind,
            required_by: by.map(ModRef::named),
            dependency: ModRef::named(dependency).at(present),
            wanted: wanted.map(str::to_string),
        }
    }

    use ModProblemKind::{Incompatible, Missing, WrongVersion};

    #[test]
    fn fabric_names_a_missing_dependency() {
        let line = "\t - Mod 'Sodium Extra' (sodium-extra) 0.5.1 requires any version of \
                    fabric-api, which is missing!";

        assert_eq!(
            brief(&problems_in(line)),
            vec![expect(
                Missing,
                "Fabric API",
                Some("Sodium Extra"),
                None,
                None
            )]
        );
    }

    #[test]
    fn fabric_keeps_the_requiring_mods_id_and_version() {
        let line = " - 'Iris' (iris) 1.8.0 requires any version after 0.6.0 of Sodium, \
                    which is missing!";
        let found = problems_in(line);

        assert_eq!(
            found[0].required_by,
            Some(ModRef::with_id("Iris", "iris").at(Some("1.8.0")))
        );
        assert_eq!(
            brief(&found),
            vec![expect(
                Missing,
                "Sodium",
                Some("Iris"),
                Some("newer than 0.6.0"),
                None
            )]
        );
    }

    #[test]
    fn fabric_names_a_required_version_and_the_one_present() {
        let missing = "\t - Mod 'Iris' (iris) 1.6.11 requires version 0.5.4 or later of \
                       sodium, which is missing!";
        assert_eq!(
            brief(&problems_in(missing)),
            vec![expect(
                Missing,
                "sodium",
                Some("Iris"),
                Some("0.5.4 or later"),
                None
            )]
        );

        let wrong = "\t - 'Fabric API' (fabric-api) 0.120.0 requires version 26.3 of \
                     'Minecraft' (minecraft), but only the wrong version is present: 26.2!";
        let found = problems_in(wrong);
        assert_eq!(found[0].dependency.id.as_deref(), Some("minecraft"));
        assert_eq!(
            brief(&found),
            vec![expect(
                WrongVersion,
                "Minecraft",
                Some("Fabric API"),
                Some("26.3"),
                Some("26.2")
            )]
        );
    }

    #[test]
    fn fabric_names_both_mods_of_an_incompatible_pair() {
        let line = "\t - 'Sodium' (sodium) 0.6.0 is incompatible with any version of \
                    'OptiFabric' (optifabric), but a matching version is present: 1.14.3!";

        assert_eq!(
            problems_in(line),
            vec![ModProblem {
                kind: Incompatible,
                required_by: Some(ModRef::with_id("Sodium", "sodium").at(Some("0.6.0"))),
                dependency: ModRef::with_id("OptiFabric", "optifabric").at(Some("1.14.3")),
                wanted: None,
            }]
        );
    }

    #[test]
    fn fabric_keeps_apostrophes_in_mod_names() {
        let line = "\t - Mod 'Xaero's Minimap' (xaerominimap) 24.0 requires any version of \
                    mod 'Xaero's Lib' (xaerolib), which is missing!";

        assert_eq!(
            brief(&problems_in(line)),
            vec![expect(
                Missing,
                "Xaero's Lib",
                Some("Xaero's Minimap"),
                None,
                None
            )]
        );
    }

    #[test]
    fn fabric_warnings_are_not_problems() {
        for line in [
            " - 'Minecraft' (minecraft) 26.3 recommends any version after 15.0.0-alpha.5 of \
             Fabulously Optimized, which is missing!",
            " - 'Sodium Extra' (sodium-extra) 0.6.0 recommends version 1.0 of \
             'Reese's Sodium Options' (reeses-sodium-options), but only the wrong version is \
             present: 0.9!",
            " - 'Sodium' (sodium) 0.6.0 conflicts with any version of 'Embeddium' (embeddium), \
             but a matching version is present: 1.0!",
        ] {
            assert!(!is_mod_load_line(line), "{line}");
            assert!(problems_in(line).is_empty(), "{line}");
        }
    }

    #[test]
    fn legacy_fabric_names_a_missing_dependency() {
        let line = "net.fabricmc.loader.discovery.ModResolutionException: Could not find \
                    required mod: modmenu requires {fabric @ [>=0.29.0]}";

        assert_eq!(
            brief(&problems_in(line)),
            vec![expect(Missing, "Fabric API", Some("modmenu"), None, None)]
        );
    }

    #[test]
    fn legacy_forge_lists_every_missing_mod() {
        let line = "[main/FATAL] [FML]: The mod skyblockaddons (SkyblockAddons) requires mods \
                    [essential, oneconfig@[1.0,)] to be available";
        let found = problems_in(line);

        assert_eq!(
            found[0].required_by,
            Some(ModRef::with_id("SkyblockAddons", "skyblockaddons"))
        );
        assert_eq!(
            brief(&found),
            vec![
                expect(Missing, "essential", Some("SkyblockAddons"), None, None),
                expect(
                    Missing,
                    "oneconfig",
                    Some("SkyblockAddons"),
                    Some("1.0 or later"),
                    None
                ),
            ]
        );
    }

    #[test]
    fn the_legacy_forge_exception_is_parsed_the_same_way() {
        let line = "net.minecraftforge.fml.common.MissingModsException: Mod skyblockaddons \
                    (SkyblockAddons) requires [oneconfig@[1.0,2.0)]";

        assert_eq!(
            brief(&problems_in(line)),
            vec![expect(
                Missing,
                "oneconfig",
                Some("SkyblockAddons"),
                Some("1.0 to below 2.0"),
                None
            )]
        );
    }

    #[test]
    fn modern_forge_names_the_dependency_and_its_requester() {
        let missing = "\tMod ID: 'architectury', Requested by: 'rei', Expected range: \
                       '[9.1.12,)', Actual version: '[MISSING]'";
        assert_eq!(
            brief(&problems_in(missing)),
            vec![expect(
                Missing,
                "architectury",
                Some("rei"),
                Some("9.1.12 or later"),
                None
            )]
        );

        let wrong = "\tMod ID: 'forge', Requested by: 'jei', Expected range: '[47.1,)', \
                     Actual version: '47.0.3'";
        assert_eq!(
            brief(&problems_in(wrong)),
            vec![expect(
                WrongVersion,
                "forge",
                Some("jei"),
                Some("47.1 or later"),
                Some("47.0.3")
            )]
        );
    }

    #[test]
    fn ordinary_lines_name_no_problem() {
        for line in [
            "[main/INFO]: Mod 'Sodium' (sodium) requires a restart",
            "[main/INFO]: This mod requires [nothing] special",
            "Loading 42 mods",
        ] {
            assert!(problems_in(line).is_empty(), "{line}");
        }
    }

    #[test]
    fn a_single_missing_dependency_is_named_in_the_title() {
        let problems = vec![problem(
            Missing,
            "fabric-api",
            Some("Sodium Extra"),
            None,
            None,
        )];

        assert_eq!(problems_title(&problems), "Sodium Extra needs fabric-api");
        assert!(problems_body(&problems).starts_with("Sodium Extra needs fabric-api."));
    }

    #[test]
    fn several_mods_needing_one_dependency_share_a_title() {
        let problems = vec![
            problem(Missing, "fabric-api", Some("Indium"), None, None),
            problem(Missing, "fabric-api", Some("Sodium Extra"), None, None),
        ];

        assert_eq!(problems_title(&problems), "2 mods need fabric-api");
    }

    #[test]
    fn a_wrong_version_is_described_with_what_is_installed() {
        let problems = vec![problem(
            WrongVersion,
            "Sodium",
            Some("Iris"),
            Some("0.5.4 or later"),
            Some("0.5.1"),
        )];

        assert_eq!(
            problems_title(&problems),
            "Iris needs a different version of Sodium"
        );
        assert_eq!(
            problems_body(&problems),
            format!("Iris needs Sodium 0.5.4 or later, but 0.5.1 is installed. {INSTALL_ADVICE}")
        );
    }

    #[test]
    fn a_mod_made_for_another_minecraft_is_told_apart_from_a_missing_mod() {
        let problems = vec![problem(
            WrongVersion,
            "minecraft",
            Some("Fabric API"),
            Some("1.20.1"),
            Some("1.20.4"),
        )];

        assert_eq!(
            problems_title(&problems),
            "Fabric API is not made for this version of Minecraft"
        );
        assert_eq!(
            problems_body(&problems),
            format!(
                "Fabric API is made for Minecraft 1.20.1, but this cluster has 1.20.4. \
                 {PLATFORM_ADVICE}"
            )
        );
    }

    #[test]
    fn an_incompatible_pair_is_named_in_the_title() {
        let problems = vec![problem(
            Incompatible,
            "OptiFabric",
            Some("Sodium"),
            None,
            Some("1.14.3"),
        )];

        assert_eq!(
            problems_title(&problems),
            "Sodium is incompatible with OptiFabric"
        );
        assert_eq!(
            problems_body(&problems),
            format!("Sodium does not work with OptiFabric 1.14.3. {INCOMPATIBLE_ADVICE}")
        );
    }

    #[test]
    fn mixed_problems_get_every_piece_of_advice() {
        let problems = vec![
            problem(Missing, "fabric-api", Some("Indium"), None, None),
            problem(
                WrongVersion,
                "minecraft",
                Some("Iris"),
                Some("1.20.1"),
                Some("1.20.4"),
            ),
            problem(Incompatible, "OptiFabric", Some("Sodium"), None, None),
        ];

        let body = problems_body(&problems);
        assert!(body.contains(INSTALL_ADVICE));
        assert!(body.contains(PLATFORM_ADVICE));
        assert!(body.contains(INCOMPATIBLE_ADVICE));
        assert_eq!(
            problems_title(&problems),
            "Some mods cannot be loaded together"
        );
    }

    #[test]
    fn a_long_list_is_cut_short_in_the_body() {
        let problems: Vec<ModProblem> = (0..6)
            .map(|n| problem(Missing, &format!("dep{n}"), Some("Foo"), None, None))
            .collect();

        let body = problems_body(&problems);
        assert!(body.contains("dep3"));
        assert!(!body.contains("dep4"));
        assert!(body.contains("And 2 more."));
    }

    #[test]
    fn issues_group_every_mod_needing_the_same_dependency() {
        let problems = vec![
            problem(Missing, "fabric-api", Some("Indium"), None, None),
            problem(Missing, "fabric-api", Some("Sodium Extra"), None, None),
        ];
        let issues = problem_issues(&problems);

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].kind, CrashModIssueKind::Missing);
        assert_eq!(issues[0].target.name, "fabric-api");
        assert_eq!(issues[0].target.note, "Not installed");

        let sources: Vec<(&str, &str)> = issues[0]
            .sources
            .iter()
            .map(|row| (row.name.as_str(), row.note.as_str()))
            .collect();
        assert_eq!(
            sources,
            vec![
                ("Indium", "Needs fabric-api"),
                ("Sodium Extra", "Needs fabric-api")
            ]
        );
    }

    #[test]
    fn an_incompatible_pair_reported_both_ways_is_one_issue() {
        let sodium = ModRef::with_id("Sodium", "sodium").at(Some("0.6.0"));
        let optifabric = ModRef::with_id("OptiFabric", "optifabric").at(Some("1.14.3"));
        let problems = vec![
            ModProblem {
                kind: Incompatible,
                required_by: Some(sodium.clone()),
                dependency: optifabric.clone(),
                wanted: None,
            },
            ModProblem {
                kind: Incompatible,
                required_by: Some(optifabric),
                dependency: sodium,
                wanted: None,
            },
        ];
        let issues = problem_issues(&problems);

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].kind, CrashModIssueKind::Incompatible);
        assert_eq!(issues[0].target.name, "OptiFabric");
        assert_eq!(issues[0].target.version.as_deref(), Some("1.14.3"));
        assert_eq!(issues[0].sources[0].name, "Sodium");
        assert_eq!(issues[0].sources[0].version.as_deref(), Some("0.6.0"));
        assert_eq!(issues[0].sources[0].note, "Incompatible with OptiFabric");
    }

    fn jar(hash: &str, enabled: bool, name: &str, id: &str, version: &str) -> InstalledJar {
        InstalledJar {
            hash: hash.to_string(),
            enabled,
            name: Some(name.to_string()),
            id: Some(id.to_string()),
            version: Some(version.to_string()),
        }
    }

    #[test]
    fn rows_pick_up_the_installed_jar_by_id() {
        let line = " - 'Iris' (iris) 1.8.0 requires any version of sodium, which is missing!";
        let mut issues = problem_issues(&problems_in(line));

        attach_installed(
            &mut issues,
            &[
                jar("iris-hash", true, "Iris Shaders", "iris", "1.8.0+mc26.3"),
                jar("sodium-hash", false, "Sodium", "sodium", "0.6.13"),
            ],
        );

        assert_eq!(issues[0].target.hash.as_deref(), Some("sodium-hash"));
        assert_eq!(issues[0].target.note, "Disabled");
        assert_eq!(issues[0].target.version.as_deref(), Some("0.6.13"));
        assert_eq!(issues[0].sources[0].hash.as_deref(), Some("iris-hash"));
        assert_eq!(issues[0].sources[0].version.as_deref(), Some("1.8.0"));
    }

    #[test]
    fn a_dependency_named_without_an_id_is_matched_by_name() {
        let mut issues = problem_issues(&[problem(
            Missing,
            "Fabulously Optimized",
            Some("Minecraft"),
            None,
            None,
        )]);

        attach_installed(
            &mut issues,
            &[jar(
                "fo-hash",
                false,
                "Fabulously Optimized",
                "fabulously_optimized",
                "15.0",
            )],
        );

        assert_eq!(issues[0].target.hash.as_deref(), Some("fo-hash"));
    }

    #[test]
    fn a_mod_that_is_not_installed_keeps_no_hash() {
        let line = " - 'Iris' (iris) 1.8.0 requires any version of sodium, which is missing!";
        let mut issues = problem_issues(&problems_in(line));
        attach_installed(&mut issues, &[]);

        assert_eq!(issues[0].target.hash, None);
        assert_eq!(issues[0].target.note, "Not installed");
    }

    #[test]
    fn fabric_api_parts_are_folded_into_fabric_api() {
        for part in [
            "fabric-rendering-v1",
            "fabric-key-binding-api-v1",
            "fabric-resource-loader-v0",
            "fabric-api-base",
        ] {
            let line = format!(
                " - 'Iris' (iris) 1.8.0 requires version 1.2.0 or later of {part}, which is missing!"
            );
            let found = problems_in(&line);

            assert_eq!(
                found[0].dependency,
                ModRef::with_id("Fabric API", "fabric-api"),
                "{part}"
            );
            assert_eq!(found[0].wanted, None, "{part}");
        }
    }

    #[test]
    fn mods_that_are_not_fabric_api_parts_keep_their_own_id() {
        for id in ["fabric-language-kotlin", "fabricloader", "fabric-api"] {
            let line =
                format!(" - 'Foo' (foo) 1.0 requires any version of {id}, which is missing!");
            assert_eq!(
                problems_in(&line)[0].dependency.id.as_deref(),
                Some(id),
                "{id}"
            );
        }
    }

    #[test]
    fn several_fabric_api_parts_from_one_mod_become_one_problem() {
        let watch = crate::game::diagnosis::CrashWatch::new();
        watch.observe("[main/ERROR]: Incompatible mods found!");
        for part in ["fabric-rendering-v1", "fabric-key-binding-api-v1"] {
            watch.observe(&format!(
                " - 'Iris' (iris) 1.8.0 requires any version of {part}, which is missing!"
            ));
        }

        let crash = watch.take().expect("a mod load failure");
        assert_eq!(crash.title(), "Iris needs Fabric API");
        assert_eq!(crash.mod_issues().len(), 1);
    }

    #[test]
    fn a_mod_that_is_not_installed_offers_an_install() {
        let line = " - 'Iris' (iris) 1.8.0 requires any version of sodium, which is missing!";
        let mut issues = problem_issues(&problems_in(line));
        attach_installed(&mut issues, &[]);

        assert_eq!(
            issues[0].target.action,
            Some(CrashModAction::Install {
                mod_id: "sodium".to_string(),
                range: None,
            })
        );
        assert_eq!(issues[0].sources[0].action, None);
    }

    #[test]
    fn a_disabled_dependency_offers_an_enable() {
        let line = " - 'Iris' (iris) 1.8.0 requires any version of sodium, which is missing!";
        let mut issues = problem_issues(&problems_in(line));
        attach_installed(
            &mut issues,
            &[jar("sodium-hash", false, "Sodium", "sodium", "0.6.13")],
        );

        assert_eq!(
            issues[0].target.action,
            Some(CrashModAction::Enable {
                hash: "sodium-hash".to_string()
            })
        );
    }

    #[test]
    fn an_enabled_dependency_offers_nothing() {
        let line = " - 'Iris' (iris) 1.8.0 requires any version of sodium, which is missing!";
        let mut issues = problem_issues(&problems_in(line));
        attach_installed(
            &mut issues,
            &[jar("sodium-hash", true, "Sodium", "sodium", "0.6.13")],
        );

        assert_eq!(issues[0].target.action, None);
    }

    #[test]
    fn wrong_versions_platforms_and_incompatibilities_offer_no_install() {
        for problem in [
            problem(
                WrongVersion,
                "Sodium",
                Some("Iris"),
                Some("0.6.0 or later"),
                Some("0.5.1"),
            ),
            problem(Missing, "minecraft", Some("Fabric API"), Some("26.3"), None),
            problem(
                Incompatible,
                "OptiFabric",
                Some("Sodium"),
                None,
                Some("1.14.3"),
            ),
        ] {
            let issues = problem_issues(std::slice::from_ref(&problem));
            assert_eq!(issues[0].target.action, None, "{problem:?}");
        }
    }

    const BOP_NEEDS_TERRABLENDER: &str = "\t - Mod 'BiomesOPlenty' (biomesoplenty) 26.3.0.0.15 requires \
         version 26.3.0.0.6 or later of terrablender, which is missing!";

    const INPUT_FIXES_FOR_OLD_MINECRAFT: &str = "\t - Mod 'MacOS Input Fixes' (macos_input_fixes) \
         1.16+1.21.11 requires any version before 26.1 of 'Minecraft' (minecraft), but only the \
         wrong version is present: 26.3!";

    #[test]
    fn a_rejected_build_for_another_minecraft_is_dropped_next_to_a_real_problem() {
        let watch = crate::game::diagnosis::CrashWatch::new();
        watch.observe("[13:05:28] [main/ERROR]: Incompatible mods found!");
        watch.observe("More details:");
        watch.observe(BOP_NEEDS_TERRABLENDER);
        watch.observe(INPUT_FIXES_FOR_OLD_MINECRAFT);

        let crash = watch.take().expect("a mod load failure");
        assert_eq!(crash.title(), "BiomesOPlenty needs terrablender");

        let issues = crash.mod_issues();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].target.name, "terrablender");
        assert!(!crash.body().contains("MacOS Input Fixes"));
    }

    #[test]
    fn a_mod_made_for_another_minecraft_is_still_shown_on_its_own() {
        let watch = crate::game::diagnosis::CrashWatch::new();
        watch.observe("[13:05:28] [main/ERROR]: Incompatible mods found!");
        watch.observe(INPUT_FIXES_FOR_OLD_MINECRAFT);

        let crash = watch.take().expect("a mod load failure");
        assert_eq!(
            crash.title(),
            "MacOS Input Fixes is not made for this version of Minecraft"
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
    fn readable_versions_turn_back_into_ranges() {
        for (wanted, expected) in [
            (
                "0.6.0 or later",
                Some(range(Some("0.6.0"), true, None, false)),
            ),
            (
                "newer than 0.6.0",
                Some(range(Some("0.6.0"), false, None, false)),
            ),
            (
                "older than 26.1",
                Some(range(None, false, Some("26.1"), false)),
            ),
            (
                "1.2 or earlier",
                Some(range(None, false, Some("1.2"), true)),
            ),
            (
                "1.0 to below 2.0",
                Some(range(Some("1.0"), true, Some("2.0"), false)),
            ),
            (
                "1.0 to 2.0",
                Some(range(Some("1.0"), true, Some("2.0"), true)),
            ),
            (
                "between 0.6.0 (inclusive) and 0.7 (exclusive)",
                Some(range(Some("0.6.0"), true, Some("0.7"), false)),
            ),
            (">=0.29.0", Some(range(Some("0.29.0"), true, None, false))),
            ("26.3.0.0.6", Some(VersionRange::exactly("26.3.0.0.6"))),
            ("any 1.2.x version", None),
        ] {
            assert_eq!(wanted_range(wanted), expected, "{wanted}");
        }
    }

    #[test]
    fn the_install_action_carries_the_wanted_range() {
        let line = "\t - Mod 'BiomesOPlenty' (biomesoplenty) 26.3.0.0.15 requires version \
                    26.3.0.0.6 or later of terrablender, which is missing!";
        let issues = problem_issues(&problems_in(line));

        assert_eq!(
            issues[0].target.action,
            Some(CrashModAction::Install {
                mod_id: "terrablender".to_string(),
                range: Some(VersionRange::at_least("26.3.0.0.6")),
            })
        );
    }

    #[test]
    fn java_and_loader_complaints_survive_next_to_a_missing_mod() {
        let problems = vec![
            problem(Missing, "terrablender", Some("BiomesOPlenty"), None, None),
            problem(
                WrongVersion,
                "fabricloader",
                Some("Sodium"),
                Some("0.16.0 or later"),
                Some("0.15.11"),
            ),
            problem(
                WrongVersion,
                "java",
                Some("Iris"),
                Some("21 or later"),
                Some("17"),
            ),
            problem(
                WrongVersion,
                "minecraft",
                Some("MacOS Input Fixes"),
                Some("older than 26.1"),
                Some("26.3"),
            ),
        ];

        let kept: Vec<String> = without_rejected_builds(problems)
            .into_iter()
            .map(|problem| problem.dependency.name)
            .collect();
        assert_eq!(kept, vec!["terrablender", "fabricloader", "java"]);
    }

    #[test]
    fn a_mod_inside_an_enabled_jar_is_not_offered_for_install() {
        let mut issues = problem_issues(&[problem(
            WrongVersion,
            "cloth-config",
            Some("Foo"),
            Some("15 or later"),
            Some("14.0.0"),
        )]);
        let container = jar(
            "rei-hash",
            true,
            "Roughly Enough Items",
            "roughlyenoughitems",
            "16.0",
        );
        attach_bundled(
            &mut issues,
            &container,
            &InstalledJar {
                hash: "rei-hash".to_string(),
                enabled: true,
                name: None,
                id: Some("cloth-config".to_string()),
                version: Some("14.0.0".to_string()),
            },
        );

        assert_eq!(
            issues[0].target.bundled_in.as_deref(),
            Some("Roughly Enough Items")
        );
        assert_eq!(issues[0].target.hash, None);
        assert_eq!(issues[0].target.action, None);
        assert_eq!(issues[0].target.version.as_deref(), Some("14.0.0"));
    }

    #[test]
    fn a_missing_mod_inside_a_disabled_jar_offers_to_enable_that_jar() {
        let line = " - 'Foo' (foo) 1.0 requires any version of cloth-config, which is missing!";
        let mut issues = problem_issues(&problems_in(line));
        let container = jar(
            "rei-hash",
            false,
            "Roughly Enough Items",
            "roughlyenoughitems",
            "16.0",
        );
        attach_bundled(
            &mut issues,
            &container,
            &InstalledJar {
                hash: "rei-hash".to_string(),
                enabled: false,
                name: None,
                id: Some("cloth-config".to_string()),
                version: Some("15.0.0".to_string()),
            },
        );

        assert_eq!(
            issues[0].target.action,
            Some(CrashModAction::Enable {
                hash: "rei-hash".to_string()
            })
        );
        assert_eq!(
            issues[0].target.note,
            "Inside Roughly Enough Items, disabled"
        );
    }

    #[test]
    fn platform_rows_never_count_as_unmatched() {
        let issues = problem_issues(&[problem(
            WrongVersion,
            "minecraft",
            None,
            Some("26.2"),
            Some("26.3"),
        )]);

        assert!(!has_unmatched(&issues));
    }

    #[test]
    fn matching_stops_needing_jars_once_every_row_is_found() {
        let line = " - 'Iris' (iris) 1.8.0 requires any version of sodium, which is missing!";
        let mut issues = problem_issues(&problems_in(line));
        assert!(has_unmatched(&issues));

        attach_jar(
            &mut issues,
            &jar("iris-hash", true, "Iris", "iris", "1.8.0"),
        );
        attach_jar(
            &mut issues,
            &jar("sodium-hash", false, "Sodium", "sodium", "0.6.13"),
        );

        assert!(!has_unmatched(&issues));
    }
}
