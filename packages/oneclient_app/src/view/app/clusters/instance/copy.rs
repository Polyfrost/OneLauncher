use oneclient_cluster::naming::validate_instance_name;

use super::data::Picks;
use super::model::{
    FolderChoice, ImportMode, ImportSource, ModpackOrigin, Step, TypeChoice, Wizard,
};
use crate::i18n::{t, tf};

pub fn heading(picks: &Picks) -> (&'static str, String) {
    match picks.step {
        Step::Type => (
            t("oneclient.clusters.instance.copy.type.title"),
            t("oneclient.clusters.instance.copy.type.subtitle").to_string(),
        ),
        Step::Folder => (
            t("oneclient.clusters.instance.copy.folder.title"),
            t("oneclient.clusters.instance.copy.folder.subtitle").to_string(),
        ),
        Step::Loader => (
            t("oneclient.clusters.instance.copy.loader.title"),
            match &picks.versions.chosen {
                Some(version) => tf(
                    "oneclient.clusters.instance.copy.loader.subtitle_with_version",
                    &[&version],
                ),
                None => t("oneclient.clusters.instance.copy.loader.subtitle").to_string(),
            },
        ),
        Step::Version => (
            t("oneclient.clusters.instance.copy.version.title"),
            match picks.choice {
                TypeChoice::OneClient => {
                    t("oneclient.clusters.instance.copy.version.subtitle.oneclient").to_string()
                }
                TypeChoice::Scratch | TypeChoice::Modpack | TypeChoice::Import => {
                    t("oneclient.clusters.instance.copy.version.subtitle.any").to_string()
                }
            },
        ),
        Step::Bundles => (
            t("oneclient.clusters.instance.copy.bundles.title"),
            t("oneclient.clusters.instance.copy.bundles.subtitle").to_string(),
        ),
        Step::Customize => (
            t("oneclient.clusters.instance.copy.customize.title"),
            t("oneclient.clusters.instance.copy.customize.subtitle").to_string(),
        ),
        Step::Modpack => (
            t("oneclient.clusters.instance.copy.modpack.title"),
            t("oneclient.clusters.instance.copy.modpack.subtitle").to_string(),
        ),
        Step::ImportSource => (
            t("oneclient.clusters.instance.copy.import_source.title"),
            t("oneclient.clusters.instance.copy.import_source.subtitle").to_string(),
        ),
        Step::ImportMode => (
            t("oneclient.clusters.instance.copy.import_mode.title"),
            t("oneclient.clusters.instance.copy.import_mode.subtitle").to_string(),
        ),
        Step::Import => (
            t("oneclient.clusters.instance.copy.import.title"),
            t("oneclient.clusters.instance.copy.import.subtitle").to_string(),
        ),
        Step::ImportReview => (
            t("oneclient.clusters.instance.copy.import_review.title"),
            t("oneclient.clusters.instance.copy.import_review.subtitle").to_string(),
        ),
    }
}

pub fn footer_note(picks: &Picks) -> String {
    match picks.step {
        Step::Type => match picks.choice {
            TypeChoice::OneClient => {
                t("oneclient.clusters.instance.copy.note.type.oneclient").to_string()
            }
            TypeChoice::Scratch => {
                t("oneclient.clusters.instance.copy.note.type.scratch").to_string()
            }
            TypeChoice::Modpack => {
                t("oneclient.clusters.instance.copy.note.type.modpack").to_string()
            }
            TypeChoice::Import => {
                t("oneclient.clusters.instance.copy.note.type.import").to_string()
            }
        },
        Step::Folder => match picks.folder {
            FolderChoice::Shared => {
                t("oneclient.clusters.instance.copy.note.folder.shared").to_string()
            }
            FolderChoice::Dedicated => {
                t("oneclient.clusters.instance.copy.note.folder.dedicated").to_string()
            }
        },
        Step::Loader => match (&picks.versions.chosen, picks.loader.chosen) {
            (Some(version), None) => {
                tf("oneclient.clusters.instance.copy.note.loader", &[&version])
            }
            _ => picks.loader_label(),
        },
        Step::Version => match &picks.versions.chosen {
            Some(version) => tf(
                "oneclient.clusters.instance.copy.note.version.chosen",
                &[&version],
            ),
            None => t("oneclient.clusters.instance.copy.note.version.none").to_string(),
        },
        Step::Bundles => {
            let taken = picks.bundles.taken_count();
            let total = picks.bundles.archives.len();
            if taken == 0 {
                t("oneclient.clusters.instance.copy.note.bundles.none").to_string()
            } else {
                tf(
                    "oneclient.clusters.instance.copy.note.bundles.some",
                    &[&taken, &total],
                )
            }
        }
        Step::Customize => t("oneclient.clusters.instance.copy.note.customize").to_string(),
        Step::Modpack => match picks.modpack_origin {
            ModpackOrigin::Browse => {
                t("oneclient.clusters.instance.copy.note.modpack.browse").to_string()
            }
            ModpackOrigin::File => {
                t("oneclient.clusters.instance.copy.note.modpack.file").to_string()
            }
        },
        Step::ImportSource => match picks.import_source {
            ImportSource::File => {
                t("oneclient.clusters.instance.copy.note.import_source.file").to_string()
            }
            ImportSource::Launcher => {
                t("oneclient.clusters.instance.copy.note.import_source.launcher").to_string()
            }
        },
        Step::ImportMode => match picks.import_mode {
            ImportMode::AsIs => {
                t("oneclient.clusters.instance.copy.note.import_mode.as_is").to_string()
            }
            ImportMode::Improve => {
                t("oneclient.clusters.instance.copy.note.import_mode.improve").to_string()
            }
        },
        Step::Import => match picks.import_count {
            0 => t("oneclient.clusters.instance.copy.note.import.none").to_string(),
            1 => t("oneclient.clusters.instance.copy.note.import.one").to_string(),
            n => tf("oneclient.clusters.instance.copy.note.import.other", &[&n]),
        },
        Step::ImportReview => match (&picks.import_screening, picks.flagged_count()) {
            (None, _) if picks.import_screening_error.is_some() => {
                t("oneclient.clusters.instance.copy.note.import_review.unchecked").to_string()
            }
            (None, _) => {
                t("oneclient.clusters.instance.copy.note.import_review.checking").to_string()
            }
            (Some(_), 0) => {
                t("oneclient.clusters.instance.copy.note.import_review.none").to_string()
            }
            (Some(_), 1) => {
                t("oneclient.clusters.instance.copy.note.import_review.one").to_string()
            }
            (Some(_), n) => tf(
                "oneclient.clusters.instance.copy.note.import_review.other",
                &[&n],
            ),
        },
    }
}

