use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use futures_util::TryStreamExt;
use oneclient_common::paths;
use oneclient_events::{EventBus, GroupedProgressSession, TaskCategory};
use oneclient_net::RequestClient;
use polyio::Checksum;
use reqwest::{Method, Request};
use serde::Deserialize;
use url::Url;

use crate::data::JavaPackage;
use crate::error::{JavaError, JavaResult};
use crate::platform::{HostArch, HostOs, HostTarget};
use crate::vendors::{JavaRuntimeProvider, JavaVendor};

pub struct MojangRuntimeProvider;

const INDEX_URL: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

const FILE_CONCURRENCY: usize = 16;

pub(crate) const INSTALL_PREFIX: &str = "mojang-";

#[derive(Deserialize)]
struct IndexEntry {
    manifest: IndexManifest,
    version: IndexVersion,
}

#[derive(Deserialize)]
struct IndexManifest {
    sha1: String,
    url: String,
}

#[derive(Deserialize)]
struct IndexVersion {
    name: String,
}

#[derive(Deserialize)]
struct RuntimeManifest {
    files: BTreeMap<String, ManifestEntry>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum ManifestEntry {
    Directory,
    File {
        #[serde(default)]
        executable: bool,
        downloads: FileDownloads,
    },
    Link {
        target: String,
    },
}

#[derive(Deserialize)]
struct FileDownloads {
    raw: RawDownload,
}

#[derive(Deserialize)]
struct RawDownload {
    sha1: String,
    size: u64,
    url: String,
}

#[async_trait::async_trait]
impl JavaRuntimeProvider for MojangRuntimeProvider {
    fn vendor(&self) -> JavaVendor {
        JavaVendor::Mojang
    }

    #[tracing::instrument(level = "debug", skip(self, net))]
    async fn list_packages(
        &self,
        major: Option<u32>,
        net: &RequestClient,
    ) -> JavaResult<Vec<JavaPackage>> {
        let Some(platform) = MOJANG_PLATFORM else {
            return Ok(Vec::new());
        };

        let mut index = net
            .send_as::<BTreeMap<String, BTreeMap<String, Vec<IndexEntry>>>>(Request::new(
                Method::GET,
                Url::parse(INDEX_URL)?,
            ))
            .await?;

        let Some(components) = index.remove(platform) else {
            return Ok(Vec::new());
        };

        let mut packages: Vec<JavaPackage> = components
            .into_iter()
            .filter(|(component, _)| {
                (component.starts_with("java-runtime-") || component == "jre-legacy")
                    && !component.ends_with("-snapshot")
            })
            .filter_map(|(component, entries)| {
                let entry = entries.into_iter().next()?;
                let this_major = leading_major(&entry.version.name)?;
                if major.is_some_and(|filter| filter != this_major) {
                    return None;
                }

                Some(
                    JavaPackage {
                        download_url: entry.manifest.url,
                        name: format!("{INSTALL_PREFIX}{component}-{}", entry.version.name),
                        java_version: vec![this_major],
                        vendor: JavaVendor::Mojang,
                        archive: HostTarget::CURRENT.archive(),
                        checksum: None,
                        size: None,
                    }
                    .with_checksum(Some(Checksum::sha1(entry.manifest.sha1))),
                )
            })
            .collect();

        packages.sort_by_key(|p| std::cmp::Reverse(p.java_version.first().copied().unwrap_or(0)));
        tracing::debug!(count = packages.len(), "listed Mojang packages");
        Ok(packages)
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn install_package(
        &self,
        package: &JavaPackage,
        net: &RequestClient,
        events: &EventBus,
        progress: Option<&GroupedProgressSession>,
    ) -> JavaResult<PathBuf> {
        let root = paths::java_dir()?.join(polyio::sanitize_path(&package.name));
        if root.exists() {
            polyio::remove_dir_all(&root).await?;
        }

        let result = install_files(package, &root, net, events, progress).await;
        if result.is_err() {
            let _ = polyio::remove_dir_all(&root).await;
        }
        result?;

        let executable = {
            let package = package.clone();
            tokio::task::spawn_blocking(move || {
                crate::install::resolve_installed_executable(&root, &package)
            })
            .await
            .map_err(|err| polyio::IOError::from(std::io::Error::other(err)))?
        };

        tracing::info!(name = %package.name, "installed Mojang Java runtime");
        Ok(executable)
    }
}

async fn install_files(
    package: &JavaPackage,
    root: &Path,
    net: &RequestClient,
    events: &EventBus,
    progress: Option<&GroupedProgressSession>,
) -> JavaResult<()> {
    let manifest_body = net
        .send(Request::new(
            Method::GET,
            Url::parse(&package.download_url)?,
        ))
        .await?
        .bytes()
        .await
        .map_err(oneclient_net::RequestError::from)?;

    if let Some(expected) = &package.checksum
        && !polyio::sha1_bytes(&manifest_body).eq_ignore_ascii_case(&expected.hex)
    {
        return Err(malformed(package));
    }

    let manifest: RuntimeManifest =
        serde_json::from_slice(&manifest_body).map_err(|_| malformed(package))?;

    let mut files = Vec::new();
    let mut links = Vec::new();
    for (path, entry) in manifest.files {
        let relative =
            contained(Path::new(""), Path::new(&path)).ok_or_else(|| malformed(package))?;
        match entry {
            ManifestEntry::Directory => polyio::create_dir_all(root.join(&relative)).await?,
            ManifestEntry::File {
                executable,
                downloads,
            } => files.push((relative, executable, downloads.raw)),
            ManifestEntry::Link { target } => {
                let parent = relative.parent().unwrap_or(Path::new(""));
                let target =
                    contained(parent, Path::new(&target)).ok_or_else(|| malformed(package))?;
                links.push((relative, target));
            }
        }
    }

    let total: u64 = files.iter().map(|(_, _, raw)| raw.size).sum();
    let major = package.java_version.first().copied().unwrap_or(0);
    let child = progress.map(|session| {
        session.child(
            format!("{} {major}", package.vendor),
            total,
            TaskCategory::Java,
        )
    });
    let done = AtomicU64::new(0);

    futures_util::stream::iter(files.into_iter().map(Ok::<_, JavaError>))
        .try_for_each_concurrent(FILE_CONCURRENCY, |(relative, executable, raw)| {
            let dest = root.join(relative);
            let child = child.as_ref();
            let done = &done;
            async move {
                oneclient_net::download_verified(
                    net,
                    events,
                    &raw.url,
                    &dest,
                    Some(&Checksum::sha1(&raw.sha1)),
                    raw.size,
                    None,
                )
                .await?;

                #[cfg(unix)]
                if executable {
                    use std::os::unix::fs::PermissionsExt;
                    tokio::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755))
                        .await
                        .map_err(polyio::IOError::from)?;
                }
                #[cfg(not(unix))]
                let _ = executable;

                let current = done.fetch_add(raw.size, Ordering::Relaxed) + raw.size;
                if let Some(child) = child {
                    child.set_progress(current, Some(total));
                }
                Ok(())
            }
        })
        .await?;

