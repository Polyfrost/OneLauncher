use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use oneclient_events::CrashRemedy;

const DOCUMENT_BUDGET: usize = 96 * 1024;
const LINE_BUDGET: usize = 8 * 1024;

const EXCERPT_BEFORE: usize = 2;
const EXCERPT_AFTER: usize = 24;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrashDiagnosis {
    CorruptArchive { file: Option<String> },
    OutOfMemory,
    UnsupportedJava,
    ModLoadFailure,
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
            Self::ModLoadFailure => "A mod stopped the game from starting".to_string(),
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
                "The game could not read {file} — the file is damaged. \
                 Verifying will re-download anything that does not match."
            ),
            Self::CorruptArchive { file: None } => "The game could not read one of its \
                 library or mod files — it is damaged. Verifying will re-download \
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
            Self::ModLoadFailure => "The mod loader refused to start. Usually one of your mods \
                 was built for a different Minecraft version, or it needs another mod that is \
                 not installed."
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
                 driver is out of date — and a rendering mod that does not match the \
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
            Self::ModLoadFailure | Self::ModLinkage | Self::NativeCrash { .. } => {
                Some(CrashRemedy::OpenMods)
            }
            Self::GraphicsDriver => None,
        }
    }

    #[must_use]
    pub fn is_fatal(&self) -> bool {
        matches!(self, Self::ModLoadFailure)
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

const MOD_LOAD_MARKERS: [&str; 9] = [
    "WrongMinecraftVersionException",
    "MissingModsException",
    "Missing or unsupported mandatory dependencies",
    "ModLoadingException",
    "net.fabricmc.loader.impl.FormattedException",
    "Incompatible mod set!",
    "Mod resolution encountered an incompatible mod set",
    "but only the wrong version is present",
    "Forge Mod Loader has found a problem with your minecraft installation",
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

    if hit(&MOD_LOAD_MARKERS) {
        return Some(CrashDiagnosis::ModLoadFailure);
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
            .and_then(|mut watched| watched.diagnosis.take())
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
            assert_eq!(
                diagnose(line),
                Some(CrashDiagnosis::ModLoadFailure),
                "{line}"
            );
        }
    }

    #[test]
    fn a_load_failure_is_fatal_but_a_linkage_error_is_not() {
        let line = "java.lang.NoSuchMethodError: net.minecraft.client.Minecraft.func_71410_x()";
        assert_eq!(diagnose(line), Some(CrashDiagnosis::ModLinkage));

        assert!(CrashDiagnosis::ModLoadFailure.is_fatal());
        assert!(!CrashDiagnosis::ModLinkage.is_fatal());
        assert!(!CrashDiagnosis::OutOfMemory.is_fatal());
    }

    #[test]
    fn a_mod_problem_sends_the_user_to_the_mods_list() {
        assert_eq!(
            CrashDiagnosis::ModLoadFailure.remedy(),
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
}
