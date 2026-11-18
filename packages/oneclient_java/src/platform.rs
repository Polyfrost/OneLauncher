//! Host detection is shared here but the *naming* stays per-vendor
//! each API spells the same platform differently (e.g. Adoptium `mac`/`alpine-linux`/`x32`)

use std::sync::OnceLock;

use crate::data::PackageArchive;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostArch {
    X86,
    X86_64,
    Arm,
    Aarch64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostOs {
    Windows,
    MacOs,
    /// Several vendors publish musl builds under a separate name
    Linux {
        musl: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostTarget {
    pub arch: HostArch,
    pub os: HostOs,
}

impl HostTarget {
    pub const CURRENT: Self = Self {
        arch: cfg_select! {
            target_arch = "x86" => HostArch::X86,
            target_arch = "x86_64" => HostArch::X86_64,
            target_arch = "arm" => HostArch::Arm,
            target_arch = "aarch64" => HostArch::Aarch64,
        },
        os: cfg_select! {
            target_os = "windows" => HostOs::Windows,
            target_os = "macos" => HostOs::MacOs,
            target_os = "linux" => cfg_select! {
                target_env = "musl" => HostOs::Linux { musl: true },
                _ => HostOs::Linux { musl: false },
            },
        },
    };

    #[must_use]
    pub fn native() -> Self {
        static NATIVE: OnceLock<HostArch> = OnceLock::new();
        let arch = *NATIVE.get_or_init(|| {
            let arch = detect_native_arch().unwrap_or(Self::CURRENT.arch);
            tracing::info!(?arch, built_for = ?Self::CURRENT.arch, "detected the native CPU architecture");
            arch
        });
        Self {
            arch,
            os: Self::CURRENT.os,
        }
    }

    #[must_use]
    pub fn runtime_arches() -> Vec<HostArch> {
        runtime_arches(Self::native().arch, Self::CURRENT.arch)
    }

    /// Liberica takes architecture and bitness as separate query parameters
    #[must_use]
    pub const fn bitness(self) -> &'static str {
        match self.arch {
            HostArch::X86 | HostArch::Arm => "32",
            HostArch::X86_64 | HostArch::Aarch64 => "64",
        }
    }

    #[must_use]
    pub const fn archive(self) -> PackageArchive {
        match self.os {
            HostOs::Windows => PackageArchive::Zip,
            _ => PackageArchive::TarGz,
        }
    }

    #[must_use]
    pub const fn archive_ext(self) -> &'static str {
        match self.os {
            HostOs::Windows => "zip",
            _ => "tar.gz",
        }
    }

    #[must_use]
    pub const fn is_musl(self) -> bool {
        matches!(self.os, HostOs::Linux { musl: true })
    }
}

impl HostArch {
    #[must_use]
    pub const fn os_arch(self) -> &'static str {
        match self {
            Self::X86 => "x86",
            Self::X86_64 => "amd64",
            Self::Arm => "arm",
            Self::Aarch64 => "aarch64",
        }
    }

    #[must_use]
    pub fn from_os_arch(os_arch: &str) -> Option<Self> {
        match os_arch {
            "amd64" | "x86_64" | "x64" => Some(Self::X86_64),
            "aarch64" | "arm64" => Some(Self::Aarch64),
            "x86" | "i386" | "i486" | "i586" | "i686" => Some(Self::X86),
            "arm" => Some(Self::Arm),
            _ => None,
        }
    }
}

#[must_use]
pub fn runs_natively(os_arch: &str) -> bool {
    HostArch::from_os_arch(os_arch).is_none_or(|arch| arch == HostTarget::native().arch)
}

#[must_use]
pub fn wrong_arch_for_game(os_arch: &str, native_ok: bool) -> bool {
    let native = HostTarget::native();
    native != HostTarget::CURRENT
        && HostArch::from_os_arch(os_arch).is_some_and(|arch| (arch == native.arch) != native_ok)
}

fn runtime_arches(native: HostArch, built_for: HostArch) -> Vec<HostArch> {
    if native == built_for {
        vec![native]
    } else {
        vec![native, built_for]
    }
}

#[cfg(any(windows, test))]
fn arch_from_machine(machine: u16) -> Option<HostArch> {
    match machine {
        0x8664 => Some(HostArch::X86_64),
        0xAA64 => Some(HostArch::Aarch64),
        0x014C => Some(HostArch::X86),
        0x01C4 => Some(HostArch::Arm),
        _ => None,
    }
}

