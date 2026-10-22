use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use oneclient_events::{CrashModIssue, CrashRemedy};

use super::mod_problems::{
    MAX_PROBLEMS, ModProblem, is_mod_load_line, problem_issues, problems_body, problems_in,
    problems_title, without_rejected_builds,
};

const DOCUMENT_BUDGET: usize = 96 * 1024;
const LINE_BUDGET: usize = 8 * 1024;

const EXCERPT_BEFORE: usize = 2;
const EXCERPT_AFTER: usize = 24;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrashDiagnosis {
    CorruptArchive { file: Option<String> },
    OutOfMemory,
    UnsupportedJava,
    ModLoadFailure { problems: Vec<ModProblem> },
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
            Self::ModLoadFailure { problems } => problems_title(problems),
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
            Self::ModLoadFailure { problems } if !problems.is_empty() => problems_body(problems),
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

    #[must_use]
    pub fn mod_issues(&self) -> Vec<CrashModIssue> {
        match self {
            Self::ModLoadFailure { problems } => problem_issues(problems),
            _ => Vec::new(),
        }
    }
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
            problems: problems_in(line),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MissingTarget {
    Mod(String),
    Class(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingDependency {
    pub requester: Option<String>,
    pub missing: MissingTarget,
    pub frames: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LogFindings {
    pub sure: Vec<MissingDependency>,
    pub classes: Vec<MissingDependency>,
}

const MISSING_DEPENDENCY_MARKERS: [&str; 5] = [
    "which is missing",
    "Requested by",
    "MissingModsException",
    "requires mods [",
    "NoClassDefFoundError",
];

const MAX_MISSING: usize = 32;
const MAX_FRAMES: usize = 24;
const CRASH_WINDOW: usize = 400;

const IGNORED_FRAME_PREFIXES: [&str; 24] = [
    "java.",
    "javax.",
    "jdk.",
    "sun.",
    "com.sun.",
    "net.minecraft.",
    "net.minecraftforge.",
    "net.neoforged.",
    "net.fabricmc.",
    "org.quiltmc.",
    "org.spongepowered.",
    "cpw.mods.",
    "com.mojang.",
    "org.objectweb.",
    "kotlin.",
    "scala.",
    "org.lwjgl.",
    "io.netty.",
    "com.google.",
    "org.apache.",
    "it.unimi.",
    "oshi.",
    "org.slf4j.",
    "paulscode.",
];

#[must_use]
pub fn missing_dependencies(line: &str) -> Vec<MissingDependency> {
    if !MISSING_DEPENDENCY_MARKERS
        .iter()
        .any(|marker| line.contains(marker))
    {
        return Vec::new();
    }

    fabric_missing(line)
        .or_else(|| forge_missing(line))
        .map(|found| vec![found])
        .or_else(|| legacy_forge_missing(line))
        .or_else(|| missing_class(line).map(|found| vec![found]))
        .unwrap_or_default()
}

fn fabric_missing(line: &str) -> Option<MissingDependency> {
    let end = line.find(", which is missing")?;
    let head = &line[..end];
    let after_mod = &head[head.find("Mod '")?..];
    // Fabric words optional dependencies as recommends or suggests and only
    // warns about them
    if !after_mod.contains(" requires ") {
        return None;
    }
    let requester = parenthesised(&after_mod[after_mod.find("' (")?..])?;
    let target = head.rsplit(" of ").next()?.trim();
    let missing = if target.ends_with(')') {
        let open = target.rfind('(')?;
        &target[open + 1..target.len() - 1]
    } else {
        target.rsplit(' ').next()?
    };
    Some(MissingDependency {
        requester: Some(mod_id(requester)?),
        missing: MissingTarget::Mod(mod_id(missing)?),
        frames: Vec::new(),
    })
}

fn forge_missing(line: &str) -> Option<MissingDependency> {
    if !line.contains("Requested by") {
        return None;
    }
    if line.contains("Actual version") && !line.contains("MISSING") {
        return None;
    }
    let missing = quoted_after(line, "Mod ID:")?;
    let requester = quoted_after(line, "Requested by:")?;
    Some(MissingDependency {
        requester: Some(mod_id(requester)?),
        missing: MissingTarget::Mod(mod_id(missing)?),
        frames: Vec::new(),
    })
}

fn legacy_forge_missing(line: &str) -> Option<Vec<MissingDependency>> {
    let (after, list_start) = if let Some(at) = line.find("MissingModsException") {
        let after = &line[at..];
        (&after[after.find("Mod ")? + "Mod ".len()..], "requires [")
    } else {
        let after = &line[line.find("The mod ")? + "The mod ".len()..];
        (after, "requires mods [")
    };
    let requester = after.split_whitespace().next()?;
    let list = &after[after.find(list_start)? + list_start.len()..];
    let list = list.trim_end();
    let list = list
        .strip_suffix("] to be available")
        .or_else(|| list.strip_suffix(']'))
        .unwrap_or(list);
    let found: Vec<MissingDependency> = list
        .split(", ")
        .filter_map(|spec| mod_id(spec.split('@').next().unwrap_or(spec)))
        .map(|missing| MissingDependency {
            requester: mod_id(requester),
            missing: MissingTarget::Mod(missing),
            frames: Vec::new(),
        })
        .collect();
    (!found.is_empty()).then_some(found)
}

fn missing_class(line: &str) -> Option<MissingDependency> {
    let marker = "NoClassDefFoundError:";
    let after = &line[line.find(marker)? + marker.len()..];
    let class = after.split_whitespace().next()?.replace('.', "/");
    (!class.is_empty() && !class.starts_with("java/")).then_some(MissingDependency {
        requester: None,
        missing: MissingTarget::Class(class),
        frames: Vec::new(),
    })
}

fn frame_class(line: &str) -> Option<String> {
    let frame = line.trim_start().strip_prefix("at ")?;
    let call = frame.split('(').next()?.trim();
    let call = call.rsplit('/').next()?;
    let (class, _) = call.rsplit_once('.')?;
    let ours = class.contains('.')
        && !IGNORED_FRAME_PREFIXES
            .iter()
            .any(|prefix| class.starts_with(prefix));
    ours.then(|| class.replace('.', "/"))
}

fn continues_trace(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with("at ")
        || line.starts_with("...")
        || line.starts_with("Caused by:")
        || line.starts_with("Suppressed:")
}

#[derive(Default)]
struct MissingScan {
    lines: usize,
    sure: Vec<MissingDependency>,
    classes: Vec<(usize, MissingDependency)>,
    tracing: Option<usize>,
}

impl MissingScan {
    fn observe(&mut self, line: &str) {
        self.lines += 1;

        if let Some(index) = self.tracing {
            if let Some(class) = frame_class(line) {
                let frames = &mut self.classes[index].1.frames;
                if frames.len() < MAX_FRAMES && !frames.contains(&class) {
                    frames.push(class);
                }
                return;
            }
            if !continues_trace(line) {
                self.tracing = None;
            }
        }

        for found in missing_dependencies(line) {
            match &found.missing {
                MissingTarget::Mod(_) => {
                    if self.sure.len() < MAX_MISSING && !self.sure.contains(&found) {
                        self.sure.push(found);
                    }
                }
                MissingTarget::Class(_) => {
                    if let Some(index) = self
                        .classes
                        .iter()
                        .position(|(_, seen)| seen.missing == found.missing)
                    {
                        self.classes[index].0 = self.lines;
                        self.tracing = Some(index);
                    } else if self.classes.len() < MAX_MISSING {
                        self.classes.push((self.lines, found));
                        self.tracing = Some(self.classes.len() - 1);
                    }
                }
            }
        }
    }

    fn finish(&mut self, window: Option<usize>) -> LogFindings {
        let floor = window.map_or(0, |window| self.lines.saturating_sub(window));
        self.tracing = None;
        LogFindings {
            sure: std::mem::take(&mut self.sure),
            classes: std::mem::take(&mut self.classes)
                .into_iter()
                .filter(|(line, _)| *line > floor)
                .map(|(_, found)| found)
                .collect(),
        }
    }
}

pub(crate) async fn crash_report_findings(game_dir: &Path, since: SystemTime) -> LogFindings {
    let Ok(mut entries) = tokio::fs::read_dir(game_dir.join("crash-reports")).await else {
        return LogFindings::default();
    };

    let mut newest: Option<(SystemTime, std::path::PathBuf)> = None;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "txt") {
            continue;
        }
        let Ok(modified) = entry.metadata().await.and_then(|meta| meta.modified()) else {
            continue;
        };
        if modified >= since && newest.as_ref().is_none_or(|(time, _)| modified > *time) {
            newest = Some((modified, path));
        }
    }

    let Some((_, path)) = newest else {
        return LogFindings::default();
    };
    let Ok(bytes) = tokio::fs::read(&path).await else {
        return LogFindings::default();
    };
    let text = String::from_utf8_lossy(&bytes);

    let mut scan = MissingScan::default();
    for line in text.lines() {
        scan.observe(line);
    }
    scan.finish(None)
}

fn parenthesised(text: &str) -> Option<&str> {
    let open = text.find('(')?;
    let close = open + text[open..].find(')')?;
    Some(&text[open + 1..close])
}

fn quoted_after<'a>(line: &'a str, label: &str) -> Option<&'a str> {
    let after = &line[line.find(label)? + label.len()..];
    let rest = &after[after.find('\'')? + 1..];
    Some(&rest[..rest.find('\'')?])
}

