use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use async_tempfile::TempDir;
use tokio::sync::OnceCell;

use crate::error::{JavaError, JavaResult};

const JAVA_INFO_CLASS: &[u8] = include_bytes!("../assets/java/JavaInfo.class");

/// Tools that only ever ship with a development kit, never with a runtime image.
const JAVAC_BIN: &str = if cfg!(windows) { "javac.exe" } else { "javac" };
const JMOD_BIN: &str = if cfg!(windows) { "jmod.exe" } else { "jmod" };

/// The native library behind `java.awt`. An image linked without `java.desktop`
/// simply does not ship it.
#[cfg(target_os = "windows")]
const AWT_LIBRARY: &str = "awt.dll";
#[cfg(target_os = "macos")]
const AWT_LIBRARY: &str = "libawt.dylib";
#[cfg(all(unix, not(target_os = "macos")))]
const AWT_LIBRARY: &str = "libawt.so";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaCheckInfo {
    pub version: String,
    pub vendor: String,
    pub os_arch: String,
    /// Whether this is a development kit rather than a bare runtime image.
    ///
    /// Minecraft starts on either, but mod loaders and the game's own tooling
    /// expect the kit, so the locator prefers one when both satisfy the
    /// requested major (see [`crate::locate::best_for_major`]).
    pub is_jdk: bool,
}

#[tracing::instrument(level = "debug", skip(absolute_path))]
pub async fn check_java_runtime(absolute_path: String) -> JavaResult<JavaCheckInfo> {
    let temp_dir = get_java_info_dir().await?;

    let mut command = tokio::process::Command::new(&absolute_path);
    command
        .arg("-cp")
        .arg(temp_dir)
        .arg("JavaInfo")
        .env_remove("_JAVA_OPTIONS");

    let program = command.as_std().get_program().to_string_lossy();
    let args: Vec<String> = command
        .as_std()
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();

    tracing::debug!("running command: {} {}", program, args.join(" "));

    let output = command.output().await
        .map_err(|e| JavaError::RuntimeCheckError {
            source: e,
            path: absolute_path.clone(),
        })?;

    let java_info = String::from_utf8_lossy(&output.stdout);

    let info = java_info
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once('=')?;

            Some((key.to_string(), value.to_string()))
        })
        .collect::<HashMap<_, _>>();

    let Some(version) = info.get("java.version").cloned() else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(
            path = %absolute_path,
            status = ?output.status.code(),
            "java probe did not report java.version; stderr: {}",
            stderr.trim()
        );
        return Err(JavaError::InvalidJavaPath { path: absolute_path });
    };

    let home = java_home(&absolute_path, info.get("java.home"));

    // Minecraft draws its window through AWT. A headless or hand-jlinked image
    // without `java.desktop` launches happily and then dies mid-game, so it is
    // rejected here rather than recorded as usable.
    if !has_awt_support(&info, &home) {
        tracing::warn!(
            path = %absolute_path,
            home = %home.display(),
            "java installation has no java.awt support"
        );
        return Err(JavaError::MissingAwtSupport { path: absolute_path });
    }

    Ok(JavaCheckInfo {
        os_arch: info
            .get("os.arch")
            .cloned()
            .unwrap_or_else(|| String::from("unknown")),
        version,
        vendor: info
            .get("java.vendor")
            .cloned()
            .unwrap_or_else(|| String::from("unknown")),
        is_jdk: probe_flag(&info, "java.jdk").unwrap_or(false) || is_jdk_home(&home),
    })
}

/// Whether the installation behind a `java` executable is a development kit.
///
/// Nothing is recorded about kit-ness in the store, so callers holding only a
/// path ask the filesystem again instead of the schema growing a column for a
/// hint this cheap to recompute.
pub(crate) fn is_jdk_executable(absolute_path: &str) -> bool {
    is_jdk_home(&java_home(absolute_path, None))
}

/// Whether `home` is a development kit rather than a runtime image.
///
/// Directory names lie in both directions (`.../jdk-21/jre`, runtimes unpacked
/// into a folder someone called `jdk`), so this only looks for artefacts a
/// runtime image never carries: the compiler, `jmod`, the JDK 8 `tools.jar`,
/// and the JNI headers.
pub(crate) fn is_jdk_home(home: &Path) -> bool {
    let bin = home.join("bin");

    bin.join(JAVAC_BIN).is_file()
        || bin.join(JMOD_BIN).is_file()
        || home.join("jmods").is_dir()
        || home.join("lib").join("tools.jar").is_file()
        || home.join("include").join("jni.h").is_file()
}

/// The installation root.
///
/// `java.home` as the probe reports it is authoritative; when the compiled
/// probe class predates that property we fall back to the executable's own
/// location, resolving symlinks first so a `/usr/bin/java` shim lands in the
/// real home rather than in `/usr`.
fn java_home(absolute_path: &str, reported: Option<&String>) -> PathBuf {
    if let Some(home) = reported.filter(|home| !home.is_empty()) {
        return PathBuf::from(home);
    }

    let executable = std::fs::canonicalize(absolute_path)
        .unwrap_or_else(|_| PathBuf::from(absolute_path));

    executable
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or(executable)
}

/// Whether `java.awt` is there: the probe's own answer when it gave one, and
/// otherwise what the installation ships on disk.
fn has_awt_support(info: &HashMap<String, String>, home: &Path) -> bool {
    if let Some(reported) = probe_flag(info, "java.awt") {
        return reported;
    }

    // Java 9+ images record the modules they were linked with.
    if let Some(modules) = release_modules(home) {
        return modules.split_whitespace().any(|module| module == "java.desktop");
    }

    // Java 8, and anything else without a `release` file: AWT is a native
    // library sitting next to the runtime.
    awt_library_exists(home)
}

