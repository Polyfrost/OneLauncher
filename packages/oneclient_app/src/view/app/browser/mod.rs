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

/// How a browsed package is already present in the cluster.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum InstallSource {
    /// Installed on its own.
    Manual,
    /// Comes from one of the cluster's bundles.
    Bundled,
}

impl InstallSource {
    fn label(self) -> &'static str {
        match self {
            Self::Manual => "Installed",
            Self::Bundled => "Bundled",
        }
    }

    /// Bundled reads blue rather than green: the package is there, but the
    /// cluster's bundle put it there, not the user.
    fn color(self) -> Color {
        match self {
            Self::Manual => colors::success(),
            Self::Bundled => colors::code_info(),
        }
    }
}

/// One version of a browsed package that the cluster has.
#[derive(Clone, PartialEq)]
pub(crate) struct InstalledVersion {
    pub version_id: String,
    /// The artifact to remove. `None` when a bundle names this version but
    /// nothing is linked yet, which is a state the user cannot act on.
    pub hash: Option<String>,
    /// Whether the game loads this one. Only meaningful alongside a `hash`;
    /// a bundle pin with nothing linked is neither on nor off, and the badge
    /// that reads this never renders for one.
    pub enabled: bool,
    pub source: InstallSource,
}

/// A browsed package the cluster already has.
///
/// More than one version is not a state the launcher aims for, but nothing
/// stops a cluster reaching it, so this holds every one it finds rather than an
/// arbitrary winner — the version list marks each of them.
#[derive(Clone, PartialEq)]
pub(crate) struct Installed {
    /// How the package as a whole got here, for the badge on cards and the
    /// sidebar. A bundle claiming the project wins over a hand-installed copy.
    pub source: InstallSource,
    /// Every version in the cluster the provider recorded one for, in the order
    /// the cluster reports them.
    pub versions: Vec<InstalledVersion>,
}

impl Installed {
    pub fn is_version(&self, version_id: &str) -> bool {
        self.find_version(version_id).is_some()
    }

    pub fn find_version(&self, version_id: &str) -> Option<&InstalledVersion> {
        self.versions.iter().find(|v| v.version_id == version_id)
    }

    /// Whether the cluster holds more than one copy of this package.
    ///
    /// Counts linked artifacts only: a bundle pin the cluster never downloaded
    /// is listed as a version but is not a second copy of anything.
    pub fn is_duplicated(&self) -> bool {
        self.versions.iter().filter(|v| v.hash.is_some()).count() > 1
    }
}

