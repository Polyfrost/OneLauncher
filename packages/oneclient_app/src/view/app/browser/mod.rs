mod index;
mod package;

pub use index::Browser;
pub use package::BrowserPackage;

use std::collections::HashMap;

use freya::prelude::*;
use oneclient_content::packages::ProviderId;
use oneclient_core::{BundleFileKind, BundleWithUpdateStatus, LinkedArtifactInfo};

use crate::components::{Icon, IconType};
use crate::hooks::{loaded_image, use_cached_image};
use crate::theme::colors;
use crate::ui::border_all_color;

const BANNER_BG: Color = Color::from_rgb(21, 28, 34);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum InstallSource {
    Manual,
    Bundled,
}

impl InstallSource {
    fn label(self) -> &'static str {
        match self {
            Self::Manual => "Installed",
            Self::Bundled => "Bundled",
        }
    }

    /// Bundled reads blue rather than green the cluster's bundle put it there not the user
    fn color(self) -> Color {
        match self {
            Self::Manual => colors::success(),
            Self::Bundled => colors::code_info(),
        }
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct InstalledVersion {
    pub version_id: String,
    /// The artifact to remove
    /// `None` when a bundle names this version but nothing is linked
    pub hash: Option<String>,
    /// Only meaningful alongside a `hash` a bundle pin with nothing linked is neither on nor off
    pub enabled: bool,
    pub source: InstallSource,
}

/// Holds every version found rather than an arbitrary winner a cluster can end up with more than one
#[derive(Clone, PartialEq)]
pub(crate) struct Installed {
    /// A bundle claiming the project wins over a hand-installed copy
    pub source: InstallSource,
    pub versions: Vec<InstalledVersion>,
}

impl Installed {
    pub fn is_version(&self, version_id: &str) -> bool {
        self.find_version(version_id).is_some()
    }

    pub fn find_version(&self, version_id: &str) -> Option<&InstalledVersion> {
        self.versions.iter().find(|v| v.version_id == version_id)
    }

    /// Counts linked artifacts only a bundle pin the cluster never downloaded is not a second copy
    pub fn is_duplicated(&self) -> bool {
        self.versions.iter().filter(|v| v.hash.is_some()).count() > 1
    }
}

/// Local files and a bundle's external files are left out they have no project id to match against
pub(crate) fn installed_map(
    content: Vec<LinkedArtifactInfo>,
    bundles: &[BundleWithUpdateStatus],
) -> HashMap<(ProviderId, String), Installed> {
    let mut map: HashMap<(ProviderId, String), Installed> = HashMap::new();

    for item in content {
        let (Some(provider), Some(project_id)) = (item.provider, item.project_id) else {
            continue;
        };

        // Created even without a recorded version the package is in the cluster it just cannot be tied to a version row
        let installed = map.entry((provider, project_id)).or_insert(Installed {
            source: InstallSource::Manual,
            versions: Vec::new(),
        });

        if let Some(version_id) = item.version_id {
            installed.versions.push(InstalledVersion {
                version_id,
                hash: Some(item.hash),
                enabled: item.enabled,
                source: InstallSource::Manual,
            });
        }
    }

    // Bundle membership wins a bundle's files would otherwise read as hand-installed
    // The manifest pin is only a fallback for a missing linked version
    for bundle in bundles {
        for (file, _status) in &bundle.files {
            if let BundleFileKind::Managed {
                provider,
                project_id,
                version_id,
                ..
            } = &file.kind
            {
                match map.get_mut(&(*provider, project_id.clone())) {
                    Some(installed) => {
                        installed.source = InstallSource::Bundled;
                        if let Some(version) = installed
                            .versions
                            .iter_mut()
                            .find(|v| &v.version_id == version_id)
                        {
                            version.source = InstallSource::Bundled;
                        }
                    }
                    None => {
                        map.insert(
                            (*provider, project_id.clone()),
                            Installed {
                                source: InstallSource::Bundled,
                                versions: vec![InstalledVersion {
                                    version_id: version_id.clone(),
                                    hash: None,
                                    enabled: false,
                                    source: InstallSource::Bundled,
                                }],
                            },
                        );
                    }
                }
            }
        }
    }

    map
}

pub(crate) fn installed_badge(installed: InstallSource, font_size: f32) -> impl IntoElement {
    badge(installed, font_size, installed.color().with_a(38), None)
}

/// Brings its own backdrop and outline so it stays legible over card artwork
pub(crate) fn installed_badge_overlay(installed: InstallSource) -> impl IntoElement {
    badge(
        installed,
        10.,
        BANNER_BG.with_a(225),
        Some(installed.color().with_a(110)),
    )
}

/// Which of several installed versions the game actually loads
pub(crate) fn activity_badge(active: bool) -> impl IntoElement {
    let (text, color) = if active {
        ("Active", colors::brand())
    } else {
        ("Inactive", colors::fg_secondary())
    };

    // No icon three pills in a row all wearing a tick reads as decoration
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .padding(Gaps::new_symmetric(2., 8.))
        .corner_radius(CornerRadius::new_all(999.))
        .background(color.with_a(38))
        .child(
            label()
                .text(text)
                .font_size(11.)
                .max_lines(1)
                .color(color),
        )
}

fn badge(
    installed: InstallSource,
    font_size: f32,
    background: Color,
    border: Option<Color>,
) -> impl IntoElement {
    let color = installed.color();

    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(4.)
        .padding(Gaps::new_symmetric(2., 8.))
        .corner_radius(CornerRadius::new_all(999.))
        .background(background)
        .map(border, |el, border| el.border(border_all_color(1., border)))
        .child(
            Icon::new(IconType::CheckCircle)
                .size(font_size)
                .color(color),
        )
        .child(
            label()
                .text(installed.label())
                .font_size(font_size)
                .max_lines(1)
                .color(color),
        )
}

#[derive(PartialEq)]
pub(crate) struct Thumbnail {
    icon_url: Option<String>,
    size: f32,
    radius: f32,
    key: DiffKey,
}

impl Thumbnail {
    pub fn new(icon_url: Option<String>, size: f32) -> Self {
        Self {
            icon_url,
            size,
            radius: 10.,
            key: DiffKey::None,
        }
    }

    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }
}

