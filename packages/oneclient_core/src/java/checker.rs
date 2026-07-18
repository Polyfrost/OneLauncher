use std::{collections::HashMap, path::PathBuf};

use async_tempfile::TempDir;
use tokio::sync::OnceCell;

use crate::java::{JavaError, JavaResult};

const JAVA_INFO_CLASS: &[u8] = include_bytes!("../../assets/java/JavaInfo.class");

#[derive(Debug)]
pub struct JavaCheckInfo {
    pub version: String,
    pub vendor: String,
    pub os_arch: String,
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
            let mut parts = line.splitn(2, '=');
            let key = parts.next()?;
            let value = parts.next()?;

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
    })
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
