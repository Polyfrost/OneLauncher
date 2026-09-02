use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use oneclient_events::CrashRemedy;

const DOCUMENT_BUDGET: usize = 96 * 1024;

const EXCERPT_BEFORE: usize = 2;
const EXCERPT_AFTER: usize = 24;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrashDiagnosis {
    CorruptArchive { file: Option<String> },
    OutOfMemory,
    UnsupportedJava,
    GraphicsDriver,
}

impl CrashDiagnosis {
    #[must_use]
    pub fn title(&self) -> String {
        match self {
            Self::CorruptArchive { .. } => "A damaged file crashed the game".to_string(),
            Self::OutOfMemory => "Minecraft ran out of memory".to_string(),
            Self::UnsupportedJava => "This Java version cannot run the game".to_string(),
            Self::GraphicsDriver => "The game could not start graphics".to_string(),
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
            Self::GraphicsDriver => "The game could not create a graphics context. This is \
                 almost always an out-of-date or missing graphics driver rather than \
                 anything in the launcher."
                .to_string(),
        }
    }

    #[must_use]
    pub fn remedy(&self) -> Option<CrashRemedy> {
        match self {
            Self::CorruptArchive { .. } => Some(CrashRemedy::VerifyFiles),
            Self::OutOfMemory => Some(CrashRemedy::RaiseMemory),
            Self::UnsupportedJava => Some(CrashRemedy::OpenJavaSettings),
            Self::GraphicsDriver => None,
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

const GRAPHICS_MARKERS: [&str; 3] = [
    "org.lwjgl.LWJGLException: Pixel format not accelerated",
    "Failed to create window",
    "GLFW error 65542",
];

#[must_use]
pub fn diagnose(line: &str) -> Option<CrashDiagnosis> {
    if CORRUPT_ARCHIVE_MARKERS
        .iter()
        .any(|marker| line.contains(marker))
    {
        return Some(CrashDiagnosis::CorruptArchive {
            file: jar_in(line),
        });
    }

    if OUT_OF_MEMORY_MARKERS
        .iter()
        .any(|marker| line.contains(marker))
    {
        return Some(CrashDiagnosis::OutOfMemory);
    }

    if UNSUPPORTED_JAVA_MARKERS
        .iter()
        .any(|marker| line.contains(marker))
    {
        return Some(CrashDiagnosis::UnsupportedJava);
    }

    if GRAPHICS_MARKERS.iter().any(|marker| line.contains(marker)) {
        return Some(CrashDiagnosis::GraphicsDriver);
    }

    None
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

#[derive(Default)]
struct Watched {
    diagnosis: Option<CrashDiagnosis>,
    excerpt: Option<Vec<String>>,
    pending_after: usize,
    lines: VecDeque<String>,
    bytes: usize,
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

        watched.bytes += line.len() + 1;
        watched.lines.push_back(line.to_string());
        while watched.bytes > DOCUMENT_BUDGET {
            match watched.lines.pop_front() {
                Some(dropped) => watched.bytes -= dropped.len() + 1,
                None => break,
            }
        }

        if watched.pending_after > 0 {
            watched.pending_after -= 1;
            if let Some(excerpt) = watched.excerpt.as_mut() {
                excerpt.push(line.to_string());
            }
        }

        if watched.diagnosis.is_some() {
            return;
        }

        if let Some(diagnosis) = diagnose(line) {
            tracing::warn!(?diagnosis, "recognised a crash cause in the game log");

            let before: Vec<String> = watched
                .lines
                .iter()
                .rev()
                .skip(1)
                .take(EXCERPT_BEFORE)
                .rev()
                .cloned()
                .collect();

            let mut excerpt = before;
            excerpt.push(line.to_string());

            watched.diagnosis = Some(diagnosis);
            watched.excerpt = Some(excerpt);
            watched.pending_after = EXCERPT_AFTER;
        }
    }

    pub(crate) fn take(&self) -> Option<CrashDiagnosis> {
        self.inner
            .lock()
            .ok()
            .and_then(|mut watched| watched.diagnosis.take())
    }

    pub(crate) fn document(&self) -> String {
        self.inner.lock().map_or_else(
            |_| String::new(),
            |watched| {
                watched
                    .lines
                    .iter()
                    .cloned()
                    .collect::<Vec<String>>()
                    .join("\n")
            },
        )
    }

    pub(crate) fn excerpt(&self) -> Vec<String> {
        let Ok(watched) = self.inner.lock() else {
            return Vec::new();
        };

        watched.excerpt.clone().unwrap_or_else(|| {
            watched
                .lines
                .iter()
                .rev()
                .take(EXCERPT_AFTER)
                .rev()
                .cloned()
                .collect()
        })
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
        let line = r"Error: Invalid or corrupt jarfile C:\Users\someone\metadata\libraries\asm-9.7.jar";

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
        assert!(watch.take().is_none(), "a diagnosis must not be reported twice");
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