impl KeyExt for Thumbnail {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for Thumbnail {
    fn render(&self) -> impl IntoElement {
        let size = self.size;
        let radius = self.radius;
        let query = use_cached_image(self.icon_url.clone(), 256);
        let loaded = loaded_image(self.icon_url.as_deref(), &query);

        match loaded {
            Some((url, bytes)) => ImageViewer::new((url, bytes))
                .width(Size::px(size))
                .height(Size::px(size))
                .aspect_ratio(AspectRatio::Min)
                .corner_radius(CornerRadius::new_all(radius))
                .into_element(),
            None => rect()
                .center()
                .width(Size::px(size))
                .height(Size::px(size))
                .corner_radius(CornerRadius::new_all(radius))
                .background(colors::component_bg())
                .child(
                    Icon::new(IconType::DotsGrid)
                        .size(size * 0.4)
                        .color(colors::fg_secondary()),
                )
                .into_element(),
        }
    }
}

#[derive(PartialEq)]
pub(crate) struct PackageBanner {
    icon_url: Option<String>,
    height: f32,
    key: DiffKey,
}

impl PackageBanner {
    pub fn new(icon_url: Option<String>, height: f32) -> Self {
        Self {
            icon_url,
            height,
            key: DiffKey::None,
        }
    }
}

impl KeyExt for PackageBanner {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for PackageBanner {
    fn render(&self) -> impl IntoElement {
        let h = self.height;
        let icon = h * 0.62;
        let query = use_cached_image(self.icon_url.clone(), 512);
        let loaded = loaded_image(self.icon_url.as_deref(), &query);

        let banner = rect()
            .width(Size::fill())
            .height(Size::px(h))
            .center()
            .overflow(Overflow::Clip)
            .background(BANNER_BG);

        match loaded {
            Some((url, bytes)) => banner
                .child(
                    rect()
                        .position(Position::new_absolute().top(0.).left(0.))
                        .width(Size::fill())
                        .height(Size::fill())
                        .overflow(Overflow::Clip)
                        .child(
                            ImageViewer::new((url.clone(), bytes.clone()))
                                .width(Size::fill())
                                .height(Size::fill())
                                .aspect_ratio(AspectRatio::Max)
                                .image_cover(ImageCover::Center),
                        )
                        .layer(Layer::Relative(1)),
                )
                .child(
                    rect()
                        .position(Position::new_absolute().top(0.).left(0.))
                        .width(Size::fill())
                        .height(Size::fill())
                        .blur(12.)
                        .background(BANNER_BG.with_a(120))
                        .overflow(Overflow::Clip)
                        .layer(Layer::Relative(3)),
                )
                .child(
                    rect()
                        .width(Size::px(icon))
                        .height(Size::px(icon))
                        .child(
                            ImageViewer::new((url, bytes))
                                .width(Size::px(icon))
                                .height(Size::px(icon))
                                .aspect_ratio(AspectRatio::Min)
                                .corner_radius(CornerRadius::new_all(10.)),
                        )
                        .layer(Layer::Relative(5)),
                ),
            None => banner.child(
                rect()
                    .center()
                    .width(Size::px(icon))
                    .height(Size::px(icon))
                    .corner_radius(CornerRadius::new_all(10.))
                    .background(colors::component_bg())
                    .child(
                        Icon::new(IconType::DotsGrid)
                            .size(icon * 0.45)
                            .color(colors::fg_secondary()),
                    ),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::onboarding::test_support::archive;
    use oneclient_content::packages::ContentType;
    use oneclient_core::{BundleFile, FileUpdateStatus};

    fn linked(project_id: &str, version_id: Option<&str>, hash: &str) -> LinkedArtifactInfo {
        disabled_linked(project_id, version_id, hash, true)
    }

    fn disabled_linked(
        project_id: &str,
        version_id: Option<&str>,
        hash: &str,
        enabled: bool,
    ) -> LinkedArtifactInfo {
        LinkedArtifactInfo {
            hash: hash.to_string(),
            cluster_file_name: format!("{hash}.jar"),
            enabled,
            content_type: ContentType::Mod,
            file_name: format!("{hash}.jar"),
            project_id: Some(project_id.to_string()),
            version_id: version_id.map(Into::into),
            display_name: None,
            display_version: None,
            provider: Some(ProviderId::Modrinth),
            published_at: None,
        }
    }

    fn managed(project_id: &str, version_id: &str) -> BundleFile {
        BundleFile {
            enabled: true,
            hidden: false,
            path: format!("mods/{project_id}.jar"),
            size: 1,
            kind: BundleFileKind::Managed {
                provider: ProviderId::Modrinth,
                project_id: project_id.to_string(),
                version_id: version_id.to_string(),
                sha1: format!("sha-{version_id}"),
            },
        }
    }

    fn bundles(files: Vec<BundleFile>) -> Vec<BundleWithUpdateStatus> {
        vec![BundleWithUpdateStatus {
            files: files
                .iter()
                .cloned()
                .map(|f| (f, FileUpdateStatus::UpToDate))
                .collect(),
            archive: archive("performance", true, files),
            has_updates: false,
        }]
    }

    fn entry(map: &HashMap<(ProviderId, String), Installed>, project: &str) -> Installed {
        map.get(&(ProviderId::Modrinth, project.to_string()))
            .expect("project missing from the map")
            .clone()
    }

    #[test]
    fn every_installed_version_of_one_project_is_listed() {
        let map = installed_map(
            vec![
                linked("sodium", Some("v1"), "hash-1"),
                linked("sodium", Some("v2"), "hash-2"),
            ],
            &[],
        );

        let sodium = entry(&map, "sodium");
        assert!(sodium.is_version("v1"), "the older version is still in there");
        assert!(sodium.is_version("v2"));
        assert_eq!(
            sodium.find_version("v2").and_then(|v| v.hash.clone()),
            Some("hash-2".to_string()),
            "each version carries the artifact the remove button needs"
        );
    }

    #[test]
    fn a_bundle_marks_only_the_version_it_ships() {
        let map = installed_map(
            vec![
                linked("sodium", Some("v1"), "hash-1"),
                linked("sodium", Some("v2"), "hash-2"),
            ],
            &bundles(vec![managed("sodium", "v1")]),
        );

        let sodium = entry(&map, "sodium");
        assert_eq!(sodium.source, InstallSource::Bundled, "the bundle owns the project");
        assert_eq!(
            sodium.find_version("v1").map(|v| v.source),
            Some(InstallSource::Bundled)
        );
        assert_eq!(
            sodium.find_version("v2").map(|v| v.source),
            Some(InstallSource::Manual),
            "a version the user added themselves does not become the bundle's"
        );
    }

    #[test]
    fn an_unlinked_bundle_pin_is_the_fallback() {
        let map = installed_map(Vec::new(), &bundles(vec![managed("sodium", "v1")]));

        let sodium = entry(&map, "sodium");
        assert_eq!(sodium.source, InstallSource::Bundled);
        assert_eq!(
            sodium.find_version("v1").map(|v| v.hash.clone()),
            Some(None),
            "nothing is linked, so there is no artifact to remove"
        );
    }

    #[test]
    fn a_bundle_pin_is_not_listed_beside_a_linked_version() {
        let map = installed_map(
            vec![linked("sodium", Some("v2"), "hash-2")],
            &bundles(vec![managed("sodium", "v1")]),
        );

        let sodium = entry(&map, "sodium");
        assert!(
            !sodium.is_version("v1"),
            "a pin the cluster never downloaded is not installed"
        );
        assert_eq!(sodium.versions.len(), 1);
    }

    #[test]
    fn a_single_copy_is_not_a_duplicate() {
        let map = installed_map(vec![linked("sodium", Some("v1"), "hash-1")], &[]);

        assert!(!entry(&map, "sodium").is_duplicated());
    }

    #[test]
    fn an_unlinked_bundle_pin_is_not_a_second_copy() {
        let map = installed_map(Vec::new(), &bundles(vec![managed("sodium", "v1")]));

        assert!(
            !entry(&map, "sodium").is_duplicated(),
            "one pin and nothing linked is not two of anything"
        );
    }

    #[test]
    fn each_copy_reports_whether_the_game_loads_it() {
        let map = installed_map(
            vec![
                disabled_linked("sodium", Some("v1"), "hash-1", false),
                linked("sodium", Some("v2"), "hash-2"),
            ],
            &[],
        );

        let sodium = entry(&map, "sodium");
        assert!(sodium.is_duplicated());
        assert_eq!(sodium.find_version("v1").map(|v| v.enabled), Some(false));
        assert_eq!(sodium.find_version("v2").map(|v| v.enabled), Some(true));
    }

    #[test]
    fn an_artifact_with_no_recorded_version_still_marks_the_project() {
        let map = installed_map(vec![linked("sodium", None, "hash-1")], &[]);

        let sodium = entry(&map, "sodium");
        assert!(
            sodium.versions.is_empty(),
            "there is no version to tie to a row in the list"
        );
        assert_eq!(sodium.source, InstallSource::Manual);
    }
}