/// The `MODULES` line of an image's `release` file, if it has one.
fn release_modules(home: &Path) -> Option<String> {
    let release = std::fs::read_to_string(home.join("release")).ok()?;

    release.lines().find_map(|line| {
        let modules = line.strip_prefix("MODULES=")?;
        Some(modules.trim_matches('"').to_string())
    })
}

fn awt_library_exists(home: &Path) -> bool {
    // Windows keeps native libraries in `bin` and everyone else in `lib`; a
    // JDK 8 also nests a whole runtime under `jre`.
    [
        home.join("lib"),
        home.join("bin"),
        home.join("jre").join("lib"),
        home.join("jre").join("bin"),
    ]
    .iter()
    .any(|dir| dir.join(AWT_LIBRARY).exists())
}

/// Reads a boolean the probe printed, `None` when it printed no such line.
fn probe_flag(info: &HashMap<String, String>, key: &str) -> Option<bool> {
    info.get(key).map(|value| value.eq_ignore_ascii_case("true"))
}

static TEMP_JAVA_INFO: OnceCell<TempDir> = OnceCell::const_new();

#[tracing::instrument(level = "debug")]
async fn get_java_info_dir() -> Result<&'static PathBuf, polyio::IOError> {
    let dir: Result<&TempDir, polyio::IOError> = TEMP_JAVA_INFO
        .get_or_try_init(async || {
            let dir = polyio::tempdir().await?;
            let file = dir.dir_path().join("JavaInfo.class");

            polyio::write(&file, JAVA_INFO_CLASS).await?;

            Ok(dir)
        })
        .await;

    Ok(dir?.dir_path())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(path, b"").expect("write file");
    }

    #[tokio::test]
    async fn a_runtime_image_is_not_mistaken_for_a_kit() {
        let dir = polyio::tempdir().await.expect("tempdir");
        let home = dir.dir_path();

        touch(&home.join("bin").join("java"));
        touch(&home.join("lib").join("modules"));

        assert!(!is_jdk_home(home));
    }

    #[tokio::test]
    async fn a_kit_is_recognised_by_its_compiler() {
        let dir = polyio::tempdir().await.expect("tempdir");
        let home = dir.dir_path();

        touch(&home.join("bin").join("java"));
        touch(&home.join("bin").join(JAVAC_BIN));

        assert!(is_jdk_home(home));
    }

    #[tokio::test]
    async fn a_kit_is_recognised_by_its_headers_alone() {
        let dir = polyio::tempdir().await.expect("tempdir");
        let home = dir.dir_path();

        touch(&home.join("bin").join("java"));
        touch(&home.join("include").join("jni.h"));

        assert!(is_jdk_home(home));
    }

    #[tokio::test]
    async fn a_legacy_kit_is_recognised_by_tools_jar() {
        let dir = polyio::tempdir().await.expect("tempdir");
        let home = dir.dir_path();

        touch(&home.join("lib").join("tools.jar"));

        assert!(is_jdk_home(home));
    }

    #[tokio::test]
    async fn the_directory_name_alone_proves_nothing() {
        let dir = polyio::tempdir().await.expect("tempdir");
        let home = dir.dir_path().join("jdk-21");

        touch(&home.join("bin").join("java"));

        assert!(!is_jdk_home(&home));
    }

    #[test]
    fn the_probe_has_the_last_word_on_awt() {
        let missing = Path::new("/nonexistent-java-home");

        assert!(has_awt_support(&info(&[("java.awt", "true")]), missing));
        assert!(!has_awt_support(&info(&[("java.awt", "false")]), missing));
    }

    #[tokio::test]
    async fn a_module_list_without_java_desktop_fails_the_check() {
        let dir = polyio::tempdir().await.expect("tempdir");
        let home = dir.dir_path();

        std::fs::write(
            home.join("release"),
            "JAVA_VERSION=\"21.0.3\"\nMODULES=\"java.base java.logging\"\n",
        )
        .expect("write release");

        assert!(!has_awt_support(&info(&[]), home));
    }

    #[tokio::test]
    async fn a_module_list_with_java_desktop_passes() {
        let dir = polyio::tempdir().await.expect("tempdir");
        let home = dir.dir_path();

        std::fs::write(
            home.join("release"),
            "JAVA_VERSION=\"21.0.3\"\nMODULES=\"java.base java.desktop java.logging\"\n",
        )
        .expect("write release");

        assert!(has_awt_support(&info(&[]), home));
    }

    #[tokio::test]
    async fn a_release_less_image_falls_back_to_the_native_library() {
        let dir = polyio::tempdir().await.expect("tempdir");
        let home = dir.dir_path();

        assert!(!has_awt_support(&info(&[]), home));

        touch(&home.join("lib").join(AWT_LIBRARY));

        assert!(has_awt_support(&info(&[]), home));
    }

    #[test]
    fn a_missing_home_falls_back_to_the_executable_location() {
        let home = java_home("/opt/jdk-21/bin/java", None);

        assert_eq!(home, PathBuf::from("/opt/jdk-21"));
    }

    #[test]
    fn a_reported_home_wins_over_the_executable_location() {
        let reported = String::from("/opt/real-home");
        let home = java_home("/usr/bin/java", Some(&reported));

        assert_eq!(home, PathBuf::from("/opt/real-home"));
    }
}
