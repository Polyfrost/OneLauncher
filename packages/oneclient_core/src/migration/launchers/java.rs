//! Which Java major an instance ran on in its old launcher
//!
//! Its JVM arguments were written for that Java; flags from a newer one stop
//! an older JVM from starting at all ("Unrecognized VM option"), so the import
//! needs to know before it carries them over

use std::path::Path;

/// `1.8.0_371` and `8u371` are Java 8; `17.0.8`, `21` and `21-ea` read as written
pub fn major_from_version(version: &str) -> Option<u32> {
    let version = version.trim().trim_matches('"');
    let digits = |s: &str| -> Option<u32> {
        s.chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse()
            .ok()
    };

    let first = digits(version)?;
    if first == 1 {
        // Legacy scheme: 1.<major>.0_<update>
        return version.split('.').nth(1).and_then(digits);
    }
    Some(first)
}

/// Every JDK and JRE ships a `release` file at its root with
/// `JAVA_VERSION="17.0.8"`; `java_path` may be the executable or the root
pub async fn major_from_install(java_path: &Path) -> Option<u32> {
    let beside_bin = java_path
        .parent()
        .and_then(Path::parent)
        .map(|root| root.join("release"));
    let candidates = std::iter::once(java_path.join("release")).chain(beside_bin);

    for release in candidates {
        let Ok(text) = polyio::read_to_string(&release).await else {
            continue;
        };
        let found = text.lines().find_map(|line| {
            let value = line.trim().strip_prefix("JAVA_VERSION=")?;
            major_from_version(value)
        });
        if found.is_some() {
            return found;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_both_version_schemes() {
        assert_eq!(major_from_version("1.8.0_371"), Some(8));
        assert_eq!(major_from_version("\"17.0.8\""), Some(17));
        assert_eq!(major_from_version("21"), Some(21));
        assert_eq!(major_from_version("21-ea"), Some(21));
        assert_eq!(major_from_version("25.0.1+8"), Some(25));
        assert_eq!(major_from_version(""), None);
        assert_eq!(major_from_version("java"), None);
    }

    #[tokio::test]
    async fn finds_the_release_file_from_the_executable_or_the_root() {
        let home = polyio::tempdir().await.unwrap();
        let root = home.dir_path();
        polyio::create_dir_all(root.join("bin")).await.unwrap();
        polyio::write(
            root.join("release"),
            "IMPLEMENTOR=\"Eclipse Adoptium\"\nJAVA_VERSION=\"21.0.4\"\n",
        )
        .await
        .unwrap();

        assert_eq!(
            major_from_install(&root.join("bin/javaw.exe")).await,
            Some(21)
        );
        assert_eq!(major_from_install(root).await, Some(21));
        assert_eq!(
            major_from_install(&root.join("nowhere/bin/java")).await,
            None
        );
    }
}
