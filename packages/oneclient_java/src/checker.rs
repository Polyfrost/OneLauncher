use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::Duration,
};

use async_tempfile::TempDir;
use tokio::sync::OnceCell;

use crate::error::{JavaError, JavaResult};

const JAVA_INFO_CLASS: &[u8] = include_bytes!("../assets/java/JavaInfo.class");

/// A probe that outlives this is treated as a broken install so a stuck
/// executable or wrapper cannot block runtime listing
const PROBE_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaCheckInfo {
    pub version: String,
    pub vendor: String,
    pub os_arch: String,
    /// Mod loaders and game tooling expect a JDK so the locator prefers one
    /// when both satisfy the requested major
    pub is_jdk: bool,
    /// Where the probed runtime thinks its files live, used to check that a
    /// JDK is complete rather than just launchable
    pub java_home: String,
}

#[tracing::instrument(level = "debug", skip(absolute_path))]
pub async fn check_java_runtime(absolute_path: String) -> JavaResult<JavaCheckInfo> {
    let temp_dir = get_java_info_dir().await?;

    let mut command = tokio::process::Command::new(&absolute_path);
    command
        .arg("-cp")
        .arg(temp_dir)
        .arg("JavaInfo")
        .env_remove("_JAVA_OPTIONS")
        .env_remove("JAVA_TOOL_OPTIONS")
        .env_remove("JDK_JAVA_OPTIONS")
        .kill_on_drop(true);
    oneclient_common::process::no_window(command.as_std_mut());
    oneclient_common::process::strip_appimage_env(command.as_std_mut());

    let program = command.as_std().get_program().to_string_lossy();
    let args: Vec<String> = command
        .as_std()
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();

    tracing::debug!("running command: {} {}", program, args.join(" "));

    let Ok(output) = tokio::time::timeout(PROBE_TIMEOUT, command.output()).await else {
        tracing::warn!(path = %absolute_path, "java probe timed out, killed it");
        return Err(JavaError::InvalidJavaPath {
            path: absolute_path,
        });
    };
    let output = output.map_err(|e| JavaError::RuntimeCheckError {
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
        return Err(JavaError::InvalidJavaPath {
            path: absolute_path,
        });
    };

    // Minecraft needs AWT a headless image launches fine and then dies mid-game
    if !has_usable_awt(&info) {
        tracing::warn!(
            path = %absolute_path,
            classes = probe_flag(&info, "java.awt"),
            natives = probe_flag(&info, "java.awt.natives"),
            links = probe_flag(&info, "java.awt.link"),
            "java installation has no usable java.awt support"
        );
        return Err(JavaError::MissingAwtSupport {
            path: absolute_path,
        });
    }

    Ok(JavaCheckInfo {
        os_arch: info
            .get("os.arch")
            .cloned()
            .unwrap_or_else(|| String::from("unknown")),
        version,
        vendor: vendor(&info),
        is_jdk: probe_flag(&info, "java.jdk"),
        java_home: info.get("java.home").cloned().unwrap_or_default(),
    })
}

/// A full probe plus a check that the JDK actually has the parts a game
/// tooling chain needs. A truncated extraction can still launch `java` while
/// the compiler, `jmods` or the JNI headers are gone.
#[tracing::instrument(level = "debug", skip(absolute_path))]
pub async fn check_jdk_runtime(absolute_path: String, major: u32) -> JavaResult<JavaCheckInfo> {
    let info = check_java_runtime(absolute_path.clone()).await?;
    if !jdk_is_complete(&info, major) {
        tracing::warn!(
            path = %absolute_path,
            home = %info.java_home,
            "java installation is missing JDK parts"
        );
        return Err(JavaError::IncompleteJdk {
            path: absolute_path,
        });
    }
    Ok(info)
}

fn jdk_is_complete(info: &JavaCheckInfo, major: u32) -> bool {
    if !info.is_jdk || info.java_home.is_empty() {
        return false;
    }

    let mut home = Path::new(&info.java_home);
    let javac = if cfg!(windows) { "javac.exe" } else { "javac" };
    if !home.join("bin").join(javac).is_file()
        && home
            .file_name()
            .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("jre"))
        && let Some(parent) = home.parent()
    {
        // Legacy JDKs report the nested jre as java.home
        home = parent;
    }
    if !home.join("bin").join(javac).is_file() || !home.is_dir() {
        return false;
    }

    if major <= 8 {
        // Java 8 keeps the compiler in tools.jar and has no jmods
        home.join("lib").join("tools.jar").is_file() && home.join("include").join("jni.h").is_file()
    } else {
        home.join("jmods").is_dir() && home.join("include").join("jni.h").is_file()
    }
}

