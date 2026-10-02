mod index;
mod package;

pub use index::Browser;
pub(crate) use index::{browsable_type, encode_package_id};

mod modpack_prompt;
mod world_prompt;
use modpack_prompt::{ModpackVersionPrompt, minecraft_choices};
use world_prompt::WorldInstallPrompt;

/// Projects shipping both a mod and a data pack tag the mod files with a loader
fn preferred_version(
    versions: &[oneclient_content::packages::types::VersionSummary],
    content_type: oneclient_content::packages::ContentType,
) -> Option<&oneclient_content::packages::types::VersionSummary> {
    if content_type == oneclient_content::packages::ContentType::DataPack {
        versions
            .iter()
            .find(|v| v.loaders.is_empty())
            .or_else(|| versions.first())
    } else {
        versions.first()
    }
}
pub use package::BrowserPackage;

use std::collections::HashMap;

use freya::prelude::*;
use oneclient_content::packages::ProviderId;
use oneclient_core::{BundleFileKind, BundleWithUpdateStatus, LinkedArtifactInfo};
use oneclient_db::models::OverrideType;

use crate::components::{Button, Icon, IconType, set_enabled_action};
use crate::hooks::{
    ClusterAction, loaded_image, mutation_is_running, use_cached_image, use_cluster_mutation,
};
use crate::theme::colors;
use crate::ui::{ImageFallbackExt, border_all_color};

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
pub(crate) struct BundlePin {
    pub bundle_name: String,
    pub package_id: String,
    pub manifest_default: bool,
}

#[derive(Clone, PartialEq)]
pub(crate) struct InstalledVersion {
    pub version_id: String,
    /// The artifact to remove
    /// `None` when a bundle names this version but nothing is linked
    pub hash: Option<String>,
    pub enabled: bool,
    pub source: InstallSource,
    pub bundle: Option<BundlePin>,
}

impl InstalledVersion {
    pub fn enable_action(&self, cluster_id: i64) -> Option<ClusterAction> {
        let pin = self.bundle.as_ref()?;
        set_enabled_action(
            cluster_id,
            self.hash.as_deref(),
            Some(&pin.bundle_name),
            &pin.package_id,
            pin.manifest_default,
            true,
        )
    }
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

    pub fn disabled_bundled(&self) -> Option<&InstalledVersion> {
        if self.versions.iter().any(|v| v.enabled) {
            return None;
        }
        self.versions.iter().find(|v| v.bundle.is_some())
    }
}