pub fn step_value(wizard: Wizard, picks: &Picks, step: Step) -> String {
    match step {
        Step::Type => match picks.choice {
            TypeChoice::OneClient => "OneClient".to_string(),
            TypeChoice::Scratch => t("oneclient.clusters.instance.copy.value.scratch").to_string(),
            TypeChoice::Modpack => t("oneclient.clusters.instance.copy.value.modpack").to_string(),
            TypeChoice::Import => t("oneclient.clusters.instance.copy.value.import").to_string(),
        },
        Step::Folder => match picks.folder {
            FolderChoice::Shared => t("oneclient.clusters.instance.copy.value.shared").to_string(),
            FolderChoice::Dedicated => {
                t("oneclient.clusters.instance.copy.value.dedicated").to_string()
            }
        },
        Step::Loader => picks.loader_label(),
        Step::Version => {
            picks.versions.chosen.clone().unwrap_or_else(|| {
                t("oneclient.clusters.instance.copy.value.not_chosen").to_string()
            })
        }
        Step::Bundles => {
            let taken = picks.bundles.taken_count();
            if taken == 0 {
                t("oneclient.clusters.instance.copy.value.none").to_string()
            } else {
                tf("oneclient.clusters.instance.copy.value.selected", &[&taken])
            }
        }
        Step::Modpack => match picks.modpack_origin {
            ModpackOrigin::Browse => t("oneclient.clusters.instance.copy.value.browse").to_string(),
            ModpackOrigin::File => t("oneclient.clusters.instance.copy.value.file").to_string(),
        },
        Step::ImportSource => match picks.import_source {
            ImportSource::File => t("oneclient.clusters.instance.copy.value.file").to_string(),
            ImportSource::Launcher => {
                t("oneclient.clusters.instance.copy.value.launcher").to_string()
            }
        },
        Step::ImportMode => match picks.import_mode {
            ImportMode::AsIs => t("oneclient.clusters.instance.copy.value.as_is").to_string(),
            ImportMode::Improve => t("oneclient.clusters.instance.copy.value.improved").to_string(),
        },
        Step::Import => match picks.import_count {
            0 => t("oneclient.clusters.instance.copy.value.none_selected").to_string(),
            n => tf("oneclient.clusters.instance.copy.value.selected", &[&n]),
        },
        Step::ImportReview => match (&picks.import_screening, picks.flagged_count()) {
            (None, _) if picks.import_screening_error.is_some() => {
                t("oneclient.clusters.instance.copy.value.not_checked").to_string()
            }
            (None, _) => t("oneclient.clusters.instance.copy.value.checking").to_string(),
            (Some(_), 0) => t("oneclient.clusters.instance.copy.value.nothing_flagged").to_string(),
            (Some(_), 1) => t("oneclient.clusters.instance.copy.value.flagged.one").to_string(),
            (Some(_), n) => tf(
                "oneclient.clusters.instance.copy.value.flagged.other",
                &[&n],
            ),
        },
        Step::Customize => match wizard.details.tags.read().len() {
            0 => t("oneclient.clusters.instance.copy.value.optional").to_string(),
            1 => t("oneclient.clusters.instance.copy.value.tags.one").to_string(),
            many => tf(
                "oneclient.clusters.instance.copy.value.tags.other",
                &[&many],
            ),
        },
    }
}

pub fn is_ready(picks: &Picks) -> bool {
    match picks.step {
        Step::Type | Step::Folder | Step::Bundles | Step::Modpack => true,
        Step::Loader => picks.loader.chosen.is_some(),
        Step::Version => {
            picks.versions.chosen.is_some()
                && (picks.choice == TypeChoice::Scratch || picks.loader.chosen.is_some())
        }
        Step::Customize => validate_instance_name(&picks.name).is_ok(),
        Step::ImportSource | Step::ImportMode => true,
        Step::Import => picks.import_count > 0,
        Step::ImportReview => {
            picks.import_count > 0
                && (picks.import_screening.is_some() || picks.import_screening_error.is_some())
        }
    }
}
