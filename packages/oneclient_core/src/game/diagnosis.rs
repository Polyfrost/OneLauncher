//! Minecraft's exit code says only that it died the log says why and some
//! reasons are ones the launcher can fix

use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrashDiagnosis {
    /// A jar the JVM could not read on the classpath this is nearly always a
    /// damaged library or mod which verification repairs
    CorruptArchive {
        /// Many of these messages carry no path so this is a bonus rather than
        /// something to depend on
        file: Option<String>,
    },
}

impl CrashDiagnosis {
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
        }
    }
}

/// Different sources printing the same underlying damage they share a remedy
/// so they share a diagnosis
const CORRUPT_ARCHIVE_MARKERS: [&str; 4] = [
    "java.util.zip.ZipException",
    "java.util.zip.ZipError",
    "Invalid or corrupt jarfile",
    "zip END header not found",
];

/// Runs against every line the game prints so it stays substring scans rather
/// than anything that parses the line
#[must_use]
pub fn diagnose(line: &str) -> Option<CrashDiagnosis> {
    if !CORRUPT_ARCHIVE_MARKERS
        .iter()
        .any(|marker| line.contains(marker))
    {
        return None;
    }

    Some(CrashDiagnosis::CorruptArchive {
        file: jar_in(line),
    })
}

/// Opportunistic some JVMs omit the path entirely and a missing name still
/// leads to the same repair
fn jar_in(line: &str) -> Option<String> {
    let token = line
        .split(|c: char| c.is_whitespace() || c == '(' || c == ')')
        .find(|token| token.trim_end_matches(['.', ',', ':']).ends_with(".jar"))?;

    let token = token.trim_end_matches(['.', ',', ':']);

    // The full path is noise in a notification the file name is what the user
    // recognises
    let name = token
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(token)
        .to_string();

    (!name.is_empty()).then_some(name)
}

/// Keeps the first recognised cause not the last a corrupt jar cascades into
/// unrelated failures so the earliest line is closest to the root cause
#[derive(Clone, Default)]
pub(crate) struct CrashWatch {
    found: Arc<Mutex<Option<CrashDiagnosis>>>,
}

impl CrashWatch {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn observe(&self, line: &str) {
        // Cheap rejection first this runs on every line the game prints
        if self.found.lock().is_ok_and(|found| found.is_some()) {
            return;
        }

        if let Some(diagnosis) = diagnose(line)
            && let Ok(mut found) = self.found.lock()
            && found.is_none()
        {
            tracing::warn!(?diagnosis, "recognised a repairable crash cause in the game log");
            *found = Some(diagnosis);
        }
    }

    pub(crate) fn take(&self) -> Option<CrashDiagnosis> {
        self.found.lock().ok().and_then(|mut found| found.take())
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
    fn ordinary_game_output_is_not_a_crash() {
        // A false positive would nag the user into re-downloading for nothing
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
}
