use oneclient_cluster::naming::validate_instance_name;

use super::data::Picks;
use super::model::{ImportMode, ModpackOrigin, Step, TypeChoice, Wizard};

pub fn heading(picks: &Picks) -> (&'static str, String) {
    match picks.step {
        Step::Type => (
            "Choose a type",
            "Four ways to start. Packages can be added to any of them later.".to_string(),
        ),
        Step::Loader => (
            "Choose a mod loader",
            match &picks.versions.chosen {
                Some(version) => format!(
                    "Loaders are what packages install into. Only the ones with a build for {version} can be picked."
                ),
                None => "Loaders are what packages install into.".to_string(),
            },
        ),
        Step::Version => (
            "Choose a version",
            match picks.choice {
                TypeChoice::OneClient => {
                    "Only versions OneClient ships a build for are listed.".to_string()
                }
                TypeChoice::Scratch | TypeChoice::Modpack | TypeChoice::Import => {
                    "Every Minecraft version. Switch the release type to reach snapshots, betas and alphas."
                        .to_string()
                }
            },
        ),
        Step::Bundles => (
            "Add bundles",
            "Curated sets of packages, installed and configured together. Take as many as you like."
                .to_string(),
        ),
        Step::Customize => (
            "Name the instance",
            "Everything here can be changed afterwards.".to_string(),
        ),
        Step::Modpack => (
            "Add a modpack",
            "The pack decides the version, the loader and the mods. The instance is named after it."
                .to_string(),
        ),
        Step::ImportMode => (
            "How to import",
            "Bring instances over untouched, or let OneClient check their mods first.".to_string(),
        ),
        Step::Import => (
            "Import instances",
            "Each one becomes its own instance with its mods, worlds and settings. The originals stay where they are."
                .to_string(),
        ),
        Step::ImportReview => (
            "Review mods",
            "These mods are known to cause problems. Choose what happens to each one before importing."
                .to_string(),
        ),
    }
}

pub fn footer_note(picks: &Picks) -> String {
    match picks.step {
        Step::Type => match picks.choice {
            TypeChoice::OneClient => {
                "Shares its game folder, worlds and packs with your other OneClient instances."
                    .to_string()
            }
            TypeChoice::Scratch => "Keeps its own game folder, worlds and packs.".to_string(),
            TypeChoice::Modpack => {
                "Keeps its own game folder, worlds and packs, set up the way the pack author made it."
                    .to_string()
            }
            TypeChoice::Import => {
                "Copies instances from Prism Launcher, MultiMC, PolyMC or the Modrinth App."
                    .to_string()
            }
        },
        Step::Loader => match (&picks.versions.chosen, picks.loader.chosen) {
            (Some(version), None) => format!("Pick a loader with a build for {version}."),
            _ => picks.loader_label(),
        },
        Step::Version => match &picks.versions.chosen {
            Some(version) => format!("{version} downloads the first time you launch it."),
            None => "Pick a version to continue.".to_string(),
        },
        Step::Bundles => {
            let taken = picks.bundles.taken_count();
            let total = picks.bundles.archives.len();
            if taken == 0 {
                "Nothing selected. OneConfig is installed either way.".to_string()
            } else {
                format!("{taken} of {total} selected. Bundles can be changed later.")
            }
        }
        Step::Customize => "The instance is created locally. Nothing is uploaded.".to_string(),
        Step::Modpack => match picks.modpack_origin {
            ModpackOrigin::Browse => {
                "Installing a modpack from the browser creates its instance.".to_string()
            }
            ModpackOrigin::File => {
                "Takes .mrpack files from Modrinth and .zip files from CurseForge.".to_string()
            }
        },
        Step::ImportMode => match picks.import_mode {
            ImportMode::AsIs => "Mods, worlds and settings come over unchanged.".to_string(),
            ImportMode::Improve => {
                "Problem mods can be swapped for a better alternative or left out.".to_string()
            }
        },
        Step::Import => match picks.import_count {
            0 => "Pick the instances to bring over.".to_string(),
            1 => "1 instance selected. Accounts are not imported; sign in here.".to_string(),
            n => format!("{n} instances selected. Accounts are not imported; sign in here."),
        },
        Step::ImportReview => match (&picks.import_screening, picks.flagged_count()) {
            (None, _) if picks.import_screening_error.is_some() => {
                "Mods were not checked. Importing brings them over unchanged.".to_string()
            }
            (None, _) => "Checking mods...".to_string(),
            (Some(_), 0) => "Nothing to change. Accounts are not imported; sign in here.".to_string(),
            (Some(_), 1) => "1 mod flagged. Accounts are not imported; sign in here.".to_string(),
            (Some(_), n) => format!("{n} mods flagged. Accounts are not imported; sign in here."),
        },
    }
}

pub fn step_value(wizard: Wizard, picks: &Picks, step: Step) -> String {
    match step {
        Step::Type => match picks.choice {
            TypeChoice::OneClient => "OneClient".to_string(),
            TypeChoice::Scratch => "From scratch".to_string(),
            TypeChoice::Modpack => "Modpack".to_string(),
            TypeChoice::Import => "Import".to_string(),
        },
        Step::Loader => picks.loader_label(),
        Step::Version => picks
            .versions
            .chosen
            .clone()
            .unwrap_or_else(|| "Not chosen".to_string()),
        Step::Bundles => {
            let taken = picks.bundles.taken_count();
            if taken == 0 {
                "None".to_string()
            } else {
                format!("{taken} selected")
            }
        }
        Step::Modpack => match picks.modpack_origin {
            ModpackOrigin::Browse => "Browse".to_string(),
            ModpackOrigin::File => "From a file".to_string(),
        },
        Step::ImportMode => match picks.import_mode {
            ImportMode::AsIs => "As it is".to_string(),
            ImportMode::Improve => "Improved".to_string(),
        },
        Step::Import => match picks.import_count {
            0 => "None selected".to_string(),
            n => format!("{n} selected"),
        },
        Step::ImportReview => match (&picks.import_screening, picks.flagged_count()) {
            (None, _) if picks.import_screening_error.is_some() => "Not checked".to_string(),
            (None, _) => "Checking".to_string(),
            (Some(_), 0) => "Nothing flagged".to_string(),
            (Some(_), 1) => "1 flagged".to_string(),
            (Some(_), n) => format!("{n} flagged"),
        },
        Step::Customize => match wizard.details.tags.read().len() {
            0 => "Optional".to_string(),
            1 => "1 tag".to_string(),
            many => format!("{many} tags"),
        },
    }
}

pub fn is_ready(picks: &Picks) -> bool {
    match picks.step {
        Step::Type | Step::Bundles | Step::Modpack => true,
        Step::Loader => picks.loader.chosen.is_some(),
        Step::Version => {
            picks.versions.chosen.is_some()
                && (picks.choice == TypeChoice::Scratch || picks.loader.chosen.is_some())
        }
        Step::Customize => validate_instance_name(&picks.name).is_ok(),
        Step::ImportMode => true,
        Step::Import => picks.import_count > 0,
        Step::ImportReview => {
            picks.import_count > 0
                && (picks.import_screening.is_some() || picks.import_screening_error.is_some())
        }
    }
}
