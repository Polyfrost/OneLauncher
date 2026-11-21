use freya::prelude::*;
use oneclient_content::packages::{ContentType, ProviderId};
use oneclient_core::clusters::Cluster;

use crate::components::{BundledInstallWarning, Button, PendingBundledInstall, centered_spinner};
use crate::hooks::{
    ClusterAction, VERSIONS_PAGE_SIZE, bundle_overrides_map, bundles_with_status_items,
    cluster_content_items, mutation_error, mutation_is_running, mutation_ok, query_error,
    query_is_busy, settled_or_loading, use_bundle_overrides, use_bundles_with_status,
    use_cluster_content, use_cluster_mutation, use_clusters, use_dispatch,
    use_package_versions_when,
};
use crate::i18n::{t, tf};
use crate::install::IncompatibleMode;
use crate::view::app::ClusterPickerModal;
use crate::view::app::cluster::dialog;

use super::{InstallSource, Installed, WorldInstallPrompt, installed_map, preferred_version};

#[derive(Clone, PartialEq)]
pub(super) struct InstallRequest {
    pub project_id: String,
    pub project_name: String,
    pub version: Option<(String, String)>,
}

#[derive(PartialEq)]
pub(super) struct BrowseInstallPicker {
    pub provider: ProviderId,
    pub content_type: ContentType,
    pub package_type: String,
    pub install: InstallRequest,
    pub on_close: EventHandler<()>,
}

impl Component for BrowseInstallPicker {
    fn render(&self) -> impl IntoElement {
        let mut target = use_state(|| None::<Cluster>);
        let clusters = settled_or_loading(&use_clusters());
        let only = clusters.map(|clusters| {
            clusters
                .iter()
                .filter(|cluster| {
                    super::browsable_type(&self.package_type, cluster) == self.package_type
                })
                .map(|cluster| cluster.id)
                .collect()
        });
        let on_close = self.on_close.clone();

        match target.read().clone() {
            None => ClusterPickerModal {
                selected: None,
                only,
                on_close,
                on_pick: (move |cluster| target.set(Some(cluster))).into(),
            }
            .into_element(),
            Some(cluster) => SelectedInstall {
                cluster,
                provider: self.provider,
                content_type: self.content_type,
                install: self.install.clone(),
                on_close,
                on_back: (move |()| target.set(None)).into(),
            }
            .into_element(),
        }
    }
}

#[derive(PartialEq)]
struct SelectedInstall {
    cluster: Cluster,
    provider: ProviderId,
    content_type: ContentType,
    install: InstallRequest,
    on_close: EventHandler<()>,
    on_back: EventHandler<()>,
}

#[derive(Clone, PartialEq)]
enum TargetIntent {
    AlreadyInstalled,
    OtherVersionEnabled,
    Enable(ClusterAction),
    BundleConflict(PendingBundledInstall),
    Install(String),
}

fn target_intent(
    request: &InstallRequest,
    provider: ProviderId,
    cluster_id: i64,
    installed: Option<&Installed>,
    version: Option<&str>,
) -> Option<TargetIntent> {
    if let Some(installed) = installed {
        if let Some((id, label)) = &request.version {
            if let Some(entry) = installed.disabled_bundled()
                && entry.version_id == *id
                && let Some(action) = entry.enable_action(cluster_id)
            {
                return Some(TargetIntent::Enable(action));
            }
            if let Some(version) = installed.find_version(id) {
                if version.bundle.is_some() && !version.enabled {
                    return Some(TargetIntent::OtherVersionEnabled);
                }
                return Some(TargetIntent::AlreadyInstalled);
            }
            if installed.conflicts_with_bundle(id) {
                return Some(TargetIntent::BundleConflict(PendingBundledInstall {
                    cluster_id,
                    provider,
                    project_id: request.project_id.clone(),
                    version_id: id.clone(),
                    project_name: request.project_name.clone(),
                    version_label: label.clone(),
                    bundled_version: installed.bundled_version_label(),
                    incompatible: IncompatibleMode::Ask,
                }));
            }
        } else if installed.source == InstallSource::Bundled {
            return Some(
                installed
                    .disabled_bundled()
                    .and_then(|entry| entry.enable_action(cluster_id))
                    .map_or(TargetIntent::AlreadyInstalled, TargetIntent::Enable),
            );
        }
        if version.is_some_and(|version| installed.is_version(version)) {
            return Some(TargetIntent::AlreadyInstalled);
        }
    }
    version.map(|version| TargetIntent::Install(version.to_string()))
}

