use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use oneclient_events::CrashRemedy;

const DOCUMENT_BUDGET: usize = 96 * 1024;
const LINE_BUDGET: usize = 8 * 1024;

const EXCERPT_BEFORE: usize = 2;
const EXCERPT_AFTER: usize = 24;

const MAX_MISSING: usize = 256;
const MAX_MISSING_DESCRIBED: usize = 4;

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

const INSTALL_ADVICE: &str = "Install what they ask for, or remove the mods that need it.";
const PLATFORM_ADVICE: &str =
    "Use a version of the mod made for this cluster, or remove it from the cluster.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingDependency {
    pub dependency: String,
    pub required_by: Option<String>,
    pub version: Option<String>,
    pub present: Option<String>,
}

impl MissingDependency {
    fn platform(&self) -> Option<&'static str> {
        let id = self.dependency.trim().to_ascii_lowercase();

        PLATFORMS
            .iter()
            .find(|(known, _)| *known == id)
            .map(|(_, name)| *name)
    }

    fn display_name(&self) -> String {
        self.platform()
            .map_or_else(|| self.dependency.clone(), str::to_string)
    }

    fn needed(&self) -> String {
        let name = self.display_name();

        match &self.version {
            Some(version) => format!("{name} {version}"),
            None => name,
        }
    }

    fn describe(&self) -> String {
        let needed = self.needed();

        if self.platform().is_some() {
            let present = self
                .present
                .as_ref()
                .map(|present| format!(", but this cluster has {present}"))
                .unwrap_or_default();

            return match &self.required_by {
                Some(by) => format!("{by} is made for {needed}{present}"),
                None => format!("{needed} is required{present}"),
            };
        }

        match (&self.required_by, &self.present) {
            (Some(by), Some(present)) => format!("{by} needs {needed}, but {present} is installed"),
            (Some(by), None) => format!("{by} needs {needed}"),
            (None, Some(present)) => format!("{needed} is required, but {present} is installed"),
            (None, None) => format!("{needed} is missing"),
        }
    }

    fn same_as(&self, other: &Self) -> bool {
        self.dependency.eq_ignore_ascii_case(&other.dependency)
            && self.required_by == other.required_by
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrashDiagnosis {
    CorruptArchive { file: Option<String> },
    OutOfMemory,
    UnsupportedJava,
    ModLoadFailure { missing: Vec<MissingDependency> },
    ModLinkage,
    GraphicsDriver,
    NativeCrash { frame: Option<String> },
}

impl CrashDiagnosis {
    #[must_use]
    pub fn title(&self) -> String {
        match self {
            Self::CorruptArchive { .. } => "A damaged file crashed the game".to_string(),
            Self::OutOfMemory => "Minecraft ran out of memory".to_string(),
            Self::UnsupportedJava => "This Java version cannot run the game".to_string(),
            Self::ModLoadFailure { missing } => missing_title(missing),
            Self::ModLinkage => "A mod does not fit this version of Minecraft".to_string(),
            Self::GraphicsDriver => "The game could not start graphics".to_string(),
            Self::NativeCrash { frame: Some(frame) } => format!("{frame} crashed the game"),
            Self::NativeCrash { frame: None } => "The game crashed outside Java".to_string(),
        }
    }

    #[must_use]
    pub fn body(&self) -> String {
        match self {
            Self::CorruptArchive { file: Some(file) } => format!(
                "The game could not read {file} - the file is damaged. \
                 Verifying will re-download anything that does not match."
            ),
            Self::CorruptArchive { file: None } => "The game could not read one of its \
                 library or mod files - it is damaged. Verifying will re-download \
                 anything that does not match."
                .to_string(),
            Self::OutOfMemory => "The game asked for more memory than it was allowed. \
                 Raising the memory allocation usually fixes this, and so does running \
                 fewer mods."
                .to_string(),
            Self::UnsupportedJava => "The game was built for a different Java version than \
                 the one it launched with. Switching this cluster to the Java version it \
                 expects will fix it."
                .to_string(),
            Self::ModLoadFailure { missing } if !missing.is_empty() => missing_body(missing),
            Self::ModLoadFailure { .. } => "The mod loader refused to start. Usually one of your \
                 mods was built for a different Minecraft version, or it needs another mod that \
                 is not installed."
                .to_string(),
            Self::ModLinkage => "A mod called into code that this version of Minecraft does \
                 not have. That normally means the mod was built for another version, or two \
                 of your mods disagree about one."
                .to_string(),
            Self::GraphicsDriver => "The game could not create a graphics context. This is \
                 almost always an out-of-date or missing graphics driver rather than \
                 anything in the launcher."
                .to_string(),
            Self::NativeCrash { frame: Some(frame) } => format!(
                "The game died inside {frame}, not in Minecraft or a mod, so nothing \
                 recorded a stack trace. A graphics driver named here usually means the \
                 driver is out of date - and a rendering mod that does not match the \
                 version it was built against pushes the driver into the same failure."
            ),
            Self::NativeCrash { frame: None } => "The game died inside native code rather \
                 than in Minecraft or a mod, so nothing recorded a stack trace. This is \
                 almost always a graphics driver, an overlay hooked into the game, or a \
                 rendering mod that does not match the version it was built against."
                .to_string(),
        }
    }

    #[must_use]
    pub fn remedy(&self) -> Option<CrashRemedy> {
        match self {
            Self::CorruptArchive { .. } => Some(CrashRemedy::VerifyFiles),
            Self::OutOfMemory => Some(CrashRemedy::RaiseMemory),
            Self::UnsupportedJava => Some(CrashRemedy::OpenJavaSettings),
            Self::ModLoadFailure { .. } | Self::ModLinkage | Self::NativeCrash { .. } => {
                Some(CrashRemedy::OpenMods)
            }
            Self::GraphicsDriver => None,
        }
    }

    #[must_use]
    pub fn is_fatal(&self) -> bool {
        matches!(self, Self::ModLoadFailure { .. })
    }

    #[must_use]
    pub fn leaves_game_broken(&self) -> bool {
        matches!(self, Self::OutOfMemory)
    }
}

fn missing_title(missing: &[MissingDependency]) -> String {
    let Some(first) = missing.first() else {
        return "A mod stopped the game from starting".to_string();
    };

    let name = first.display_name();
    let platform = first.platform().is_some();

    if missing.len() > 1 {
        let shared = missing
            .iter()
            .all(|other| other.display_name().eq_ignore_ascii_case(&name));

        return match (shared, platform) {
            (true, true) => format!(
                "{} mods are not made for this version of {name}",
                missing.len()
            ),
            (true, false) => format!("{} mods need {name}", missing.len()),
            (false, _) if missing.iter().all(|other| other.platform().is_some()) => {
                "Some mods are not made for this cluster".to_string()
            }
            (false, _) => "Some mods are missing what they depend on".to_string(),
        };
    }

    match (&first.required_by, &first.present, platform) {
        (Some(by), _, true) => format!("{by} is not made for this version of {name}"),
        (None, _, true) => format!("This version of {name} is not supported"),
        (Some(by), Some(_), false) => format!("{by} needs a different version of {name}"),
        (Some(by), None, false) => format!("{by} needs {name}"),
        (None, Some(_), false) => format!("The wrong version of {name} is installed"),
        (None, None, false) => format!("{name} is missing"),
    }
}

fn missing_body(missing: &[MissingDependency]) -> String {
    let mut sentences: Vec<String> = missing
        .iter()
        .take(MAX_MISSING_DESCRIBED)
        .map(|dependency| format!("{}.", dependency.describe()))
        .collect();

    let hidden = missing.len().saturating_sub(MAX_MISSING_DESCRIBED);
    if hidden > 0 {
        sentences.push(format!("And {hidden} more."));
    }

    if missing
        .iter()
        .any(|dependency| dependency.platform().is_none())
    {
        sentences.push(INSTALL_ADVICE.to_string());
    }
    if missing
        .iter()
        .any(|dependency| dependency.platform().is_some())
    {
        sentences.push(PLATFORM_ADVICE.to_string());
    }

    sentences.join(" ")
}

const CORRUPT_ARCHIVE_MARKERS: [&str; 7] = [
    "java.util.zip.ZipException",
    "java.util.zip.ZipError",
    "Invalid or corrupt jarfile",
    "zip END header not found",
    "java.lang.ClassFormatError",
    "Invalid signature file digest",
    "Incompatible magic value",
];

const OUT_OF_MEMORY_MARKERS: [&str; 3] = [
    "java.lang.OutOfMemoryError",
    "Could not reserve enough space for",
    "There is insufficient memory for the Java Runtime Environment",
];

const UNSUPPORTED_JAVA_MARKERS: [&str; 3] = [
    "java.lang.UnsupportedClassVersionError",
    "has been compiled by a more recent version of the Java Runtime",
    "Unsupported major.minor version",
];

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

const FABRIC_MISSING_SUFFIX: &str = ", which is missing!";
const FABRIC_WRONG_VERSION: &str = ", but only the wrong version is present: ";
const FABRIC_REQUIRES: &str = " requires ";
const FABRIC_LEGACY_MARKER: &str = "Could not find required mod: ";
const FORGE_LEGACY_LISTS: [&str; 2] = ["requires mods [", "requires ["];
const FORGE_MOD_ID: &str = "Mod ID: '";
const FORGE_REQUESTED_BY: &str = "Requested by: '";
const FORGE_EXPECTED_RANGE: &str = "Expected range: '";
const FORGE_ACTUAL_VERSION: &str = "Actual version: '";
const FORGE_MISSING_VERSION: &str = "[MISSING]";

const MOD_LINKAGE_MARKERS: [&str; 3] = [
    "java.lang.NoSuchMethodError",
    "java.lang.NoSuchFieldError",
    "java.lang.AbstractMethodError",
];

const NATIVE_CRASH_MARKERS: [&str; 5] = [
    "EXCEPTION_ACCESS_VIOLATION",
    "EXCEPTION_ILLEGAL_INSTRUCTION",
    "SIGSEGV",
    "SIGBUS",
    "SIGILL",
];

const PROBLEMATIC_FRAME_MARKER: &str = "Problematic frame:";

const GRAPHICS_MARKERS: [&str; 3] = [
    "org.lwjgl.LWJGLException: Pixel format not accelerated",
    "Failed to create window",
    "GLFW error 65542",
];

#[must_use]
pub fn diagnose(line: &str) -> Option<CrashDiagnosis> {
    let hit = |markers: &[&str]| markers.iter().any(|marker| line.contains(marker));

    if hit(&CORRUPT_ARCHIVE_MARKERS) {
        return Some(CrashDiagnosis::CorruptArchive { file: jar_in(line) });
    }

    if hit(&OUT_OF_MEMORY_MARKERS) {
        return Some(CrashDiagnosis::OutOfMemory);
    }

    if hit(&UNSUPPORTED_JAVA_MARKERS) {
        return Some(CrashDiagnosis::UnsupportedJava);
    }

    if is_mod_load_line(line) {
        return Some(CrashDiagnosis::ModLoadFailure {
            missing: missing_dependencies(line),
        });
    }

    if hit(&MOD_LINKAGE_MARKERS) {
        return Some(CrashDiagnosis::ModLinkage);
    }

    if hit(&GRAPHICS_MARKERS) {
        return Some(CrashDiagnosis::GraphicsDriver);
    }

    if hit(&NATIVE_CRASH_MARKERS) {
        return Some(CrashDiagnosis::NativeCrash { frame: None });
    }

    None
}

fn is_mod_load_line(line: &str) -> bool {
    MOD_LOAD_MARKERS.iter().any(|marker| line.contains(marker))
        || line.contains(FORGE_MOD_ID)
        || is_fabric_requirement_line(line)
}

fn is_fabric_requirement_line(line: &str) -> bool {
    line.contains(FABRIC_REQUIRES)
        && (line.contains(FABRIC_MISSING_SUFFIX) || line.contains(FABRIC_WRONG_VERSION))
}

#[must_use]
pub fn missing_dependencies(line: &str) -> Vec<MissingDependency> {
    if !is_mod_load_line(line) {
        return Vec::new();
    }

    if let Some(found) = fabric_requirement(line) {
        return vec![found];
    }

    if let Some(found) = fabric_legacy_requirement(line) {
        return vec![found];
    }

    if let Some(found) = forge_requirement(line) {
        return vec![found];
    }

    forge_legacy_requirements(line)
}

fn fabric_requirement(line: &str) -> Option<MissingDependency> {
    let (head, rest) = line.split_once(FABRIC_REQUIRES)?;
    let quoted = match head.split_once("Mod '") {
        Some((_, quoted)) => quoted,
        None => head
            .trim_start_matches(|c: char| c == '-' || c.is_whitespace())
            .strip_prefix('\'')?,
    };
    let required_by = fabric_name(quoted)?;

    let (spec, present) = if let Some(spec) = rest.trim_end().strip_suffix(FABRIC_MISSING_SUFFIX) {
        (spec, None)
    } else {
        let (spec, present) = rest.split_once(FABRIC_WRONG_VERSION)?;
        let present = present.trim().trim_end_matches('!').trim();
        (spec, (!present.is_empty()).then(|| present.to_string()))
    };

    let (version, dependency) = spec.split_once(" of ")?;
    let dependency = dependency.trim();
    let dependency = dependency.strip_prefix("mod ").unwrap_or(dependency);
    let dependency = dependency
        .strip_prefix('\'')
        .and_then(fabric_name)
        .unwrap_or_else(|| dependency.to_string());

    let version = version.trim();
    let version = (version != "any version").then(|| fabric_version(version));

    named(MissingDependency {
        dependency,
        required_by: Some(required_by),
        version,
        present,
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

fn fabric_legacy_requirement(line: &str) -> Option<MissingDependency> {
    let (_, rest) = line.split_once(FABRIC_LEGACY_MARKER)?;
    let (required_by, spec) = rest.split_once(" requires ")?;

    let spec = spec.trim().trim_start_matches('{');
    let spec = spec.split('}').next().unwrap_or(spec);
    let (dependency, range) = spec.split_once(" @ ").unwrap_or((spec, ""));

    let range = range
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim();

    named(MissingDependency {
        dependency: dependency.trim().to_string(),
        required_by: Some(required_by.trim().to_string()),
        version: (!range.is_empty() && range != "*").then(|| range.to_string()),
        present: None,
    })
}

fn forge_requirement(line: &str) -> Option<MissingDependency> {
    let dependency = quoted_after(line, FORGE_MOD_ID)?;
    let present = quoted_after(line, FORGE_ACTUAL_VERSION).filter(|v| v != FORGE_MISSING_VERSION);

    named(MissingDependency {
        dependency,
        required_by: quoted_after(line, FORGE_REQUESTED_BY),
        version: quoted_after(line, FORGE_EXPECTED_RANGE).and_then(|range| maven_range(&range)),
        present,
    })
}

fn forge_legacy_requirements(line: &str) -> Vec<MissingDependency> {
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
        Some((_, name)) if !name.trim_end_matches(')').trim().is_empty() => {
            name.trim_end_matches(')').trim().to_string()
        }
        Some((id, _)) => id.trim().to_string(),
        None => owner.to_string(),
    };

    top_level_entries(list)
        .into_iter()
        .filter_map(|entry| {
            let (dependency, range) = entry.split_once('@').unwrap_or((entry, ""));

            named(MissingDependency {
                dependency: dependency.trim().to_string(),
                required_by: (!required_by.is_empty()).then(|| required_by.clone()),
                version: maven_range(range),
                present: None,
            })
        })
        .take(MAX_MISSING)
        .collect()
}

fn named(found: MissingDependency) -> Option<MissingDependency> {
    (!found.dependency.is_empty()).then_some(found)
}

fn fabric_name(quoted: &str) -> Option<String> {
    let (name, _) = quoted
        .split_once("' (")
        .or_else(|| quoted.split_once('\''))?;
    let name = name.trim();

    (!name.is_empty()).then(|| name.to_string())
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

#[must_use]
pub fn native_frame(line: &str) -> Option<String> {
    let frame = line.trim_start_matches('#').trim();

    if let Some(open) = frame.find('[') {
        let inside = frame[open + 1..].split(']').next().unwrap_or_default();
        let name = inside.split('+').next().unwrap_or(inside).trim();

        return (!name.is_empty()).then(|| name.to_string());
    }

    frame
        .split_whitespace()
        .find(|token| token.len() > 3 && token.contains('.'))
        .map(ToString::to_string)
}

fn jar_in(line: &str) -> Option<String> {
    let token = line
        .split(|c: char| c.is_whitespace() || c == '(' || c == ')')
        .find(|token| token.trim_end_matches(['.', ',', ':']).ends_with(".jar"))?;

    let token = token.trim_end_matches(['.', ',', ':']);

    let name = token
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(token)
        .to_string();

    (!name.is_empty()).then_some(name)
}

const CRASH_REPORT_MARKERS: [&str; 2] = [
    "---- Minecraft Crash Report ----",
    "# A fatal error has been detected by the Java Runtime Environment",
];

const SUSPECT_MARKER: &str = "Suspected Mods:";
const SUSPECT_NOTHING: [&str; 3] = ["unknown", "none", "n/a"];
const SUSPECT_IGNORED: [&str; 1] = ["minecraft"];
const MAX_SUSPECTS: usize = 6;

#[must_use]
pub fn suspected_mods(line: &str) -> Option<Vec<String>> {
    let (_, listed) = line.split_once(SUSPECT_MARKER)?;

    let mods: Vec<String> = listed
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .filter(|entry| !SUSPECT_NOTHING.contains(&entry.to_ascii_lowercase().as_str()))
        .filter(|entry| !suspect_id(entry).is_some_and(|id| SUSPECT_IGNORED.contains(&id.as_str())))
        .take(MAX_SUSPECTS)
        .map(ToString::to_string)
        .collect();

    (!mods.is_empty()).then_some(mods)
}

fn suspect_id(entry: &str) -> Option<String> {
    let (_, id) = entry.rsplit_once('(')?;

    Some(id.trim_end_matches(')').trim().to_ascii_lowercase())
}

fn clamp_line(line: &str) -> String {
    if line.len() <= LINE_BUDGET {
        return line.to_string();
    }

    let mut end = LINE_BUDGET;
    while end > 0 && !line.is_char_boundary(end) {
        end -= 1;
    }

    line[..end].to_string()
}

#[derive(Default)]
struct Watched {
    crash_report: bool,
    pending_frame: bool,
    suspects: Vec<String>,
    missing: Vec<MissingDependency>,
    diagnosis: Option<CrashDiagnosis>,
    excerpt: Option<Vec<String>>,
    pending_after: usize,
    lines: VecDeque<String>,
    bytes: usize,
}

impl Watched {
    fn note_crash_report(&mut self, line: &str) {
        if self.crash_report {
            return;
        }

        if CRASH_REPORT_MARKERS
            .iter()
            .any(|marker| line.contains(marker))
        {
            tracing::warn!("the game wrote a crash report");
            self.crash_report = true;
        }
    }

    fn buffer(&mut self, line: &str) {
        let kept = clamp_line(line);
        self.bytes += kept.len() + 1;
        self.lines.push_back(kept);

        while self.bytes > DOCUMENT_BUDGET && self.lines.len() > 1 {
            match self.lines.pop_front() {
                Some(dropped) => self.bytes -= dropped.len() + 1,
                None => break,
            }
        }
    }

    fn extend_excerpt(&mut self, line: &str) {
        if self.pending_after == 0 {
            return;
        }

        self.pending_after -= 1;

        if let Some(excerpt) = self.excerpt.as_mut() {
            excerpt.push(clamp_line(line));
        }
    }

    fn capture_suspects(&mut self, line: &str) {
        if !self.suspects.is_empty() {
            return;
        }

        if let Some(suspects) = suspected_mods(line) {
            tracing::warn!(?suspects, "the crash report named suspected mods");
            self.suspects = suspects;
        }
    }

    fn capture_missing(&mut self, line: &str) {
        let found = missing_dependencies(line);
        if found.is_empty() {
            return;
        }

        if self
            .diagnosis
            .as_ref()
            .is_some_and(|diagnosis| !matches!(diagnosis, CrashDiagnosis::ModLoadFailure { .. }))
        {
            tracing::warn!(
                earlier = ?self.diagnosis,
                "a missing dependency outranks the earlier crash cause"
            );
            self.anchor(
                CrashDiagnosis::ModLoadFailure {
                    missing: Vec::new(),
                },
                line,
            );
        }

        for found in found {
            if self.missing.len() >= MAX_MISSING {
                return;
            }

            if self.missing.iter().any(|known| known.same_as(&found)) {
                continue;
            }

            tracing::warn!(?found, "the mod loader named a missing dependency");
            self.missing.push(found);
        }
    }

    fn take_diagnosis(&mut self) -> Option<CrashDiagnosis> {
        let mut diagnosis = self.diagnosis.take()?;

        if let CrashDiagnosis::ModLoadFailure { missing } = &mut diagnosis {
            *missing = std::mem::take(&mut self.missing);
        }

        Some(diagnosis)
    }

    fn resolve_native_frame(&mut self, line: &str) {
        if std::mem::take(&mut self.pending_frame)
            && let Some(frame) = native_frame(line)
            && let Some(CrashDiagnosis::NativeCrash { frame: slot }) = self.diagnosis.as_mut()
            && slot.is_none()
        {
            tracing::warn!(%frame, "the crash names the native frame that failed");
            *slot = Some(frame);
        }

        if self.crash_report && line.contains(PROBLEMATIC_FRAME_MARKER) {
            self.pending_frame = true;
        }
    }

    fn document(&self) -> String {
        self.lines.iter().cloned().collect::<Vec<String>>().join(
            "
",
        )
    }

    fn excerpt(&self) -> Vec<String> {
        self.excerpt.clone().unwrap_or_else(|| {
            self.lines
                .iter()
                .rev()
                .take(EXCERPT_AFTER)
                .rev()
                .cloned()
                .collect()
        })
    }

    fn capture_diagnosis(&mut self, line: &str) {
        if self.diagnosis.is_some() {
            return;
        }

        let Some(diagnosis) = diagnose(line) else {
            return;
        };

        tracing::warn!(?diagnosis, "recognised a crash cause in the game log");

        self.anchor(diagnosis, line);
    }

    fn anchor(&mut self, diagnosis: CrashDiagnosis, line: &str) {
        let mut excerpt: Vec<String> = self
            .lines
            .iter()
            .rev()
            .skip(1)
            .take(EXCERPT_BEFORE)
            .rev()
            .cloned()
            .collect();

        excerpt.push(clamp_line(line));

        self.diagnosis = Some(diagnosis);
        self.excerpt = Some(excerpt);
        self.pending_after = EXCERPT_AFTER;
    }
}

#[derive(Clone, Default)]
pub(crate) struct CrashWatch {
    inner: Arc<Mutex<Watched>>,
}

impl CrashWatch {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn observe(&self, line: &str) {
        let Ok(mut watched) = self.inner.lock() else {
            return;
        };

        watched.note_crash_report(line);
        watched.buffer(line);
        watched.extend_excerpt(line);
        watched.capture_suspects(line);
        watched.capture_missing(line);
        watched.resolve_native_frame(line);
        watched.capture_diagnosis(line);
    }

    fn with<T>(&self, read: impl FnOnce(&Watched) -> T) -> Option<T> {
        self.inner.lock().ok().map(|watched| read(&watched))
    }

    pub(crate) fn reported_crash(&self) -> bool {
        self.with(|watched| watched.crash_report).unwrap_or(false)
    }

    pub(crate) fn suspects(&self) -> Vec<String> {
        self.with(|watched| watched.suspects.clone())
            .unwrap_or_default()
    }

    pub(crate) fn document(&self) -> String {
        self.with(Watched::document).unwrap_or_default()
    }

    pub(crate) fn excerpt(&self) -> Vec<String> {
        self.with(Watched::excerpt).unwrap_or_default()
    }

    pub(crate) fn take(&self) -> Option<CrashDiagnosis> {
        self.inner
            .lock()
            .ok()
            .and_then(|mut watched| watched.take_diagnosis())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zip_exception_is_recognised() {
        let diagnosis = diagnose("java.util.zip.ZipException: error in opening zip file");
        assert_eq!(
            diagnosis,
            Some(CrashDiagnosis::CorruptArchive { file: None })
        );
    }

    #[test]
    fn a_nested_zip_exception_is_recognised() {
        let line = "Caused by: java.util.zip.ZipException: zip END header not found";
        assert!(diagnose(line).is_some());
    }

    #[test]
    fn the_jar_is_named_when_the_message_carries_one() {
        let line = "java.util.zip.ZipException: error in opening zip file: \
                    /Users/someone/metadata/libraries/net/fabricmc/fabric-loader-0.15.jar";

        assert_eq!(
            diagnose(line),
            Some(CrashDiagnosis::CorruptArchive {
                file: Some("fabric-loader-0.15.jar".to_string()),
            })
        );
    }

    #[test]
    fn a_windows_path_is_reduced_to_its_file_name() {
        let line =
            r"Error: Invalid or corrupt jarfile C:\Users\someone\metadata\libraries\asm-9.7.jar";

        assert_eq!(
            diagnose(line),
            Some(CrashDiagnosis::CorruptArchive {
                file: Some("asm-9.7.jar".to_string()),
            })
        );
    }

    #[test]
    fn damaged_class_bytes_inside_an_intact_jar_are_recognised() {
        let line = "java.lang.ClassFormatError: Invalid code attribute name index 0 \
                    in class file org/objectweb/asm/tree/LookupSwitchInsnNode";

        assert_eq!(
            diagnose(line),
            Some(CrashDiagnosis::CorruptArchive { file: None })
        );
    }

    #[test]
    fn the_other_built_in_causes_are_recognised() {
        assert_eq!(
            diagnose("java.lang.OutOfMemoryError: Java heap space"),
            Some(CrashDiagnosis::OutOfMemory)
        );
        assert_eq!(
            diagnose("java.lang.UnsupportedClassVersionError: net/minecraft/client/Main"),
            Some(CrashDiagnosis::UnsupportedJava)
        );
        assert_eq!(
            diagnose("org.lwjgl.LWJGLException: Pixel format not accelerated"),
            Some(CrashDiagnosis::GraphicsDriver)
        );
    }

    #[test]
    fn a_mod_built_for_another_version_is_recognised() {
        for line in [
            "cpw.mods.fml.common.WrongMinecraftVersionException: The mod Foo does not run on 1.8.9",
            "net.minecraftforge.fml.common.MissingModsException: Mod Foo requires [bar]",
            "Missing or unsupported mandatory dependencies:",
            "net.fabricmc.loader.impl.FormattedException: Incompatible mods found!",
            "\t- Mod 'Foo' (foo) 1.0 requires version 1.20.1 of minecraft, \
             but only the wrong version is present: minecraft 1.21!",
        ] {
            assert!(
                matches!(diagnose(line), Some(CrashDiagnosis::ModLoadFailure { .. })),
                "{line}"
            );
        }
    }

    #[test]
    fn a_load_failure_is_fatal_but_a_linkage_error_is_not() {
        let line = "java.lang.NoSuchMethodError: net.minecraft.client.Minecraft.func_71410_x()";
        assert_eq!(diagnose(line), Some(CrashDiagnosis::ModLinkage));

        assert!(
            CrashDiagnosis::ModLoadFailure {
                missing: Vec::new()
            }
            .is_fatal()
        );
        assert!(!CrashDiagnosis::ModLinkage.is_fatal());
        assert!(!CrashDiagnosis::OutOfMemory.is_fatal());
    }

    #[test]
    fn running_out_of_memory_leaves_the_game_broken() {
        let line = "Exception: java.lang.OutOfMemoryError thrown from the \
                    UncaughtExceptionHandler in thread \"MicrophoneThread\"";
        let diagnosis = diagnose(line).expect("an out of memory error");

        assert_eq!(diagnosis, CrashDiagnosis::OutOfMemory);
        assert!(diagnosis.leaves_game_broken());
        assert!(!CrashDiagnosis::ModLinkage.leaves_game_broken());
        assert!(
            !CrashDiagnosis::ModLoadFailure {
                missing: Vec::new()
            }
            .leaves_game_broken()
        );
    }

    #[test]
    fn a_mod_problem_sends_the_user_to_the_mods_list() {
        assert_eq!(
            CrashDiagnosis::ModLoadFailure {
                missing: Vec::new()
            }
            .remedy(),
            Some(CrashRemedy::OpenMods)
        );
        assert_eq!(
            CrashDiagnosis::ModLinkage.remedy(),
            Some(CrashRemedy::OpenMods)
        );
    }

    #[test]
    fn ordinary_game_output_is_not_a_crash() {
        for line in [
            "[Render thread/INFO]: Setting user: Dev",
            "[main/INFO]: Loading 42 mods",
            "Loaded jar file sodium-0.5.jar",
            "[Worker-Main-1/WARN]: Unable to play unknown soundEvent",
            "",
        ] {
            assert_eq!(diagnose(line), None, "{line}");
        }
    }

    #[test]
    fn the_watch_keeps_the_first_cause_not_the_last() {
        let watch = CrashWatch::new();

        watch.observe("[main/INFO]: Loading mods");
        watch.observe("java.util.zip.ZipException: error in opening zip file: first.jar");
        watch.observe("java.util.zip.ZipException: error in opening zip file: second.jar");

        assert_eq!(
            watch.take(),
            Some(CrashDiagnosis::CorruptArchive {
                file: Some("first.jar".to_string()),
            })
        );
    }

    #[test]
    fn a_crash_report_is_noticed_even_with_no_recognised_cause() {
        let watch = CrashWatch::new();
        watch.observe("[main/INFO]: Loading 42 mods");
        assert!(!watch.reported_crash());

        watch.observe("---- Minecraft Crash Report ----");
        watch.observe("// Everything's going to plan. No, really, that was supposed to happen.");

        assert!(watch.reported_crash());
        assert_eq!(watch.take(), None);
    }

    #[test]
    fn the_suspected_mods_line_is_split_into_names() {
        let line =
            "[12:04:11] [Render thread/ERROR]: \tSuspected Mods: Sodium (sodium), Iris (iris)";

        assert_eq!(
            suspected_mods(line),
            Some(vec![
                "Sodium (sodium)".to_string(),
                "Iris (iris)".to_string(),
            ])
        );
    }

    #[test]
    fn a_blameless_suspected_mods_line_names_nobody() {
        for line in [
            "Suspected Mods: Unknown",
            "Suspected Mods: None",
            "Suspected Mods:",
            "Suspected Mods: Minecraft (minecraft)",
        ] {
            assert_eq!(suspected_mods(line), None, "{line}");
        }
    }

    #[test]
    fn the_vanilla_entry_is_dropped_but_the_mods_beside_it_are_kept() {
        let line = "Suspected Mods: Minecraft (minecraft), Biomes O' Plenty (biomesoplenty)";

        assert_eq!(
            suspected_mods(line),
            Some(vec!["Biomes O' Plenty (biomesoplenty)".to_string()])
        );
    }

    #[test]
    fn an_ordinary_line_names_no_suspects() {
        assert_eq!(suspected_mods("[main/INFO]: Loading 42 mods"), None);
    }

    #[test]
    fn the_watch_keeps_the_suspects_named_after_the_cause() {
        let watch = CrashWatch::new();

        watch.observe("java.lang.NoSuchMethodError: net.minecraft.client.Minecraft.func_71410_x()");
        watch.observe("---- Minecraft Crash Report ----");
        watch.observe("\tSuspected Mods: Skytils (skytils)");

        assert_eq!(watch.suspects(), vec!["Skytils (skytils)".to_string()]);
        assert!(watch.reported_crash());
        assert_eq!(watch.take(), Some(CrashDiagnosis::ModLinkage));
    }

    #[test]
    fn a_session_with_no_crash_reporter_names_no_suspects() {
        let watch = CrashWatch::new();
        watch.observe("java.lang.OutOfMemoryError: Java heap space");

        assert!(watch.suspects().is_empty());
    }

    #[test]
    fn a_native_crash_is_recognised() {
        for line in [
            "#  EXCEPTION_ACCESS_VIOLATION (0xc0000005) at pc=0x00007ffb1e2d1e40, pid=8452",
            "#  SIGSEGV (0xb) at pc=0x00007f8a1c0d1e40, pid=8452, tid=8460",
        ] {
            assert_eq!(
                diagnose(line),
                Some(CrashDiagnosis::NativeCrash { frame: None }),
                "{line}"
            );
        }
    }

    #[test]
    fn the_problematic_frame_is_read_off_a_native_dump() {
        assert_eq!(
            native_frame("# C  [atio6axx.dll+0x9d1e40]"),
            Some("atio6axx.dll".to_string())
        );
        assert_eq!(
            native_frame("# V  [jvm.dll+0x5a1b2c]"),
            Some("jvm.dll".to_string())
        );
        assert_eq!(
            native_frame("# C  [libGLX_nvidia.so.0+0x2b1c40]"),
            Some("libGLX_nvidia.so.0".to_string())
        );
        assert_eq!(
            native_frame("# j  me.jellysquid.mods.sodium.client.render.Chunk.build()V+12"),
            Some("me.jellysquid.mods.sodium.client.render.Chunk.build()V+12".to_string())
        );
    }

    #[test]
    fn the_watch_names_the_library_that_died() {
        let watch = CrashWatch::new();

        watch.observe("# A fatal error has been detected by the Java Runtime Environment:");
        watch.observe("#");
        watch.observe("#  EXCEPTION_ACCESS_VIOLATION (0xc0000005) at pc=0x00007ffb1e2d1e40");
        watch.observe("#");
        watch.observe("# Problematic frame:");
        watch.observe("# C  [atio6axx.dll+0x9d1e40]");

        assert!(watch.reported_crash());
        assert_eq!(
            watch.take(),
            Some(CrashDiagnosis::NativeCrash {
                frame: Some("atio6axx.dll".to_string()),
            })
        );
    }

    #[test]
    fn a_native_crash_gets_a_title_that_names_the_library() {
        let crash = CrashDiagnosis::NativeCrash {
            frame: Some("atio6axx.dll".to_string()),
        };

        assert_eq!(crash.title(), "atio6axx.dll crashed the game");
        assert!(crash.body().contains("atio6axx.dll"));
        assert_eq!(crash.remedy(), Some(CrashRemedy::OpenMods));

        assert_eq!(
            CrashDiagnosis::NativeCrash { frame: None }.title(),
            "The game crashed outside Java"
        );
    }

    #[test]
    fn a_frame_named_after_an_earlier_cause_does_not_overwrite_it() {
        let watch = CrashWatch::new();

        watch.observe("java.lang.OutOfMemoryError: Java heap space");
        watch.observe("# Problematic frame:");
        watch.observe("# C  [atio6axx.dll+0x9d1e40]");

        assert_eq!(watch.take(), Some(CrashDiagnosis::OutOfMemory));
    }

    #[test]
    fn a_clean_session_diagnoses_nothing() {
        let watch = CrashWatch::new();
        watch.observe("[Render thread/INFO]: Stopping!");
        assert_eq!(watch.take(), None);
    }

    #[test]
    fn taking_a_diagnosis_consumes_it() {
        let watch = CrashWatch::new();
        watch.observe("java.util.zip.ZipException: error in opening zip file");

        assert!(watch.take().is_some());
        assert!(
            watch.take().is_none(),
            "a diagnosis must not be reported twice"
        );
    }

    #[test]
    fn the_document_carries_every_line_for_multi_cause_rules() {
        let watch = CrashWatch::new();
        watch.observe("java.lang.NullPointerException: Initializing game");
        watch.observe("\tat net.minecraft.client.Minecraft.run");
        watch.observe("\tat bingobrewers.Overlay.render");

        let document = watch.document();
        assert!(document.contains("NullPointerException"));
        assert!(document.contains("bingobrewers"));
    }

    #[test]
    fn the_document_stays_bounded() {
        let watch = CrashWatch::new();
        let line = "x".repeat(1024);
        for _ in 0..400 {
            watch.observe(&line);
        }

        assert!(
            watch.document().len() <= DOCUMENT_BUDGET + 1024,
            "{}",
            watch.document().len()
        );
    }

    #[test]
    fn the_excerpt_starts_at_the_cause_and_carries_the_stack() {
        let watch = CrashWatch::new();
        watch.observe("[main/INFO]: Loading mods");
        watch.observe("[main/INFO]: Almost there");
        watch.observe("java.util.zip.ZipException: error in opening zip file");
        watch.observe("\tat java.util.zip.ZipFile.open");

        let excerpt = watch.excerpt();
        assert!(excerpt.contains(&"[main/INFO]: Almost there".to_string()));
        assert!(
            excerpt
                .iter()
                .any(|line| line.contains("java.util.zip.ZipException"))
        );
        assert!(excerpt.iter().any(|line| line.contains("ZipFile.open")));
    }

    #[test]
    fn an_unrecognised_crash_still_yields_the_tail() {
        let watch = CrashWatch::new();
        for n in 0..40 {
            watch.observe(&format!("line {n}"));
        }

        let excerpt = watch.excerpt();
        assert!(!excerpt.is_empty());
        assert_eq!(excerpt.last().unwrap(), "line 39");
    }

    fn dependency(
        dependency: &str,
        required_by: Option<&str>,
        version: Option<&str>,
        present: Option<&str>,
    ) -> MissingDependency {
        MissingDependency {
            dependency: dependency.to_string(),
            required_by: required_by.map(str::to_string),
            version: version.map(str::to_string),
            present: present.map(str::to_string),
        }
    }

    #[test]
    fn fabric_names_a_missing_dependency() {
        let line = "\t - Mod 'Sodium Extra' (sodium-extra) 0.5.1 requires any version of \
                    fabric-api, which is missing!";

        assert_eq!(
            missing_dependencies(line),
            vec![dependency("fabric-api", Some("Sodium Extra"), None, None)]
        );
    }

    #[test]
    fn fabric_names_a_required_version_and_the_one_present() {
        let missing = "\t - Mod 'Iris' (iris) 1.6.11 requires version 0.5.4 or later of \
                       sodium, which is missing!";
        assert_eq!(
            missing_dependencies(missing),
            vec![dependency(
                "sodium",
                Some("Iris"),
                Some("0.5.4 or later"),
                None
            )]
        );

        let wrong = "\t - Mod 'Fabric API' (fabric-api) 0.92.0+1.20.1 requires version 1.20.1 \
                     of 'Minecraft' (minecraft), but only the wrong version is present: 1.20.4!";
        assert_eq!(
            missing_dependencies(wrong),
            vec![dependency(
                "Minecraft",
                Some("Fabric API"),
                Some("1.20.1"),
                Some("1.20.4")
            )]
        );
    }

    #[test]
    fn fabric_keeps_apostrophes_in_mod_names() {
        let line = "\t - Mod 'Xaero's Minimap' (xaerominimap) 24.0 requires any version of \
                    mod 'Xaero's Lib' (xaerolib), which is missing!";

        assert_eq!(
            missing_dependencies(line),
            vec![dependency(
                "Xaero's Lib",
                Some("Xaero's Minimap"),
                None,
                None
            )]
        );
    }

    #[test]
    fn legacy_fabric_names_a_missing_dependency() {
        let line = "net.fabricmc.loader.discovery.ModResolutionException: Could not find \
                    required mod: modmenu requires {fabric @ [>=0.29.0]}";

        assert_eq!(
            missing_dependencies(line),
            vec![dependency(
                "fabric",
                Some("modmenu"),
                Some(">=0.29.0"),
                None
            )]
        );
    }

    #[test]
    fn legacy_forge_lists_every_missing_mod() {
        let line = "[main/FATAL] [FML]: The mod skyblockaddons (SkyblockAddons) requires mods \
                    [essential, oneconfig@[1.0,)] to be available";

        assert_eq!(
            missing_dependencies(line),
            vec![
                dependency("essential", Some("SkyblockAddons"), None, None),
                dependency(
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
            missing_dependencies(line),
            vec![dependency(
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
            missing_dependencies(missing),
            vec![dependency(
                "architectury",
                Some("rei"),
                Some("9.1.12 or later"),
                None
            )]
        );

        let wrong = "\tMod ID: 'forge', Requested by: 'jei', Expected range: '[47.1,)', \
                     Actual version: '47.0.3'";
        assert_eq!(
            missing_dependencies(wrong),
            vec![dependency(
                "forge",
                Some("jei"),
                Some("47.1 or later"),
                Some("47.0.3")
            )]
        );
    }

    #[test]
    fn ordinary_lines_name_no_dependency() {
        for line in [
            "[main/INFO]: Mod 'Sodium' (sodium) requires a restart",
            "[main/INFO]: This mod requires [nothing] special",
            "Loading 42 mods",
        ] {
            assert!(missing_dependencies(line).is_empty(), "{line}");
        }
    }

    #[test]
    fn the_watch_gathers_dependencies_listed_after_the_header() {
        let watch = CrashWatch::new();
        watch.observe("[main/ERROR]: Incompatible mods found!");
        watch.observe(
            "net.fabricmc.loader.impl.FormattedException: Some of your mods are incompatible",
        );
        watch.observe("A potential solution has been determined:");
        watch.observe("\t - Install fabric-api, any version.");
        watch.observe("More details:");
        watch.observe(
            "\t - Mod 'Indium' (indium) 1.0.27 requires any version of fabric-api, which is missing!",
        );
        watch.observe(
            "\t - Mod 'Sodium Extra' (sodium-extra) 0.5.1 requires any version of fabric-api, \
             which is missing!",
        );
        watch.observe(
            "\t - Mod 'Indium' (indium) 1.0.27 requires any version of fabric-api, which is missing!",
        );

        let Some(CrashDiagnosis::ModLoadFailure { missing }) = watch.take() else {
            panic!("expected a mod load failure");
        };

        assert_eq!(
            missing,
            vec![
                dependency("fabric-api", Some("Indium"), None, None),
                dependency("fabric-api", Some("Sodium Extra"), None, None),
            ]
        );
    }

    #[test]
    fn a_single_missing_dependency_is_named_in_the_title() {
        let crash = CrashDiagnosis::ModLoadFailure {
            missing: vec![dependency("fabric-api", Some("Sodium Extra"), None, None)],
        };

        assert_eq!(crash.title(), "Sodium Extra needs fabric-api");
        assert!(crash.body().starts_with("Sodium Extra needs fabric-api."));
    }

    #[test]
    fn several_mods_needing_one_dependency_share_a_title() {
        let crash = CrashDiagnosis::ModLoadFailure {
            missing: vec![
                dependency("fabric-api", Some("Indium"), None, None),
                dependency("fabric-api", Some("Sodium Extra"), None, None),
            ],
        };

        assert_eq!(crash.title(), "2 mods need fabric-api");
    }

    #[test]
    fn a_wrong_version_is_described_with_what_is_installed() {
        let crash = CrashDiagnosis::ModLoadFailure {
            missing: vec![dependency(
                "Sodium",
                Some("Iris"),
                Some("0.5.4 or later"),
                Some("0.5.1"),
            )],
        };

        assert_eq!(crash.title(), "Iris needs a different version of Sodium");
        assert_eq!(
            crash.body(),
            format!("Iris needs Sodium 0.5.4 or later, but 0.5.1 is installed. {INSTALL_ADVICE}")
        );
    }

    #[test]
    fn a_mod_made_for_another_minecraft_is_told_apart_from_a_missing_mod() {
        let crash = CrashDiagnosis::ModLoadFailure {
            missing: vec![dependency(
                "Minecraft",
                Some("Fabric API"),
                Some("1.20.1"),
                Some("1.20.4"),
            )],
        };

        assert_eq!(
            crash.title(),
            "Fabric API is not made for this version of Minecraft"
        );
        assert_eq!(
            crash.body(),
            format!(
                "Fabric API is made for Minecraft 1.20.1, but this cluster has 1.20.4. \
                 {PLATFORM_ADVICE}"
            )
        );
    }

    #[test]
    fn platform_ids_are_shown_by_name() {
        let line = "\tMod ID: 'minecraft', Requested by: 'jei', Expected range: '[1.20.1]', \
                    Actual version: '1.20.4'";
        let crash = CrashDiagnosis::ModLoadFailure {
            missing: missing_dependencies(line),
        };

        assert_eq!(
            crash.title(),
            "jei is not made for this version of Minecraft"
        );

        for (id, name) in [
            ("java", "Java"),
            ("fabricloader", "Fabric Loader"),
            ("forge", "Forge"),
            ("neoforge", "NeoForge"),
        ] {
            assert_eq!(dependency(id, Some("Foo"), None, None).display_name(), name);
        }
    }

    #[test]
    fn mixed_causes_get_both_pieces_of_advice() {
        let crash = CrashDiagnosis::ModLoadFailure {
            missing: vec![
                dependency("fabric-api", Some("Indium"), None, None),
                dependency("minecraft", Some("Iris"), Some("1.20.1"), Some("1.20.4")),
            ],
        };

        let body = crash.body();
        assert!(body.contains(INSTALL_ADVICE));
        assert!(body.contains(PLATFORM_ADVICE));
        assert_eq!(crash.title(), "Some mods are missing what they depend on");
    }

    #[test]
    fn several_mods_made_for_another_version_share_a_title() {
        let crash = CrashDiagnosis::ModLoadFailure {
            missing: vec![
                dependency("minecraft", Some("Iris"), Some("1.20.1"), None),
                dependency("Minecraft", Some("Sodium"), Some("1.20.1"), None),
            ],
        };

        assert_eq!(
            crash.title(),
            "2 mods are not made for this version of Minecraft"
        );
        assert!(!crash.body().contains(INSTALL_ADVICE));
    }

    #[test]
    fn a_large_pack_is_counted_in_full() {
        let watch = CrashWatch::new();
        watch.observe("[main/ERROR]: Incompatible mods found!");
        for n in 0..30 {
            watch.observe(&format!(
                "\t - Mod 'Mod {n}' (mod{n}) 1.0 requires any version of fabric-api, \
                 which is missing!"
            ));
        }

        let crash = watch.take().expect("a mod load failure");
        assert_eq!(crash.title(), "30 mods need fabric-api");
        assert!(crash.body().contains("And 26 more."));
    }

    #[test]
    fn a_named_missing_dependency_outranks_an_earlier_cause() {
        let watch = CrashWatch::new();
        watch.observe("java.util.zip.ZipException: error in opening zip file: optional.jar");
        watch.observe("[main/ERROR]: Incompatible mods found!");
        watch.observe(
            "\t - Mod 'Iris' (iris) 1.6.11 requires any version of sodium, which is missing!",
        );

        let excerpt = watch.excerpt();
        let Some(CrashDiagnosis::ModLoadFailure { missing }) = watch.take() else {
            panic!("the missing dependency should win");
        };

        assert_eq!(
            missing,
            vec![dependency("sodium", Some("Iris"), None, None)]
        );
        assert!(
            excerpt
                .iter()
                .any(|line| line.contains("requires any version of sodium"))
        );
    }

    #[test]
    fn a_loader_error_without_details_keeps_the_earlier_cause() {
        let watch = CrashWatch::new();
        watch.observe("java.util.zip.ZipException: error in opening zip file: broken.jar");
        watch.observe("net.fabricmc.loader.impl.FormattedException: Failed to read mod file");

        assert_eq!(
            watch.take(),
            Some(CrashDiagnosis::CorruptArchive {
                file: Some("broken.jar".to_string()),
            })
        );
    }

    #[test]
    fn a_long_list_is_cut_short_in_the_body() {
        let crash = CrashDiagnosis::ModLoadFailure {
            missing: (0..6)
                .map(|n| dependency(&format!("dep{n}"), Some("Foo"), None, None))
                .collect(),
        };

        let body = crash.body();
        assert!(body.contains("dep3"));
        assert!(!body.contains("dep4"));
        assert!(body.contains("And 2 more."));
    }

    #[test]
    fn without_details_the_generic_message_stays() {
        let crash = CrashDiagnosis::ModLoadFailure {
            missing: Vec::new(),
        };

        assert_eq!(crash.title(), "A mod stopped the game from starting");
        assert!(crash.body().starts_with("The mod loader refused to start."));
    }

    const FABRIC_RECOMMENDS_WARNING: &str = " - 'Minecraft' (minecraft) 26.3 recommends any version \
         after 15.0.0-alpha.5 of Fabulously Optimized, which is missing!";

    #[test]
    fn a_fabric_recommendation_warning_is_not_a_crash() {
        assert_eq!(diagnose(FABRIC_RECOMMENDS_WARNING), None);
        assert!(missing_dependencies(FABRIC_RECOMMENDS_WARNING).is_empty());

        let wrong = " - 'Sodium Extra' (sodium-extra) 0.6.0 recommends version 1.0 of \
                     'Reese's Sodium Options' (reeses-sodium-options), but only the wrong \
                     version is present: 0.9!";
        assert_eq!(diagnose(wrong), None);
    }

    #[test]
    fn a_healthy_launch_with_warnings_leaves_no_cause_behind() {
        let watch = CrashWatch::new();
        watch.observe("[11:44:35] [main/WARN]: Warnings were found!");
        watch.observe(FABRIC_RECOMMENDS_WARNING);
        watch.observe("[11:44:40] [Render thread/INFO]: Stopping!");

        assert_eq!(watch.take(), None);
    }

    #[test]
    fn a_warning_does_not_steal_a_later_real_cause() {
        let watch = CrashWatch::new();
        watch.observe("[11:44:35] [main/WARN]: Warnings were found!");
        watch.observe(FABRIC_RECOMMENDS_WARNING);
        watch.observe("Caused by: java.lang.OutOfMemoryError: Java heap space");

        assert_eq!(watch.take(), Some(CrashDiagnosis::OutOfMemory));
    }

    #[test]
    fn current_fabric_lines_without_the_mod_prefix_are_parsed() {
        let missing = " - 'Iris' (iris) 1.8.0 requires any version after 0.6.0 of Sodium, \
                       which is missing!";
        assert_eq!(
            missing_dependencies(missing),
            vec![dependency(
                "Sodium",
                Some("Iris"),
                Some("newer than 0.6.0"),
                None
            )]
        );

        let wrong = "\t - 'Fabric API' (fabric-api) 0.120.0 requires version 26.3 of \
                     'Minecraft' (minecraft), but only the wrong version is present: 26.2!";
        let crash = CrashDiagnosis::ModLoadFailure {
            missing: missing_dependencies(wrong),
        };
        assert_eq!(
            crash.title(),
            "Fabric API is not made for this version of Minecraft"
        );
    }

    #[test]
    fn current_fabric_errors_reach_the_dialog_through_the_watch() {
        let watch = CrashWatch::new();
        watch.observe("[main/ERROR]: Incompatible mods found!");
        watch.observe(" - 'Iris' (iris) 1.8.0 requires any version of sodium, which is missing!");

        assert_eq!(
            watch.take().map(|crash| crash.title()),
            Some("Iris needs sodium".to_string())
        );
    }
}
