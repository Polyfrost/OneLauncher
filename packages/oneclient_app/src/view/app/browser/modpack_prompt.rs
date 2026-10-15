use std::collections::HashSet;

use freya::prelude::*;
use oneclient_common::version::parse_mc_version;
use oneclient_content::packages::ProviderId;
use oneclient_content::packages::types::{ReleaseType, VersionSummary};
use oneclient_core::clusters::ModpackSource;

use crate::components::{Button, Icon, IconType, ScrollArea};
use crate::hooks::use_dispatch;
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::view::app::cluster::dialog;

const ROW_H: f32 = 44.;
const ROW_GAP: f32 = 6.;
const VISIBLE_ROWS: usize = 5;

#[derive(Clone, PartialEq)]
pub(crate) struct MinecraftChoice {
    mc: String,
    version_id: String,
    detail: String,
}

pub(crate) fn minecraft_choices(versions: &[VersionSummary]) -> Vec<MinecraftChoice> {
    let mut seen = HashSet::new();
    let mut order: Vec<_> = versions
        .iter()
        .flat_map(|v| &v.game_versions)
        .filter(|mc| seen.insert(mc.as_str()))
        .filter(|mc| mc.bytes().all(|b| b.is_ascii_digit() || b == b'.'))
        .filter_map(|mc| parse_mc_version(mc).map(|p| (mc.clone(), (p.major, p.minor, p.patch))))
        .collect();
    order.sort_by(|a, b| b.1.cmp(&a.1));

    let mut choices: Vec<MinecraftChoice> = Vec::new();
    for (mc, _) in order {
        let for_mc: Vec<_> = versions
            .iter()
            .filter(|v| v.game_versions.contains(&mc))
            .collect();
        let mut loader_sets: Vec<&Vec<_>> = Vec::new();
        for v in &for_mc {
            if !loader_sets.contains(&&v.loaders) {
                loader_sets.push(&v.loaders);
            }
        }
        for loaders in loader_sets {
            let same = || for_mc.iter().filter(|v| &v.loaders == loaders);
            let Some(pick) = same()
                .find(|v| matches!(v.release_type, ReleaseType::Release))
                .or_else(|| same().next())
            else {
                continue;
            };
            push_choice(&mut choices, &mc, pick);
        }
    }
    choices
}

fn push_choice(choices: &mut Vec<MinecraftChoice>, mc: &str, pick: &VersionSummary) {
    if let Some(existing) = choices.iter_mut().find(|c| c.version_id == pick.version_id) {
        existing.mc.push_str(" / ");
        existing.mc.push_str(mc);
        return;
    }
    let mut detail = vec![pick.version_number.clone()];
    detail.extend(pick.loaders.iter().map(|l| l.to_string()));
    match pick.release_type {
        ReleaseType::Release => {}
        ReleaseType::Beta => detail.push("Beta".to_string()),
        ReleaseType::Alpha => detail.push("Alpha".to_string()),
    }
    detail.push(pick.published.format("%Y-%m-%d").to_string());
    choices.push(MinecraftChoice {
        mc: mc.to_string(),
        version_id: pick.version_id.clone(),
        detail: detail.join("  ·  "),
    });
}

#[derive(PartialEq)]
pub(crate) struct ModpackVersionPrompt {
    pub provider: ProviderId,
    pub project_id: String,
    pub choices: Vec<MinecraftChoice>,
    pub open: State<bool>,
}

impl Component for ModpackVersionPrompt {
    fn render(&self) -> impl IntoElement {
        let provider = self.provider;
        let project_id = self.project_id.clone();
        let mut open = self.open;
        let dispatch = use_dispatch();
        let mut selected = use_state(|| 0usize);
        let current = (*selected.read()).min(self.choices.len().saturating_sub(1));

        let rows: Vec<Element> = self
            .choices
            .iter()
            .enumerate()
            .map(|(i, choice)| {
                choice_row(choice, i == current, move || selected.set(i)).into_element()
            })
            .collect();
        let shown = self.choices.len().min(VISIBLE_ROWS) as f32;
        let list_h = shown * ROW_H + (shown - 1.).max(0.) * ROW_GAP;
        let control = ScrollArea::new()
            .width(Size::fill())
            .height(Size::px(list_h))
            .spacing(ROW_GAP)
            .children(rows)
            .into_element();

        let version_id = self.choices.get(current).map(|c| c.version_id.clone());
        let install = move |_| {
            let Some(version_id) = version_id.clone() else {
                return;
            };
            dispatch.install_modpack(ModpackSource::Provider {
                provider,
                project_id: project_id.clone(),
                version_id,
            });
            open.set(false);
        };

        dialog(
            "Which Minecraft version?".to_string(),
            "This modpack is made for several Minecraft versions. Pick the one to install."
                .to_string(),
            Some(control),
            move || open.set(false),
            [
                Button::new()
                    .secondary()
                    .on_press(move |_| open.set(false))
                    .text("Cancel")
                    .into_element(),
                Button::new()
                    .primary()
                    .on_press(install)
                    .child(Icon::new(IconType::Download01).size(14.))
                    .text("Install")
                    .into_element(),
            ],
        )
    }
}