impl Component for SelectedInstall {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster.id;
        let provider = self.provider;
        let kind = self.content_type;
        let project_id = self.install.project_id.clone();
        let dispatch = use_dispatch();
        let enable = use_cluster_mutation();
        let mut pending_world = use_state(|| None::<(String, IncompatibleMode)>);
        let mut pending_conflict = use_state(|| None::<PendingBundledInstall>);
        let mut followup = use_state(|| false);
        let mut started = use_state(|| false);
        let latest = self.install.version.is_none();
        let versions = use_package_versions_when(
            latest,
            provider,
            project_id.clone(),
            Some(self.cluster.mc_version.clone()),
            (kind == ContentType::Mod).then_some(self.cluster.mc_loader),
            0,
            VERSIONS_PAGE_SIZE,
        );
        let result = self
            .install
            .version
            .as_ref()
            .map(|(id, _)| Ok(id.clone()))
            .or_else(|| {
                if let Some(error) = query_error(&versions) {
                    return Some(Err(error));
                }
                settled_or_loading(&versions).map(|versions| {
                    preferred_version(&versions.items, kind)
                        .map(|version| version.version_id.clone())
                        .ok_or_else(|| t("oneclient.browser.package.versions.none").to_string())
                })
            });

        let content = use_cluster_content(cluster_id, kind);
        let bundles = use_bundles_with_status(cluster_id);
        let overrides = use_bundle_overrides(cluster_id);
        let datapack = kind == ContentType::DataPack;
        let ownership_error = (!datapack)
            .then(|| {
                query_error(&content)
                    .or_else(|| query_error(&bundles))
                    .or_else(|| query_error(&overrides))
            })
            .flatten();
        let ownership_ready = datapack
            || (!query_is_busy(&content)
                && !query_is_busy(&bundles)
                && !query_is_busy(&overrides)
                && ownership_error.is_none());
        let installed = if ownership_ready && !datapack {
            installed_map(
                cluster_content_items(&content),
                &bundles_with_status_items(&bundles),
                &bundle_overrides_map(&overrides),
            )
            .remove(&(provider, project_id.clone()))
        } else {
            None
        };
        let intent = ownership_ready
            .then(|| {
                target_intent(
                    &self.install,
                    provider,
                    cluster_id,
                    installed.as_ref(),
                    result
                        .as_ref()
                        .and_then(|value| value.as_ref().ok())
                        .map(String::as_str),
                )
            })
            .flatten();
        let on_close = self.on_close.clone();
        let install_close = on_close.clone();
        let install_project = project_id.clone();
        let instance_name = self.cluster.name.clone();
        use_side_effect_with_deps(&intent, move |intent| {
            let Some(intent) = intent else { return };
            if *started.peek() || matches!(intent, TargetIntent::Enable(_)) {
                return;
            }
            started.set(true);
            match intent {
                TargetIntent::OtherVersionEnabled => {
                    dispatch
                        .notify(t("oneclient.browser.install.other_version_enabled"))
                        .body(tf(
                            "oneclient.browser.install.other_version_enabled.body",
                            &[&instance_name],
                        ))
                        .info()
                        .toast_only()
                        .send();
                    install_close.call(());
                }
                TargetIntent::AlreadyInstalled => {
                    dispatch
                        .notify(t("oneclient.browser.common.source.installed"))
                        .body(tf(
                            "oneclient.browser.install.already_installed",
                            &[&instance_name],
                        ))
                        .info()
                        .toast_only()
                        .send();
                    install_close.call(());
                }
                TargetIntent::BundleConflict(warning) => {
                    pending_conflict.set(Some(warning.clone()));
                    followup.set(true);
                }
                TargetIntent::Install(version) if datapack => {
                    pending_world.set(Some((version.clone(), IncompatibleMode::Ask)));
                    followup.set(true);
                }
                TargetIntent::Install(version) => {
                    dispatch.install_package(
                        cluster_id,
                        provider,
                        install_project.clone(),
                        version.clone(),
                        None,
                        IncompatibleMode::Ask,
                    );
                    install_close.call(());
                }
                TargetIntent::Enable(_) => {}
            }
        });
        let done =
            followup() && pending_world.read().is_none() && pending_conflict.read().is_none();
        let enabled = mutation_ok(&enable).is_some();
        let close_followup = on_close.clone();
        use_side_effect_with_deps(&(done, enabled), move |&(done, enabled)| {
            if done || enabled {
                close_followup.call(());
            }
        });

        if followup() {
            return rect()
                .maybe_child(
                    pending_world
                        .read()
                        .is_some()
                        .then_some(WorldInstallPrompt {
                            cluster_id,
                            provider,
                            project_id,
                            pending: pending_world,
                        }),
                )
                .child(BundledInstallWarning {
                    pending: pending_conflict,
                })
                .into_element();
        }

