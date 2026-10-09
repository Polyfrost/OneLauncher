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
                "The game could not read {file} - the file is damaged. \
                 Verifying will re-download anything that does not match."
            ),
            Self::CorruptArchive { file: None } => "The game could not read one of its \
                 library or mod files - it is damaged. Verifying will re-download \
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

    Some(CrashDiagnosis::CorruptArchive { file: jar_in(line) })
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MissingTarget {
    Mod(String),
    Class(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingDependency {
    pub requester: Option<String>,
    pub missing: MissingTarget,
}

const MISSING_DEPENDENCY_MARKERS: [&str; 4] = [
    "which is missing",
    "Requested by",
    "MissingModsException",
    "NoClassDefFoundError",
];

const MAX_MISSING: usize = 32;

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
    })
}

fn legacy_forge_missing(line: &str) -> Option<Vec<MissingDependency>> {
    let after = &line[line.find("MissingModsException")?..];
    let after = &after[after.find("Mod ")? + 4..];
    let requester = after.split_whitespace().next()?;
    let list = &after[after.find("requires [")? + "requires [".len()..];
    let list = list.trim_end().strip_suffix(']').unwrap_or(list);
    let found: Vec<MissingDependency> = list
        .split(", ")
        .filter_map(|spec| mod_id(spec.split('@').next().unwrap_or(spec)))
        .map(|missing| MissingDependency {
            requester: mod_id(requester),
            missing: MissingTarget::Mod(missing),
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
    })
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

/// Keeps the first recognised cause not the last a corrupt jar cascades into
/// unrelated failures so the earliest line is closest to the root cause
#[derive(Clone, Default)]
pub(crate) struct CrashWatch {
    found: Arc<Mutex<Option<CrashDiagnosis>>>,
    missing: Arc<Mutex<Vec<MissingDependency>>>,
}

impl CrashWatch {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn observe(&self, line: &str) {
        for found in missing_dependencies(line) {
            if let Ok(mut missing) = self.missing.lock()
                && missing.len() < MAX_MISSING
                && !missing.contains(&found)
            {
                missing.push(found);
            }
        }

        // Cheap rejection first this runs on every line the game prints
        if self.found.lock().is_ok_and(|found| found.is_some()) {
            return;
        }

        if let Some(diagnosis) = diagnose(line)
            && let Ok(mut found) = self.found.lock()
            && found.is_none()
        {
            tracing::warn!(
                ?diagnosis,
                "recognised a repairable crash cause in the game log"
            );
            *found = Some(diagnosis);
        }
    }

    pub(crate) fn take(&self) -> Option<CrashDiagnosis> {
        self.found.lock().ok().and_then(|mut found| found.take())
    }

    pub(crate) fn take_missing(&self) -> Vec<MissingDependency> {
        self.missing
            .lock()
            .map(|mut missing| std::mem::take(&mut *missing))
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

    fn needs(requester: Option<&str>, missing: MissingTarget) -> Vec<MissingDependency> {
        vec![MissingDependency {
            requester: requester.map(str::to_string),
            missing,
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
                },
                MissingDependency {
                    requester: Some("sba".into()),
                    missing: MissingTarget::Mod("essential".into()),
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
        assert_eq!(watch.take_missing().len(), 1);
        assert!(watch.take_missing().is_empty());
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
}