    for (link, target) in links {
        polyio::symlink_file(root.join(target), root.join(link)).await?;
    }

    Ok(())
}

fn malformed(package: &JavaPackage) -> JavaError {
    JavaError::ArchiveExtractFailed {
        archive: package.name.clone(),
    }
}

fn contained(base: &Path, relative: &Path) -> Option<PathBuf> {
    let mut parts: Vec<_> = base.components().collect();
    for component in relative.components() {
        match component {
            Component::Normal(_) => parts.push(component),
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.iter().collect())
}

fn leading_major(name: &str) -> Option<u32> {
    let digits: String = name.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

const MOJANG_PLATFORM: Option<&str> = match (HostTarget::CURRENT.os, HostTarget::CURRENT.arch) {
    (HostOs::Windows, HostArch::X86_64) => Some("windows-x64"),
    (HostOs::Windows, HostArch::X86) => Some("windows-x86"),
    (HostOs::Windows, HostArch::Aarch64) => Some("windows-arm64"),
    (HostOs::MacOs, HostArch::X86_64) => Some("mac-os"),
    (HostOs::MacOs, HostArch::Aarch64) => Some("mac-os-arm64"),
    (HostOs::Linux { musl: false }, HostArch::X86_64) => Some("linux"),
    (HostOs::Linux { musl: false }, HostArch::X86) => Some("linux-i386"),
    _ => None,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn majors_come_from_the_leading_digits() {
        assert_eq!(leading_major("8u74-cacert462b08"), Some(8));
        assert_eq!(leading_major("16.0.1.9.1_3"), Some(16));
        assert_eq!(leading_major("21.0.7"), Some(21));
        assert_eq!(leading_major("beta"), None);
    }

    #[test]
    fn paths_stay_inside_the_install_root() {
        let base = Path::new("jre.bundle/Contents/Home/legal/java.compiler");
        assert_eq!(
            contained(base, Path::new("../java.base/LICENSE")),
            Some(PathBuf::from(
                "jre.bundle/Contents/Home/legal/java.base/LICENSE"
            ))
        );
        assert_eq!(contained(Path::new(""), Path::new("../escape")), None);
        assert_eq!(contained(Path::new("a"), Path::new("../../escape")), None);
        assert_eq!(contained(Path::new(""), Path::new("/etc/passwd")), None);
    }
}