fn vendor(info: &HashMap<String, String>) -> String {
    if info
        .get("java.vendor.version")
        .is_some_and(|version| version.contains("Oracle GraalVM"))
    {
        return String::from("Oracle GraalVM");
    }
    info.get("java.vendor")
        .cloned()
        .unwrap_or_else(|| String::from("unknown"))
}

fn has_usable_awt(info: &HashMap<String, String>) -> bool {
    if !probe_flag(info, "java.awt.link") {
        tracing::warn!("java.awt.link returned false. ignoring")
    }

    probe_flag(info, "java.awt") && probe_flag(info, "java.awt.natives")
}

fn probe_flag(info: &HashMap<String, String>, key: &str) -> bool {
    info.get(key)
        .is_some_and(|value| value.eq_ignore_ascii_case("true"))
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

    #[test]
    fn oracle_graalvm_is_told_apart_from_oracle_jdk() {
        let graalvm = info(&[
            ("java.vendor", "Oracle Corporation"),
            ("java.vendor.version", "Oracle GraalVM 21.0.5+9.1"),
        ]);
        assert_eq!(
            vendor(&graalvm)
                .parse::<crate::vendors::JavaVendor>()
                .unwrap(),
            crate::vendors::JavaVendor::GraalVm
        );
        let jdk = info(&[("java.vendor", "Oracle Corporation")]);
        assert_eq!(vendor(&jdk), "Oracle Corporation");
    }

    #[test]
    fn a_flag_the_probe_never_printed_is_false() {
        assert!(!probe_flag(&info(&[]), "java.awt"));
    }

    #[test]
    fn a_flag_is_read_regardless_of_case() {
        assert!(probe_flag(&info(&[("java.awt", "TRUE")]), "java.awt"));
        assert!(!probe_flag(&info(&[("java.awt", "false")]), "java.awt"));
    }

    fn awt(classes: &str, natives: &str, link: &str) -> HashMap<String, String> {
        info(&[
            ("java.awt", classes),
            ("java.awt.natives", natives),
            ("java.awt.link", link),
        ])
    }

    #[test]
    fn a_complete_image_is_usable() {
        assert!(has_usable_awt(&awt("true", "true", "true")));
    }

    #[test]
    fn awt_classes_without_their_natives_are_not_usable() {
        assert!(!has_usable_awt(&awt("true", "false", "true")));
    }

    #[test]
    fn a_link_probe_that_fails_does_not_rule_the_image_out() {
        assert!(has_usable_awt(&awt("true", "true", "false")));
        assert!(has_usable_awt(&info(&[
            ("java.awt", "true"),
            ("java.awt.natives", "true"),
        ])));
    }

    #[test]
    fn every_half_is_required() {
        assert!(!has_usable_awt(&info(&[("java.awt", "true")])));
        assert!(!has_usable_awt(&info(&[("java.awt.natives", "true")])));
        assert!(!has_usable_awt(&info(&[("java.awt.link", "true")])));
    }

    fn jdk_info(home: String, is_jdk: bool) -> JavaCheckInfo {
        JavaCheckInfo {
            version: "21.0.5".to_string(),
            vendor: "Test".to_string(),
            os_arch: "x64".to_string(),
            is_jdk,
            java_home: home,
        }
    }

    #[test]
    fn a_jdk_missing_its_parts_is_incomplete() {
        let root = std::env::temp_dir().join(format!("oneclient-jdk-check-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("bin")).unwrap();
        std::fs::create_dir_all(root.join("include")).unwrap();
        std::fs::create_dir_all(root.join("lib")).unwrap();
        let javac = if cfg!(windows) { "javac.exe" } else { "javac" };
        std::fs::write(root.join("bin").join(javac), b"").unwrap();
        std::fs::write(root.join("include").join("jni.h"), b"").unwrap();
        let home = root.to_string_lossy().into_owned();
        let info = jdk_info(home.clone(), true);

        assert!(!jdk_is_complete(&info, 21), "jmods are missing");
        std::fs::create_dir(root.join("jmods")).unwrap();
        assert!(jdk_is_complete(&info, 21));
        assert!(!jdk_is_complete(&jdk_info(home.clone(), false), 21));
        assert!(!jdk_is_complete(&jdk_info(String::new(), true), 21));

        // Java 8 keeps the compiler in tools.jar instead of jmods
        assert!(!jdk_is_complete(&info, 8));
        std::fs::write(root.join("lib").join("tools.jar"), b"").unwrap();
        assert!(jdk_is_complete(&info, 8));

        let _ = std::fs::remove_dir_all(&root);
    }
}