fn mod_id(raw: &str) -> Option<String> {
    let id = raw
        .trim()
        .trim_start_matches("mod ")
        .trim_matches(['\'', '"'])
        .trim()
        .to_ascii_lowercase();
    let valid = !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    valid.then_some(id)
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
    report_cause: bool,
    pending_frame: bool,
    suspects: Vec<String>,
    problems: Vec<ModProblem>,
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

    fn capture_problems(&mut self, line: &str) {
        let found = problems_in(line);
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
                "a named mod problem outranks the earlier crash cause"
            );
            self.anchor(
                CrashDiagnosis::ModLoadFailure {
                    problems: Vec::new(),
                },
                line,
            );
        }

        for found in found {
            if self.problems.len() >= MAX_PROBLEMS {
                return;
            }

            if self.problems.iter().any(|known| known.same_as(&found)) {
                continue;
            }

            tracing::warn!(?found, "the mod loader named a mod problem");
            self.problems.push(found);
        }
    }

    fn take_diagnosis(&mut self) -> Option<CrashDiagnosis> {
        let mut diagnosis = self.diagnosis.take()?;

        if let CrashDiagnosis::ModLoadFailure { problems } = &mut diagnosis {
            *problems = without_rejected_builds(std::mem::take(&mut self.problems));
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
        if matches!(
            self.diagnosis,
            Some(CrashDiagnosis::ModLoadFailure { .. } | CrashDiagnosis::OutOfMemory)
        ) {
            return;
        }

        let Some(diagnosis) = diagnose(line) else {
            return;
        };

        let replaces = self.diagnosis.is_none()
            || diagnosis == CrashDiagnosis::OutOfMemory
            || (self.crash_report && !self.report_cause);
        if !replaces {
            return;
        }
        self.report_cause |= self.crash_report;

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
    missing: Arc<Mutex<MissingScan>>,
}

impl CrashWatch {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn observe(&self, line: &str) {
        if let Ok(mut scan) = self.missing.lock() {
            scan.observe(line);
        }

        let Ok(mut watched) = self.inner.lock() else {
            return;
        };

        watched.note_crash_report(line);
        watched.buffer(line);
        watched.extend_excerpt(line);
        watched.capture_suspects(line);
        watched.capture_problems(line);
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

    pub(crate) fn take_missing(&self) -> LogFindings {
        self.missing
            .lock()
            .map(|mut scan| scan.finish(Some(CRASH_WINDOW)))
            .unwrap_or_default()
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
                problems: Vec::new()
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
                problems: Vec::new()
            }
            .leaves_game_broken()
        );
    }

    #[test]
    fn a_mod_problem_sends_the_user_to_the_mods_list() {
        assert_eq!(
            CrashDiagnosis::ModLoadFailure {
                problems: Vec::new()
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

    fn needs(requester: Option<&str>, missing: MissingTarget) -> Vec<MissingDependency> {
        vec![MissingDependency {
            requester: requester.map(str::to_string),
            missing,
            frames: Vec::new(),
        }]
    }

    #[test]
    fn fabric_names_the_mod_and_the_missing_dependency() {
        assert_eq!(
            missing_dependencies(
                "\t - Mod 'Sodium Extra' (sodium-extra) 0.5.1 requires any version of fabric-api, which is missing!"
            ),
            needs(
                Some("sodium-extra"),
                MissingTarget::Mod("fabric-api".into())
            )
        );
        assert_eq!(
            missing_dependencies(
                "\t - Mod 'Mod Menu' (modmenu) 9.0.0 requires version 0.90.0 or later of 'Fabric API' (fabric-api), which is missing!"
            ),
            needs(Some("modmenu"), MissingTarget::Mod("fabric-api".into()))
        );
        assert_eq!(
            missing_dependencies(
                "\t - Mod 'Old' (old) 1.0 requires any version of mod cloth-config2, which is missing!"
            ),
            needs(Some("old"), MissingTarget::Mod("cloth-config2".into()))
        );
    }

    #[test]
    fn fabric_optional_dependencies_are_not_missing() {
        assert!(
            missing_dependencies(
                "\t - Mod 'Mod Menu' (modmenu) 9.0.0 recommends any version of fabric-api, which is missing!"
            )
            .is_empty()
        );
        assert!(
            missing_dependencies(
                "\t - Mod 'Mod Menu' (modmenu) 9.0.0 suggests version 1.0 or later of 'Cloth Config' (cloth-config), which is missing!"
            )
            .is_empty()
        );
    }

    #[test]
    fn forge_reports_only_the_missing_ones() {
        assert_eq!(
            missing_dependencies(
                "\tMod ID: 'cloth_config', Requested by: 'examplemod', Expected range: '[11,)', Actual version: '[MISSING]'"
            ),
            needs(
                Some("examplemod"),
                MissingTarget::Mod("cloth_config".into())
            )
        );
        assert!(
            missing_dependencies(
                "\tMod ID: 'cloth_config', Requested by: 'examplemod', Expected range: '[11,)', Actual version: '10.1'"
            )
            .is_empty()
        );
    }

    #[test]
    fn a_mod_name_with_brackets_still_finds_the_id() {
        assert_eq!(
            missing_dependencies(
                "\t - Mod 'Zoomify (Fabric)' (zoomify) 2.0 requires any version of yet_another_config_lib_v3, which is missing!"
            ),
            needs(
                Some("zoomify"),
                MissingTarget::Mod("yet_another_config_lib_v3".into())
            )
        );
    }

    #[test]
    fn a_version_range_is_not_read_as_a_mod() {
        assert_eq!(
            missing_dependencies(
                "net.minecraftforge.fml.common.MissingModsException: Mod a (A) requires [b@[1.0, 2.0)]"
            ),
            needs(Some("a"), MissingTarget::Mod("b".into()))
        );
    }

    #[test]
    fn legacy_forge_lists_every_missing_mod() {
        assert_eq!(
            missing_dependencies(
                "net.minecraftforge.fml.common.MissingModsException: Mod sba (SkyblockAddons) requires [oneconfig@[1.0,), essential]"
            ),
            vec![
                MissingDependency {
                    requester: Some("sba".into()),
                    missing: MissingTarget::Mod("oneconfig".into()),
                    frames: Vec::new(),
                },
                MissingDependency {
                    requester: Some("sba".into()),
                    missing: MissingTarget::Mod("essential".into()),
                    frames: Vec::new(),
                },
            ]
        );
    }

    #[test]
    fn a_missing_class_has_no_known_requester() {
        assert_eq!(
            missing_dependencies("java.lang.NoClassDefFoundError: gg/essential/api/EssentialAPI"),
            needs(
                None,
                MissingTarget::Class("gg/essential/api/EssentialAPI".into())
            )
        );
        assert!(
            missing_dependencies("java.lang.NoClassDefFoundError: java/awt/Toolkit").is_empty()
        );
    }

    #[test]
    fn the_watch_collects_missing_dependencies_alongside_a_diagnosis() {
        let watch = CrashWatch::new();
        watch.observe("java.util.zip.ZipException: error in opening zip file");
        watch.observe("java.lang.NoClassDefFoundError: a/B");
        watch.observe("java.lang.NoClassDefFoundError: a/B");

        assert!(watch.take().is_some());
        assert_eq!(watch.take_missing().classes.len(), 1);
        assert_eq!(watch.take_missing(), LogFindings::default());
    }

    #[test]
    fn forge_189_logs_the_missing_mods_in_its_own_words() {
        assert_eq!(
            missing_dependencies(
                "[Client thread/ERROR] [FML]: The mod sba (SkyblockAddons) requires mods [oneconfig] to be available"
            ),
            needs(Some("sba"), MissingTarget::Mod("oneconfig".into()))
        );
    }

    #[test]
    fn loader_messages_are_sure_and_class_errors_are_not() {
        let watch = CrashWatch::new();
        watch.observe(
            "\t - Mod 'Mod Menu' (modmenu) 9.0.0 requires any version of fabric-api, which is missing!",
        );
        watch.observe("java.lang.NoClassDefFoundError: a/B");

        let findings = watch.take_missing();
        assert_eq!(findings.sure.len(), 1);
        assert_eq!(findings.classes.len(), 1);
    }

    #[test]
    fn the_stack_under_a_missing_class_names_the_mods_in_it() {
        let watch = CrashWatch::new();
        for line in [
            "java.lang.NoClassDefFoundError: gg/essential/api/EssentialAPI",
            "\tat java.base/java.lang.Class.forName0(Native Method)",
            "\tat TRANSFORMER/minecraft@1.20.1/net.minecraft.client.Minecraft.<init>(Minecraft.java:1)",
            "\tat TRANSFORMER/coolmod@1.0/com.example.cool.CoolMod.init(CoolMod.java:42)",
            "\tat com.example.cool.CoolMod$Inner.run(CoolMod.java:50)",
            "Caused by: java.lang.ClassNotFoundException: gg.essential.api.EssentialAPI",
            "\t... 12 more",
            "[Render thread/INFO]: Stopping!",
            "\tat com.example.other.Unrelated.run(Unrelated.java:1)",
        ] {
            watch.observe(line);
        }

        let classes = watch.take_missing().classes;
        assert_eq!(classes.len(), 1);
        assert_eq!(
            classes[0].frames,
            ["com/example/cool/CoolMod", "com/example/cool/CoolMod$Inner"]
        );
    }

    #[test]
    fn a_class_error_long_before_the_end_is_not_trusted() {
        let watch = CrashWatch::new();
        watch.observe("java.lang.NoClassDefFoundError: dev/emi/emi/api/EmiPlugin");
        for _ in 0..CRASH_WINDOW {
            watch.observe("[Render thread/INFO]: still playing");
        }
        watch.observe("java.lang.NullPointerException: the real problem");

        assert!(watch.take_missing().classes.is_empty());
    }

    #[test]
    fn ordinary_lines_name_no_missing_dependency() {
        for line in [
            "[main/INFO]: Loading 42 mods",
            "[Render thread/WARN]: Missing sound for event: minecraft:item.goat_horn.play",
            "",
        ] {
            assert!(missing_dependencies(line).is_empty(), "{line}");
        }
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

    const FABRIC_RECOMMENDS_WARNING: &str = " - 'Minecraft' (minecraft) 26.3 recommends any version \
         after 15.0.0-alpha.5 of Fabulously Optimized, which is missing!";

    const IRIS_NEEDS_SODIUM: &str =
        " - 'Iris' (iris) 1.8.0 requires any version of sodium, which is missing!";

    fn problems_of(diagnosis: Option<CrashDiagnosis>) -> Vec<ModProblem> {
        match diagnosis {
            Some(CrashDiagnosis::ModLoadFailure { problems }) => problems,
            other => panic!("expected a mod load failure, got {other:?}"),
        }
    }

    #[test]
    fn the_watch_gathers_problems_listed_after_the_header() {
        let watch = CrashWatch::new();
        watch.observe("[main/ERROR]: Incompatible mods found!");
        watch.observe("A potential solution has been determined:");
        watch.observe("\t - Install fabric-api, any version.");
        watch.observe(
            " - 'Indium' (indium) 1.0.27 requires any version of fabric-api, which is missing!",
        );
        watch.observe(
            " - 'Sodium Extra' (sodium-extra) 0.5.1 requires any version of fabric-api, \
             which is missing!",
        );
        watch.observe(
            " - 'Indium' (indium) 1.0.27 requires any version of fabric-api, which is missing!",
        );

        let names: Vec<String> = problems_of(watch.take())
            .into_iter()
            .filter_map(|problem| problem.required_by.map(|by| by.name))
            .collect();
        assert_eq!(names, vec!["Indium", "Sodium Extra"]);
    }

    #[test]
    fn the_cause_in_the_crash_report_outranks_an_earlier_warning() {
        let watch = CrashWatch::new();
        watch.observe("[WARN]: Caught java.lang.NoSuchMethodError in an optional integration");
        watch.observe("---- Minecraft Crash Report ----");
        watch.observe("java.lang.UnsupportedClassVersionError: bad class");
        watch.observe("java.lang.NoSuchFieldError: later noise");

        assert_eq!(watch.take(), Some(CrashDiagnosis::UnsupportedJava));
    }

    #[test]
    fn running_out_of_memory_outranks_an_earlier_warning() {
        let watch = CrashWatch::new();
        watch.observe("[WARN]: java.util.zip.ZipException: broken resource pack");
        watch.observe("java.lang.OutOfMemoryError: Java heap space");

        assert_eq!(watch.take(), Some(CrashDiagnosis::OutOfMemory));
    }

    #[test]
    fn a_large_pack_is_counted_in_full() {
        let watch = CrashWatch::new();
        watch.observe("[main/ERROR]: Incompatible mods found!");
        for n in 0..30 {
            watch.observe(&format!(
                " - 'Mod {n}' (mod{n}) 1.0 requires any version of fabric-api, which is missing!"
            ));
        }

        let crash = watch.take().expect("a mod load failure");
        assert_eq!(crash.title(), "30 mods need Fabric API");
        assert!(crash.body().contains("And 26 more."));
    }

    #[test]
    fn current_fabric_errors_reach_the_dialog_through_the_watch() {
        let watch = CrashWatch::new();
        watch.observe("[main/ERROR]: Incompatible mods found!");
        watch.observe(IRIS_NEEDS_SODIUM);

        let crash = watch.take().expect("a mod load failure");
        assert_eq!(crash.title(), "Iris needs sodium");

        let issues = crash.mod_issues();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].target.name, "sodium");
        assert_eq!(issues[0].sources[0].name, "Iris");
        assert_eq!(issues[0].sources[0].version.as_deref(), Some("1.8.0"));
    }

    #[test]
    fn an_incompatible_pair_reaches_the_dialog_through_the_watch() {
        let watch = CrashWatch::new();
        watch.observe("[main/ERROR]: Incompatible mods found!");
        watch.observe(
            " - 'Sodium' (sodium) 0.6.0 is incompatible with any version of 'OptiFabric' \
             (optifabric), but a matching version is present: 1.14.3!",
        );

        let crash = watch.take().expect("a mod load failure");
        assert_eq!(crash.title(), "Sodium is incompatible with OptiFabric");
        assert_eq!(crash.remedy(), Some(CrashRemedy::OpenMods));
    }

    #[test]
    fn a_named_problem_outranks_an_earlier_cause() {
        let watch = CrashWatch::new();
        watch.observe("java.util.zip.ZipException: error in opening zip file: optional.jar");
        watch.observe("[main/ERROR]: Incompatible mods found!");
        watch.observe(IRIS_NEEDS_SODIUM);

        let excerpt = watch.excerpt();
        let problems = problems_of(watch.take());

        assert_eq!(problems.len(), 1);
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
    fn a_fabric_recommendation_warning_is_not_a_crash() {
        assert_eq!(diagnose(FABRIC_RECOMMENDS_WARNING), None);
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
    fn without_details_the_generic_message_stays() {
        let crash = CrashDiagnosis::ModLoadFailure {
            problems: Vec::new(),
        };

        assert_eq!(crash.title(), "A mod stopped the game from starting");
        assert!(crash.body().starts_with("The mod loader refused to start."));
        assert!(crash.mod_issues().is_empty());
    }

    #[test]
    fn only_mod_load_failures_carry_mod_rows() {
        assert!(CrashDiagnosis::OutOfMemory.mod_issues().is_empty());
    }
}