#[cfg(windows)]
fn detect_native_arch() -> Option<HostArch> {
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    use windows_sys::core::BOOL;

    type IsWow64Process2 = unsafe extern "system" fn(HANDLE, *mut u16, *mut u16) -> BOOL;

    let kernel32: Vec<u16> = "kernel32.dll\0".encode_utf16().collect();
    unsafe {
        let module = GetModuleHandleW(kernel32.as_ptr());
        if module.is_null() {
            return None;
        }
        let proc = GetProcAddress(module, c"IsWow64Process2".as_ptr().cast())?;
        let is_wow64_process2: IsWow64Process2 = std::mem::transmute(proc);

        let mut process = 0u16;
        let mut native = 0u16;
        if is_wow64_process2(GetCurrentProcess(), &raw mut process, &raw mut native) == 0 {
            return None;
        }
        arch_from_machine(native)
    }
}

#[cfg(target_os = "macos")]
fn detect_native_arch() -> Option<HostArch> {
    let mut translated: libc::c_int = 0;
    let mut size = size_of::<libc::c_int>();
    let ret = unsafe {
        libc::sysctlbyname(
            c"sysctl.proc_translated".as_ptr(),
            (&raw mut translated).cast(),
            &raw mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    (ret == 0 && translated == 1).then_some(HostArch::Aarch64)
}

#[cfg(not(any(windows, target_os = "macos")))]
fn detect_native_arch() -> Option<HostArch> {
    None
}

#[cfg(windows)]
const GPU_PREFERENCES_KEY: &str = r"HKCU\Software\Microsoft\DirectX\UserGpuPreferences";

/// A JVM is started through either stub so both are registered
#[cfg(windows)]
const GPU_PREFERENCE_EXECUTABLES: [&str; 2] = ["javaw.exe", "java.exe"];

/// Absolute so a `reg.exe` earlier in the search order cannot run in its place
#[cfg(windows)]
fn reg_command() -> tokio::process::Command {
    const NO_WINDOW: u32 = 0x0800_0000;

    let system_root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    let exe = std::path::Path::new(&system_root)
        .join("System32")
        .join("reg.exe");

    let mut command = tokio::process::Command::new(exe);
    command.creation_flags(NO_WINDOW);
    command
}

#[cfg(windows)]
fn registered_value<'a>(query: &'a str, name: &str) -> Option<&'a str> {
    query.lines().find_map(|line| {
        let (value_name, data) = line.split_once("REG_SZ")?;
        if value_name.trim().eq_ignore_ascii_case(name) {
            Some(data.trim())
        } else {
            None
        }
    })
}

#[cfg(windows)]
pub async fn set_gpu_preference(java_path: &std::path::Path, preference: u8) {
    let Some(dir) = java_path.parent() else {
        return;
    };

    let mut targets = Vec::new();
    for exe in GPU_PREFERENCE_EXECUTABLES {
        let path = dir.join(exe);
        if polyio::try_exists(&path).await.unwrap_or(false) {
            targets.push(path);
        }
    }

    if targets.is_empty() {
        return;
    }

    // One query for the whole key instead of one per executable
    let query = match reg_command()
        .args(["query", GPU_PREFERENCES_KEY])
        .output()
        .await
    {
        Ok(out) => String::from_utf8_lossy(&out.stdout).into_owned(),
        Err(err) => {
            tracing::warn!(%err, "could not run reg.exe");
            return;
        }
    };

    let data = format!("GpuPreference={preference};");
    for path in targets {
        if registered_value(&query, &path.to_string_lossy()) == Some(data.as_str()) {
            continue;
        }

        let added = reg_command()
            .args(["add", GPU_PREFERENCES_KEY, "/v"])
            .arg(&path)
            .args(["/t", "REG_SZ", "/d", data.as_str(), "/f"])
            .output()
            .await;

        match added {
            Ok(out) if out.status.success() => {
                tracing::info!(path = %path.display(), preference, "registered JVM GPU preference");
            }
            Ok(out) => tracing::warn!(
                path = %path.display(),
                stderr = %String::from_utf8_lossy(&out.stderr).trim(),
                "could not set GPU preference"
            ),
            Err(err) => tracing::warn!(%err, "could not run reg.exe"),
        }
    }
}