        let error = ownership_error.or_else(|| {
            intent
                .is_none()
                .then(|| result.and_then(Result::err))
                .flatten()
        });
        let running = mutation_is_running(&enable);
        let on_back = self.on_back.clone();
        let (body, control, primary) = match intent {
            Some(TargetIntent::Enable(action)) => {
                let button = Button::new()
                    .primary()
                    .enabled(!running)
                    .text(if running {
                        t("oneclient.browser.common.enabling_ellipsis")
                    } else {
                        t("oneclient.browser.common.enable")
                    })
                    .on_press(move |_| {
                        started.set(true);
                        enable.mutate(action.clone());
                    });
                (
                    mutation_error(&enable).unwrap_or_else(|| {
                        tf(
                            "oneclient.browser.install.enable_bundled",
                            &[&self.install.project_name, &self.cluster.name],
                        )
                    }),
                    None,
                    button,
                )
            }
            _ if error.is_some() => {
                let button = Button::new()
                    .primary()
                    .text(t("oneclient.common.retry"))
                    .on_press(move |_| {
                        if latest {
                            versions.invalidate();
                        }
                        if query_error(&content).is_some() {
                            content.invalidate();
                        }
                        if query_error(&bundles).is_some() {
                            bundles.invalidate();
                        }
                        if query_error(&overrides).is_some() {
                            overrides.invalidate();
                        }
                    });
                let change = Button::new()
                    .ghost()
                    .text(t("oneclient.browser.index.select_version"))
                    .on_press(move |_| on_back.call(()));
                (
                    error.unwrap_or_default(),
                    Some(change.into_element()),
                    button,
                )
            }
            _ => (
                self.cluster.name.clone(),
                Some(centered_spinner(t("oneclient.browser.install.finding")).into_element()),
                Button::new()
                    .primary()
                    .enabled(false)
                    .text(t("oneclient.common.install")),
            ),
        };

        dialog(
            t("oneclient.common.install").to_string(),
            body,
            control,
            move || on_close.call(()),
            [
                Button::new()
                    .secondary()
                    .text(t("oneclient.common.cancel"))
                    .on_press({
                        let close = self.on_close.clone();
                        move |_| close.call(())
                    })
                    .into_element(),
                primary.into_element(),
            ],
        )
        .into_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::app::browser::{BundlePin, InstalledVersion};

    fn request(version: Option<&str>) -> InstallRequest {
        InstallRequest {
            project_id: "sodium".into(),
            project_name: "Sodium".into(),
            version: version.map(|id| (id.into(), format!("Version {id}"))),
        }
    }

    fn bundled(enabled: bool) -> Installed {
        Installed {
            source: InstallSource::Bundled,
            bundled_version: Some("1".into()),
            versions: vec![InstalledVersion {
                version_id: "1".into(),
                display_version: Some("One".into()),
                hash: enabled.then(|| "one".into()),
                enabled,
                source: InstallSource::Bundled,
                bundle: Some(BundlePin {
                    bundle_name: "test".into(),
                    package_id: "sodium".into(),
                    manifest_default: enabled,
                }),
            }],
            has_enabled_unversioned: false,
        }
    }

    #[test]
    fn explicit_alternative_keeps_its_version_in_the_bundle_warning() {
        let target = target_intent(
            &request(Some("2")),
            ProviderId::Modrinth,
            3,
            Some(&bundled(true)),
            Some("2"),
        );
        let Some(TargetIntent::BundleConflict(warning)) = target else {
            panic!("expected a bundle warning");
        };
        assert_eq!(warning.version_id, "2");
        assert_eq!(warning.version_label, "Version 2");
        assert_eq!(warning.bundled_version.as_deref(), Some("One"));
    }

    #[test]
    fn disabled_bundled_version_can_be_enabled_for_latest_or_exact_requests() {
        for version in [None, Some("1")] {
            assert!(matches!(
                target_intent(
                    &request(version),
                    ProviderId::Modrinth,
                    3,
                    Some(&bundled(false)),
                    version
                ),
                Some(TargetIntent::Enable(
                    ClusterAction::SetBundlePackageEnabled {
                        cluster_id: 3,
                        enabled: true,
                        ..
                    }
                ))
            ));
        }
    }

    #[test]
    fn an_exact_disabled_bundle_pin_does_not_enable_a_second_copy() {
        let mut installed = bundled(false);
        installed.versions.push(InstalledVersion {
            version_id: "2".into(),
            display_version: None,
            hash: Some("two".into()),
            enabled: true,
            source: InstallSource::Manual,
            bundle: None,
        });

        assert!(matches!(
            target_intent(
                &request(Some("1")),
                ProviderId::Modrinth,
                3,
                Some(&installed),
                Some("1")
            ),
            Some(TargetIntent::OtherVersionEnabled)
        ));
    }
}
