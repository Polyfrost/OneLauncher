use std::path::{Path, PathBuf};


use crate::LauncherResult;

#[tracing::instrument(level = "debug", skip(exclude_top))]
pub async fn copy_tree(src: &Path, dst: &Path, exclude_top: &[&str]) -> LauncherResult<()> {
    let mut stack: Vec<(PathBuf, PathBuf, bool)> =
        vec![(src.to_path_buf(), dst.to_path_buf(), true)];

    while let Some((cur_src, cur_dst, is_top)) = stack.pop() {
        let mut entries = polyio::read_dir(&cur_src).await?;
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name();

            if is_top
                && let Some(name_str) = name.to_str()
                && exclude_top.iter().any(|e| e.eq_ignore_ascii_case(name_str))
            {
                continue;
            }

            let child_src = entry.path();
            let child_dst = cur_dst.join(&name);
            let file_type = entry.file_type().await?;

            if file_type.is_dir() {
                polyio::create_dir_all(&child_dst).await?;

                stack.push((child_src, child_dst, false));
            } else if file_type.is_file() {
                if let Some(parent) = child_dst.parent() {
                    polyio::create_dir_all(parent).await?;
                }

                polyio::copy(&child_src, &child_dst).await?;
            }
        }
    }

    Ok(())
}

pub async fn dir_has_content(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }

    match polyio::read_dir(dir).await {
        Ok(mut entries) => matches!(entries.next_entry().await, Ok(Some(_))),
        Err(_) => false,
    }
}