/// Runtimes install into version-stamped folders so without this the key keeps
/// a dead entry for every upgrade
#[cfg(windows)]
pub async fn forget_dedicated_gpu(java_path: &std::path::Path) {
    let Some(dir) = java_path.parent() else {
        return;
    };

    for exe in GPU_PREFERENCE_EXECUTABLES {
        let path = dir.join(exe);
        let removed = reg_command()
            .args(["delete", GPU_PREFERENCES_KEY, "/v"])
            .arg(&path)
            .arg("/f")
            .output()
            .await;

        match removed {
            Ok(out) if out.status.success() => {
                tracing::info!(path = %path.display(), "removed GPU preference");
            }
            // A value that was never written is reported as an error
            Ok(_) => {}
            Err(err) => tracing::warn!(%err, "could not run reg.exe"),
        }
    }
}

#[cfg(not(windows))]
pub async fn set_gpu_preference(_java_path: &std::path::Path, _preference: u8) {}

#[cfg(not(windows))]
pub async fn forget_dedicated_gpu(_java_path: &std::path::Path) {}

#[must_use]
pub fn is_legacy_macos() -> bool {
    #[cfg(target_os = "macos")]
    {
        static LEGACY: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *LEGACY.get_or_init(|| {
            let plist = std::fs::read_to_string("/System/Library/CoreServices/SystemVersion.plist")
                .unwrap_or_default();
            let legacy = plist_is_legacy_macos(&plist);
            tracing::info!(legacy, "checked the macOS version");
            legacy
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

#[cfg(any(target_os = "macos", test))]
fn plist_is_legacy_macos(plist: &str) -> bool {
    let Some(version) = plist
        .split_once("<key>ProductVersion</key>")
        .and_then(|(_, rest)| rest.split_once("<string>"))
        .and_then(|(_, rest)| rest.split_once("</string>"))
        .map(|(version, _)| version.trim())
    else {
        return false;
    };

    let mut parts = version.split('.').map(|part| part.parse::<u32>().ok());
    match (parts.next().flatten(), parts.next().flatten()) {
        (Some(major), _) if major < 10 => true,
        (Some(10), Some(minor)) => minor < 16,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plist(version: &str) -> String {
        format!(
            "<dict>\n\t<key>ProductName</key>\n\t<string>macOS</string>\n\t<key>ProductVersion</key>\n\t<string>{version}</string>\n</dict>"
        )
    }

    #[test]
    fn only_macos_10_is_legacy() {
        assert!(plist_is_legacy_macos(&plist("10.15.7")));
        assert!(plist_is_legacy_macos(&plist("10.14")));
        assert!(!plist_is_legacy_macos(&plist("10.16")));
        assert!(!plist_is_legacy_macos(&plist("11.7.10")));
        assert!(!plist_is_legacy_macos(&plist("15.1")));
        assert!(!plist_is_legacy_macos(""));
    }

    #[test]
    fn an_emulated_launcher_falls_back_to_its_own_architecture() {
        assert_eq!(
            runtime_arches(HostArch::Aarch64, HostArch::X86_64),
            vec![HostArch::Aarch64, HostArch::X86_64]
        );
        assert_eq!(
            runtime_arches(HostArch::X86_64, HostArch::X86_64),
            vec![HostArch::X86_64]
        );
    }

    #[test]
    fn windows_machine_types_map_to_architectures() {
        assert_eq!(arch_from_machine(0xAA64), Some(HostArch::Aarch64));
        assert_eq!(arch_from_machine(0x8664), Some(HostArch::X86_64));
        assert_eq!(arch_from_machine(0x014C), Some(HostArch::X86));
        assert_eq!(arch_from_machine(0), None);
    }

    #[test]
    fn jvm_os_arch_spellings_are_understood() {
        assert_eq!(HostArch::from_os_arch("amd64"), Some(HostArch::X86_64));
        assert_eq!(HostArch::from_os_arch("x86_64"), Some(HostArch::X86_64));
        assert_eq!(HostArch::from_os_arch("aarch64"), Some(HostArch::Aarch64));
        assert_eq!(HostArch::from_os_arch("ppc64le"), None);
        assert!(runs_natively("ppc64le"));
    }

    #[test]
    fn bitness_matches_the_architecture() {
        assert_eq!(HostTarget::CURRENT.bitness(), {
            if cfg!(target_pointer_width = "64") {
                "64"
            } else {
                "32"
            }
        });
    }

    #[test]
    fn windows_ships_zips_and_everything_else_tarballs() {
        let expected = if cfg!(windows) {
            PackageArchive::Zip
        } else {
            PackageArchive::TarGz
        };
        assert_eq!(HostTarget::CURRENT.archive(), expected);
    }
}
