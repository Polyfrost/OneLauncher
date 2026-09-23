use super::data::Picks;
use super::model::{Step, TypeChoice, Wizard};

pub fn heading(picks: &Picks) -> (&'static str, String) {
    match picks.step {
        Step::Type => (
            "Choose a type",
            "Two ways to start. Packages can be added to either of them later.".to_string(),
        ),
        Step::Loader => (
            "Choose a mod loader",
            "Loaders are what packages install into. Pick one, then the Minecraft version it runs on."
                .to_string(),
        ),
        Step::Version => (
            "Choose a version",
            match picks.choice {
                Some(TypeChoice::OneClient) => {
                    "Only versions OneClient ships a build for are listed.".to_string()
                }
                _ => format!(
                    "Versions {} can run. Switch the release type to reach snapshots, betas and alphas.",
                    picks.loader_label()
                ),
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
    }
}

pub fn footer_note(picks: &Picks) -> String {
    match picks.step {
        Step::Type => match picks.choice {
            Some(TypeChoice::OneClient) => {
                "Shares its game folder, worlds and packs with your other OneClient instances."
                    .to_string()
            }
            Some(TypeChoice::Scratch) => "Keeps its own game folder, worlds and packs.".to_string(),
            None => "Pick a type to continue.".to_string(),
        },
        Step::Loader => picks.loader_label(),
        Step::Version => match (&picks.versions.chosen, picks.loader.chosen) {
            (Some(version), Some(_)) => {
                format!("{version} downloads the first time you launch it.")
            }
            (Some(version), None) => {
                format!("{} has no build for {version}.", picks.loader_label())
            }
            (None, _) => "Pick a version to continue.".to_string(),
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
    }
}

pub fn step_value(wizard: Wizard, picks: &Picks, step: Step) -> String {
    match step {
        Step::Type => match picks.choice {
            Some(TypeChoice::OneClient) => "OneClient".to_string(),
            Some(TypeChoice::Scratch) => "From scratch".to_string(),
            None => String::new(),
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
        Step::Customize => match wizard.details.tags.read().len() {
            0 => "Optional".to_string(),
            1 => "1 tag".to_string(),
            many => format!("{many} tags"),
        },
    }
}

pub fn is_ready(picks: &Picks) -> bool {
    match picks.step {
        Step::Type => picks.choice.is_some(),
        Step::Loader | Step::Bundles => true,
        Step::Version => picks.versions.chosen.is_some() && picks.loader.chosen.is_some(),
        Step::Customize => !picks.name.trim().is_empty(),
    }
}