/// Local files and a bundle's external files are left out they have no project id to match against
pub(crate) fn installed_map(
    content: Vec<LinkedArtifactInfo>,
    bundles: &[BundleWithUpdateStatus],
    overrides: &HashMap<(String, String), String>,
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
                bundle: None,
            });
        }
    }

    // Bundle membership wins a bundle's files would otherwise read as hand-installed
    // The manifest pin is only a fallback for a missing linked version
    for bundle in bundles {
        let bundle_name = &bundle.archive.manifest.name;
        for (file, _status) in &bundle.files {
            if let BundleFileKind::Managed {
                provider,
                project_id,
                version_id,
                ..
            } = &file.kind
            {
                let pin = BundlePin {
                    bundle_name: bundle_name.clone(),
                    package_id: file.kind.package_id(),
                    manifest_default: file.enabled,
                };
                match map.get_mut(&(*provider, project_id.clone())) {
                    Some(installed) => {
                        installed.source = InstallSource::Bundled;
                        let only_copy = installed.versions.len() == 1;
                        match installed
                            .versions
                            .iter_mut()
                            .find(|v| &v.version_id == version_id)
                        {
                            Some(version) => {
                                version.source = InstallSource::Bundled;
                                version.bundle.get_or_insert(pin);
                            }
                            None if only_copy => {
                                installed.versions[0].bundle.get_or_insert(pin);
                            }
                            None => {}
                        }
                    }
                    None => {
                        let user_override = overrides
                            .get(&(bundle_name.clone(), pin.package_id.clone()))
                            .and_then(|o| OverrideType::parse(o));
                        map.insert(
                            (*provider, project_id.clone()),
                            Installed {
                                source: InstallSource::Bundled,
                                versions: vec![InstalledVersion {
                                    version_id: version_id.clone(),
                                    hash: None,
                                    enabled: oneclient_core::effective_enabled(file, user_override),
                                    source: InstallSource::Bundled,
                                    bundle: Some(pin),
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

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum EnableVariant {
    Sidebar,
    Card { height: f32 },
    VersionRow,
}

#[derive(PartialEq)]
pub(crate) struct EnableButton {
    pub action: ClusterAction,
    pub variant: EnableVariant,
}

impl Component for EnableButton {
    fn render(&self) -> impl IntoElement {
        let mutation = use_cluster_mutation();
        let running = mutation_is_running(&mutation);
        let action = self.action.clone();

        let button = Button::new()
            .enabled(!running)
            .on_press(move |_| mutation.mutate(action.clone()));

        match self.variant {
            EnableVariant::Sidebar => button
                .primary()
                .width(Size::fill())
                .child(Icon::new(IconType::CheckCircle).size(14.))
                .text(if running { "Enabling..." } else { "Enable" })
                .into_element(),
            EnableVariant::Card { height } => rect()
                .on_press(|e: Event<PressEventData>| e.stop_propagation())
                .child(
                    button
                        .primary()
                        .small()
                        .height(Size::px(height))
                        .padding(Gaps::new_symmetric(0., 11.))
                        .child(
                            Icon::new(if running {
                                IconType::Loading02
                            } else {
                                IconType::CheckCircle
                            })
                            .size(12.)
                            .color(colors::fg_primary()),
                        )
                        .child(
                            label()
                                .text(if running { "Enabling" } else { "Enable" })
                                .font_size(11.)
                                .font_weight(FontWeight::SEMI_BOLD)
                                .max_lines(1)
                                .color(colors::fg_primary()),
                        ),
                )
                .into_element(),
            EnableVariant::VersionRow => button.secondary().small().text("Enable").into_element(),
        }
    }
}

pub(crate) fn installed_badge(installed: InstallSource, font_size: f32) -> impl IntoElement {
    badge(installed, font_size, installed.color().with_a(38), None)
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
        .child(label().text(text).font_size(11.).max_lines(1).color(color))
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

        let placeholder = thumbnail_placeholder(size, radius, 0.4);

        match loaded {
            Some((url, bytes)) => ImageViewer::new((url, bytes))
                .width(Size::px(size))
                .height(Size::px(size))
                .aspect_ratio(AspectRatio::Min)
                .corner_radius(CornerRadius::new_all(radius))
                .fallback(placeholder)
                .into_element(),
            None => placeholder,
        }
    }
}

fn thumbnail_placeholder(size: f32, radius: f32, icon_ratio: f32) -> Element {
    rect()
        .center()
        .width(Size::px(size))
        .height(Size::px(size))
        .corner_radius(CornerRadius::new_all(radius))
        .background(colors::component_bg())
        .child(
            Icon::new(IconType::DotsGrid)
                .size(size * icon_ratio)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

#[derive(PartialEq)]
pub(crate) struct PackageBanner {
    icon_url: Option<String>,
    height: f32,
    backdrop_only: bool,
    sharp: bool,
    key: DiffKey,
}

impl PackageBanner {
    pub fn new(icon_url: Option<String>, height: f32) -> Self {
        Self {
            icon_url,
            height,
            backdrop_only: false,
            sharp: false,
            key: DiffKey::None,
        }
    }

    /// Drops the centred icon so a caller can place its own artwork over the blur
    pub fn backdrop_only(mut self) -> Self {
        self.backdrop_only = true;
        self
    }

    pub fn sharp(mut self) -> Self {
        self.sharp = true;
        self
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
        let query = use_cached_image(self.icon_url.clone(), if self.sharp { 384 } else { 256 });
        let loaded = loaded_image(self.icon_url.as_deref(), &query);

        let banner = rect()
            .width(Size::fill())
            .height(Size::px(h))
            .center()
            .overflow(Overflow::Clip)
            .background(BANNER_BG);

        let icon_placeholder = thumbnail_placeholder(icon, 10., 0.45);
        let backdrop_only = self.backdrop_only;

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
                                .image_cover(ImageCover::Center)
                                .fallback(rect().width(Size::fill()).height(Size::fill())),
                        )
                        .layer(Layer::Relative(1)),
                )
                .maybe_child((!self.sharp).then(|| {
                    rect()
                        .position(Position::new_absolute().top(0.).left(0.))
                        .width(Size::fill())
                        .height(Size::fill())
                        .backdrop_blur(12.)
                        .background(BANNER_BG.with_a(120))
                        .overflow(Overflow::Clip)
                        .layer(Layer::Relative(3))
                        .into_element()
                }))
                .maybe_child((!backdrop_only).then(|| {
                    rect()
                        .width(Size::px(icon))
                        .height(Size::px(icon))
                        .child(
                            ImageViewer::new((url, bytes))
                                .width(Size::px(icon))
                                .height(Size::px(icon))
                                .aspect_ratio(AspectRatio::Min)
                                .corner_radius(CornerRadius::new_all(10.))
                                .fallback(icon_placeholder.clone()),
                        )
                        .layer(Layer::Relative(5))
                        .into_element()
                })),
            None if backdrop_only => banner,
            None => banner.child(icon_placeholder),
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
            seen_status: oneclient_core::SeenStatus::Seen,
        }
    }

    fn managed(project_id: &str, version_id: &str) -> BundleFile {
        BundleFile {
            enabled: true,
            hidden: false,
            path: format!("mods/{project_id}.jar"),
            size: 1,
            file_type: oneclient_core::BundleFileType::Normal,
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
            opted_in_types: [ContentType::Mod].into(),
        }]
    }

    fn installed_map(
        content: Vec<LinkedArtifactInfo>,
        bundles: &[BundleWithUpdateStatus],
    ) -> HashMap<(ProviderId, String), Installed> {
        super::installed_map(content, bundles, &HashMap::new())
    }

    fn optional(project_id: &str, version_id: &str) -> BundleFile {
        BundleFile {
            enabled: false,
            ..managed(project_id, version_id)
        }
    }

    fn overridden(
        project_id: &str,
        override_type: OverrideType,
    ) -> HashMap<(String, String), String> {
        HashMap::from([(
            ("performance".to_string(), project_id.to_string()),
            override_type.as_str().to_string(),
        )])
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
        assert!(
            sodium.is_version("v1"),
            "the older version is still in there"
        );
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
        assert_eq!(
            sodium.source,
            InstallSource::Bundled,
            "the bundle owns the project"
        );
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

    #[test]
    fn an_optional_mod_never_downloaded_offers_enable_through_the_bundle() {
        let map = installed_map(Vec::new(), &bundles(vec![optional("sodium", "v1")]));

        let sodium = entry(&map, "sodium");
        let disabled = sodium.disabled_bundled().expect("offers enable");
        assert!(matches!(
            disabled.enable_action(7),
            Some(ClusterAction::SetBundlePackageEnabled {
                cluster_id: 7,
                enabled: true,
                manifest_default: false,
                ..
            })
        ));
    }

    #[test]
    fn an_enabled_pin_waiting_for_sync_does_not_offer_enable() {
        let map = installed_map(Vec::new(), &bundles(vec![managed("sodium", "v1")]));

        assert!(entry(&map, "sodium").disabled_bundled().is_none());
    }

    #[test]
    fn a_disabled_override_on_an_unlinked_pin_offers_enable() {
        let map = super::installed_map(
            Vec::new(),
            &bundles(vec![managed("sodium", "v1")]),
            &overridden("sodium", OverrideType::Disabled),
        );

        let sodium = entry(&map, "sodium");
        assert!(matches!(
            sodium.disabled_bundled().and_then(|v| v.enable_action(7)),
            Some(ClusterAction::SetBundlePackageEnabled {
                manifest_default: true,
                ..
            })
        ));
    }

    #[test]
    fn a_disabled_linked_bundle_artifact_is_enabled_by_hash() {
        let map = installed_map(
            vec![disabled_linked("sodium", Some("v1"), "hash-1", false)],
            &bundles(vec![managed("sodium", "v1")]),
        );

        let sodium = entry(&map, "sodium");
        assert!(matches!(
            sodium.disabled_bundled().and_then(|v| v.enable_action(7)),
            Some(ClusterAction::SetArtifactEnabled { ref hash, enabled: true, .. }) if hash == "hash-1"
        ));
    }

    #[test]
    fn a_disabled_copy_from_an_older_bundle_pin_still_offers_enable() {
        let map = installed_map(
            vec![disabled_linked("sodium", Some("v1"), "hash-1", false)],
            &bundles(vec![managed("sodium", "v2")]),
        );

        assert!(entry(&map, "sodium").disabled_bundled().is_some());
    }

    #[test]
    fn another_enabled_copy_hides_enable() {
        let map = installed_map(
            vec![
                disabled_linked("sodium", Some("v1"), "hash-1", false),
                linked("sodium", Some("v2"), "hash-2"),
            ],
            &bundles(vec![managed("sodium", "v1")]),
        );

        assert!(
            entry(&map, "sodium").disabled_bundled().is_none(),
            "enabling the bundle copy would load two"
        );
    }

    #[test]
    fn a_hand_installed_disabled_mod_is_not_offered_enable() {
        let map = installed_map(
            vec![disabled_linked("sodium", Some("v1"), "hash-1", false)],
            &[],
        );

        assert!(entry(&map, "sodium").disabled_bundled().is_none());
    }
}