fn choice_row(
    choice: &MinecraftChoice,
    active: bool,
    mut on_select: impl FnMut() + 'static,
) -> impl IntoElement {
    let ring = if active {
        colors::brand()
    } else {
        colors::component_border()
    };
    let radio = rect()
        .width(Size::px(16.))
        .height(Size::px(16.))
        .center()
        .corner_radius(CornerRadius::new_all(8.))
        .border(border_all_color(1.5, ring))
        .maybe_child(active.then(|| {
            rect()
                .width(Size::px(8.))
                .height(Size::px(8.))
                .corner_radius(CornerRadius::new_all(4.))
                .background(colors::brand())
        }));

    rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::px(ROW_H))
        .cross_align(Alignment::Center)
        .content(Content::Flex)
        .spacing(10.)
        .padding(Gaps::new_symmetric(0., 12.))
        .corner_radius(CornerRadius::new_all(8.))
        .background(colors::component_bg())
        .border(border_all_color(1., ring))
        .on_press(move |_| on_select())
        .child(radio)
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.))
                .spacing(2.)
                .child(
                    label()
                        .text(format!("Minecraft {}", choice.mc))
                        .font_size(13.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .max_lines(1)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(choice.detail.clone())
                        .font_size(11.)
                        .max_lines(1)
                        .color(colors::fg_secondary()),
                ),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(id: &str, mc: &[&str], release_type: ReleaseType) -> VersionSummary {
        VersionSummary {
            version_id: id.to_string(),
            game_versions: mc.iter().map(|s| s.to_string()).collect(),
            project_id: String::new(),
            name: String::new(),
            version_number: String::new(),
            published: Default::default(),
            release_type,
            loaders: Vec::new(),
            downloads: 0,
            file_size: 0,
            dependencies: Vec::new(),
        }
    }

    #[test]
    fn one_choice_per_minecraft_version_preferring_releases() {
        let versions = [
            version("b3", &["26.3"], ReleaseType::Beta),
            version("r3", &["26.3"], ReleaseType::Release),
            version("b2", &["26.2"], ReleaseType::Beta),
            version(
                "r1",
                &["1.21.11", "Fabric", "26.1", "1.21.5-rc1", "Client"],
                ReleaseType::Release,
            ),
        ];
        let picks: Vec<(String, String)> = minecraft_choices(&versions)
            .into_iter()
            .map(|c| (c.mc, c.version_id))
            .collect();
        let expect = [("26.3", "r3"), ("26.2", "b2"), ("26.1 / 1.21.11", "r1")];
        assert_eq!(
            picks,
            expect.map(|(a, b)| (a.to_string(), b.to_string())).to_vec()
        );
    }

    #[test]
    fn one_choice_per_loader_on_the_same_minecraft_version() {
        use oneclient_common::domain::GameLoader;
        let with = |id, loader, release_type| VersionSummary {
            loaders: vec![loader],
            ..version(id, &["1.21.1"], release_type)
        };
        let versions = [
            with("fabric-new", GameLoader::Fabric, ReleaseType::Release),
            with("forge-beta", GameLoader::Forge, ReleaseType::Beta),
            with("fabric-old", GameLoader::Fabric, ReleaseType::Release),
            with("forge-rel", GameLoader::Forge, ReleaseType::Release),
        ];
        let ids: Vec<String> = minecraft_choices(&versions)
            .into_iter()
            .map(|c| c.version_id)
            .collect();
        assert_eq!(ids, ["fabric-new", "forge-rel"]);
    }
}
