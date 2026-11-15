use std::path::{Path, PathBuf};
use std::time::Duration;

use sysinfo::{Pid, ProcessesToUpdate, System};
use tokio::process::Command;

use crate::LauncherResult;
use crate::game::GameError;
use crate::state::LauncherState;

const JCMD_TIMEOUT: Duration = Duration::from_secs(15);

fn is_java(exe: &Path) -> bool {
    exe.file_stem()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("java") || s.eq_ignore_ascii_case("javaw"))
}

fn find_java(pid: u32) -> Option<(u32, PathBuf)> {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let processes: Vec<_> = sys
        .processes()
        .iter()
        .map(|(pid, p)| (pid.as_u32(), p.parent().map(Pid::as_u32), p.exe()))
        .collect();
    find_java_in(pid, &processes)
}

fn find_java_in(
    pid: u32,
    processes: &[(u32, Option<u32>, Option<&Path>)],
) -> Option<(u32, PathBuf)> {
    let mut queue = vec![pid];
    while let Some(current) = queue.pop() {
        for &(child, parent, exe) in processes {
            if child == current
                && let Some(exe) = exe
                && is_java(exe)
            {
                return Some((child, exe.to_path_buf()));
            }
            if parent == Some(current) {
                queue.push(child);
            }
        }
    }
    None
}

fn jcmd_beside(java: &Path) -> Option<PathBuf> {
    let jcmd = java
        .parent()?
        .join(format!("jcmd{}", std::env::consts::EXE_SUFFIX));
    jcmd.is_file().then_some(jcmd)
}

#[tracing::instrument(skip(state))]
pub async fn thread_dump(state: &LauncherState, cluster_id: i64) -> LauncherResult<PathBuf> {
    let pid = state
        .games
        .pid(cluster_id)
        .filter(|_| state.games.is_running(cluster_id))
        .ok_or(GameError::NotRunning)?;
    let (java_pid, java) = tokio::task::spawn_blocking(move || find_java(pid))
        .await
        .map_err(|err| GameError::ThreadDump(err.to_string()))?
        .ok_or_else(|| GameError::ThreadDump("couldn't find the game's Java process".into()))?;
    let jcmd = jcmd_beside(&java).ok_or(GameError::JcmdMissing)?;

    let mut command = Command::new(&jcmd);
    command
        .args([java_pid.to_string().as_str(), "Thread.print", "-l"])
        .kill_on_drop(true);
    oneclient_common::process::no_window(command.as_std_mut());
    oneclient_common::process::strip_appimage_env(command.as_std_mut());

    let output = tokio::time::timeout(JCMD_TIMEOUT, command.output())
        .await
        .map_err(|_| GameError::ThreadDump("jcmd timed out".into()))?
        .map_err(|err| GameError::ThreadDump(err.to_string()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let reason = [stderr.trim(), stdout.trim()]
            .into_iter()
            .find(|s| !s.is_empty())
            .unwrap_or("jcmd failed")
            .to_string();
        return Err(GameError::ThreadDump(reason).into());
    }

    let cluster = state.clusters.get(cluster_id).await?;
    let path = cluster.dir()?.join("logs").join(format!(
        "thread-dump-{}.log",
        chrono::Local::now().format("%Y-%m-%d_%H-%M-%S")
    ));
    polyio::create_dir_all(path.parent().unwrap_or(&path)).await?;
    polyio::write(&path, output.stdout).await?;
    tracing::info!(path = %path.display(), "wrote thread dump");
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn finds_jcmd_only_beside_java() {
        let dir = polyio::tempdir().await.expect("temp dir");
        let bin = dir.dir_path();
        let java = bin.join(format!("javaw{}", std::env::consts::EXE_SUFFIX));
        polyio::write(&java, b"").await.unwrap();
        assert_eq!(jcmd_beside(&java), None);

        let jcmd = bin.join(format!("jcmd{}", std::env::consts::EXE_SUFFIX));
        polyio::write(&jcmd, b"").await.unwrap();
        assert_eq!(jcmd_beside(&java), Some(jcmd));
    }

    #[test]
    fn finds_java_under_wrapper() {
        let wrapper = Path::new("/usr/bin/prime-run");
        let java = Path::new("/jdk/bin/java");
        let processes = [
            (1, None, Some(wrapper)),
            (2, Some(1), Some(Path::new("/usr/bin/env"))),
            (3, Some(2), Some(java)),
            (4, None, Some(Path::new("/other/bin/java"))),
        ];
        assert_eq!(find_java_in(1, &processes), Some((3, java.to_path_buf())));
        assert_eq!(find_java_in(3, &processes), Some((3, java.to_path_buf())));
        assert_eq!(find_java_in(2, &processes[..2]), None);
        assert_eq!(find_java_in(9, &processes), None);
    }

    #[test]
    fn recognises_java_executables() {
        assert!(is_java(Path::new("/jdk/bin/java")));
        assert!(is_java(Path::new("C:/jdk/bin/javaw.exe")));
        assert!(!is_java(Path::new("/usr/bin/prime-run")));
    }
}