/// Indexes what the cluster already has by the project it came from, so a
/// search result can tell at a glance whether it's in there and how it got
/// there. Local files have no project to match a search result against and
/// are left out, as are a bundle's external (non-provider) files.
pub(crate) fn installed_map(
    content: Vec<LinkedArtifactInfo>,
    bundles: &[BundleWithUpdateStatus],
) -> HashMap<(ProviderId, String), Installed> {
    let mut map: HashMap<(ProviderId, String), Installed> = HashMap::new();

    for item in content {
        let (Some(provider), Some(project_id)) = (item.provider, item.project_id) else {
            continue;
        };

        // The entry is created even for an artifact with no recorded version:
        // the package is in the cluster either way, and the card badge only
        // asks that much. It just cannot be tied to a row in the version list.
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

    // Bundle membership wins: a bundle's files land in the cluster as ordinary
    // content, so they'd otherwise read as hand-installed.
    //
    // Every pinned version gets a row, whether or not the cluster has linked it
    // and whether or not some *other* version of the same project is linked. A
    // bundle names a version as a fact about the bundle; nothing the user does
    // to the cluster afterwards changes which version it ships, so nothing the
    // user does should be able to take the label off it.
    //
    // A pin with nothing linked still carries `hash: None`, which is what keeps
    // it from reading as installed: `is_duplicated` does not count it, and
    // `version_button` leaves it unpressable rather than offering a copy the
    // bundle would not own.
    for bundle in bundles {
        for (file, _status) in &bundle.files {
            if let BundleFileKind::Managed {
                provider,
                project_id,
                version_id,
                ..
            } = &file.kind
            {
                let installed = map
                    .entry((*provider, project_id.clone()))
                    .or_insert_with(|| Installed {
                        source: InstallSource::Bundled,
                        versions: Vec::new(),
                    });

                installed.source = InstallSource::Bundled;

                match installed
                    .versions
                    .iter_mut()
                    .find(|v| &v.version_id == version_id)
                {
                    // Linked: the artifact on disk is the truth, and the pin only
                    // says who it belongs to.
                    Some(version) => version.source = InstallSource::Bundled,
                    // Not linked: the pin is all there is to go on, so it becomes
                    // the row itself.
                    None => installed.versions.push(InstalledVersion {
                        version_id: version_id.clone(),
                        hash: None,
                        enabled: false,
                        source: InstallSource::Bundled,
                    }),
                }
            }
        }
    }

    map
}

pub(crate) fn installed_badge(installed: InstallSource, font_size: f32) -> impl IntoElement {
    badge(installed, font_size, installed.color().with_a(38), None)
}

/// The same badge sat on top of a card's artwork. The faint tint the in-card
/// badge uses has nothing to read against there, so this one brings its own
/// backdrop and outlines itself to stay legible over whatever the image is.
pub(crate) fn installed_badge_overlay(installed: InstallSource) -> impl IntoElement {
    badge(
        installed,
        10.,
        BANNER_BG.with_a(225),
        Some(installed.color().with_a(110)),
    )
}

/// Which of several installed versions the game actually loads.
///
/// Only worth showing when a package has more than one copy in the cluster —
/// with a single one "active" says nothing "installed" hasn't.
///
/// Reads the artifact's `enabled` flag rather than working the answer out
/// itself, because
/// [`reconcile_duplicate_activity`](oneclient_content::packages::reconcile_duplicate_activity)
/// has already resolved duplicates to the newest copy. Exactly one row in a
/// duplicated set should read Active; more than one means that pass has not run
/// since the copies appeared.
pub(crate) fn activity_badge(active: bool) -> impl IntoElement {
    let (text, color) = if active {
        ("Active", colors::brand())
    } else {
        ("Inactive", colors::fg_secondary())
    };

    // No icon, unlike the source badges it sits beside: three pills in a row
    // all wearing a tick reads as decoration rather than as three facts.
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
    fn a_bundle_pin_survives_another_version_being_installed() {
        // The bug this replaces a test for: the pin used to be listed only when
        // the project had no entry at all, so installing any other version of it
        // created one and the pin was dropped on the floor — taking the Bundled
        // tag off the version list row with it. Which version a bundle ships is
        // a fact about the bundle, not about what the cluster has linked.
        let map = installed_map(
            vec![linked("sodium", Some("v2"), "hash-2")],
            &bundles(vec![managed("sodium", "v1")]),
        );

        let sodium = entry(&map, "sodium");
        assert_eq!(
            sodium.find_version("v1").map(|v| v.source),
            Some(InstallSource::Bundled),
            "the bundle still ships v1, whatever else was installed beside it"
        );
        assert_eq!(
            sodium.find_version("v1").map(|v| v.hash.clone()),
            Some(None),
            "still nothing linked for it, so still nothing to remove"
        );
        assert_eq!(
            sodium.find_version("v2").map(|v| v.source),
            Some(InstallSource::Manual),
        );
    }

    #[test]
    fn a_pin_the_cluster_never_downloaded_is_not_a_second_copy() {
        // The pin is listed beside the linked version, but only the linked one
        // is a copy of anything — the "Active" badge must stay off both.
        let map = installed_map(
            vec![linked("sodium", Some("v2"), "hash-2")],
            &bundles(vec![managed("sodium", "v1")]),
        );

        assert!(!entry(&map, "sodium").is_duplicated());
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
