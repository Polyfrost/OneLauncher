use std::fs::{self, OpenOptions, Permissions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const HELPER: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/gamemode"));
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

pub(super) fn prepare(root: &Path) -> io::Result<PathBuf> {
    let target = root.join("helpers/gamemode");
    install(&target, HELPER, 0o755)?;
    Ok(target)
}

fn install(target: &Path, bytes: &[u8], mode: u32) -> io::Result<()> {
    let directory = target.parent().expect("helper bundle directory");
    if fs::symlink_metadata(target).is_ok_and(|info| info.is_file())
        && fs::read(target).is_ok_and(|installed| installed == bytes)
        && fs::metadata(target)?.permissions().mode() & 0o777 == mode
    {
        return Ok(());
    }

    fs::create_dir_all(directory)?;
    let (temporary, mut file) = loop {
        let temporary = directory.join(format!(
            ".game-mode-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o700)
            .open(&temporary)
        {
            Ok(file) => break (temporary, file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };
    let result = (|| {
        file.write_all(bytes)?;
        file.set_permissions(Permissions::from_mode(mode))?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, target)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
