use std::collections::HashMap;
use std::path::Path;

use freya::prelude::*;
use oneclient_content::packages::ResolvedAlternative;
use oneclient_core::{FlaggedImportMod, ScreenedInstance};

use super::cards::{SelectCard, marker};
use super::data::Picks;
use super::model::{DecisionKey, ModDecision, Wizard, decision_for};
use crate::components::{
    Button, Icon, IconType, ScrollArea, centered_spinner, explanation_panel, remote_icon,
};
use crate::hooks::{retry_import_screening, use_cached_image};
use crate::theme::colors;
use crate::ui::border_all_color;

const OPTION_H: f32 = 48.;
const OPTION_ICON: f32 = 28.;

type Decisions = State<HashMap<DecisionKey, ModDecision>>;

pub fn review_step(wizard: Wizard, picks: &Picks) -> Element {
    if picks.import_screening.is_none()
        && let Some(error) = &picks.import_screening_error
    {
        return not_checked(wizard, error.clone());
    }
    let Some(screened) = &picks.import_screening else {
        return rect()
            .width(Size::fill())
            .height(Size::fill())
            .child(centered_spinner("Checking mods..."))
            .into_element();
    };

    let flagged: Vec<&ScreenedInstance> = screened
        .iter()
        .filter(|screened| !screened.flagged.is_empty())
        .collect();
    if flagged.is_empty() {
        return all_clear();
    }

    ScrollArea::new()
        .width(Size::fill())
        .height(Size::fill())
        .child(rect().vertical().width(Size::fill()).spacing(20.).children(
            flagged.into_iter().map(|screened| {
                instance_section(wizard.import_decisions, &picks.import_decisions, screened)
            }),
        ))
        .into_element()
}

fn not_checked(wizard: Wizard, error: String) -> Element {
    let chosen = wizard.import_chosen;
    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::fill())
        .center()
        .padding(Gaps::new_symmetric(0., 32.))
        .spacing(8.)
        .child(
            Icon::new(IconType::AlertCircle)
                .size(28.)
                .color(colors::code_warn()),
        )
        .child(
            label()
                .text("Couldn't check the mods")
                .font_size(14.)
                .font_weight(FontWeight::MEDIUM)
                .color(colors::fg_primary()),
        )
        .child(
            label()
                .text("OneClient's list of problem mods could not be loaded. Try again, or import without checking.")
                .width(Size::fill())
                .font_size(12.)
                .text_align(TextAlign::Center)
                .color(colors::fg_secondary()),
        )
        .child(
            label()
                .text(error)
                .width(Size::fill())
                .font_size(11.)
                .max_lines(2)
                .text_overflow(TextOverflow::Ellipsis)
                .text_align(TextAlign::Center)
                .color(colors::fg_secondary()),
        )
        .child(
            rect().padding(Gaps::new(8., 0., 0., 0.)).child(
                Button::new()
                    .secondary()
                    .small()
                    .on_press(move |_| {
                        let instances = chosen.read().clone();
                        spawn(async move { retry_import_screening(instances).await });
                    })
                    .child(Icon::new(IconType::RefreshCw01).size(14.))
                    .text("Try again"),
            ),
        )
        .into_element()
}

fn all_clear() -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::fill())
        .center()
        .padding(Gaps::new_symmetric(0., 32.))
        .spacing(8.)
        .child(
            Icon::new(IconType::CheckCircle)
                .size(28.)
                .color(colors::fg_secondary()),
        )
        .child(
            label()
                .text("Nothing to improve")
                .font_size(14.)
                .font_weight(FontWeight::MEDIUM)
                .color(colors::fg_primary()),
        )
        .child(
            label()
                .text("None of the mods in the selected instances are on OneClient's list of problem mods.")
                .width(Size::fill())
                .font_size(12.)
                .text_align(TextAlign::Center)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

fn instance_section(
    state: Decisions,
    decisions: &HashMap<DecisionKey, ModDecision>,
    screened: &ScreenedInstance,
) -> Element {
    let count = match screened.flagged.len() {
        1 => "1 flagged mod".to_string(),
        n => format!("{n} flagged mods"),
    };

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(10.)
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .content(Content::Flex)
                .cross_align(Alignment::Center)
                .spacing(8.)
                .child(
                    label()
                        .width(Size::flex(1.0))
                        .text(screened.name.clone())
                        .font_size(13.)
                        .font_weight(FontWeight::MEDIUM)
                        .max_lines(1)
                        .text_overflow(TextOverflow::Ellipsis)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(count)
                        .font_size(12.)
                        .color(colors::fg_secondary()),
                ),
        )
        .children(screened.flagged.iter().map(|flagged| {
            let current = decision_for(decisions, &screened.game_dir, flagged);
            flagged_card(
                state,
                &screened.game_dir,
                &screened.mc_version,
                flagged,
                current,
            )
        }))
        .into_element()
}

fn flagged_card(
    state: Decisions,
    game_dir: &Path,
    mc_version: &str,
    flagged: &FlaggedImportMod,
    current: ModDecision,
) -> Element {
    let key: DecisionKey = (game_dir.to_path_buf(), flagged.hash.clone());
    let file_line = if flagged.enabled {
        flagged.file_name.clone()
    } else {
        format!("{} · disabled", flagged.file_name)
    };

    let alternatives = flagged.alternatives.iter().map(|alternative| {
        let selected = current
            == ModDecision::Replace {
                provider: alternative.resolved.provider,
                project_id: alternative.resolved.project_id.clone(),
            };
        AlternativeOption {
            decisions: state,
            key: key.clone(),
            alternative: alternative.resolved.clone(),
            already_installed: alternative.already_installed,
            mc_version: mc_version.to_string(),
            selected,
        }
        .into_element()
    });

    rect()
        .key(format!("{}:{}", game_dir.display(), flagged.hash))
        .vertical()
        .width(Size::fill())
        .spacing(12.)
        .padding(Gaps::new_all(14.))
        .corner_radius(CornerRadius::new_all(10.))
        .background(colors::page_elevated())
        .border(border_all_color(1., colors::component_border()))
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .content(Content::Flex)
                .cross_align(Alignment::Center)
                .spacing(10.)
                .child(
                    Icon::new(IconType::AlertTriangle)
                        .size(18.)
                        .color(colors::code_warn()),
                )
                .child(
                    rect()
                        .vertical()
                        .width(Size::flex(1.0))
                        .spacing(2.)
                        .child(
                            label()
                                .text(flagged.name.clone())
                                .font_size(13.)
                                .font_weight(FontWeight::SEMI_BOLD)
                                .max_lines(1)
                                .text_overflow(TextOverflow::Ellipsis)
                                .color(colors::fg_primary()),
                        )
                        .child(
                            label()
                                .text(file_line)
                                .font_size(11.)
                                .max_lines(1)
                                .text_overflow(TextOverflow::Ellipsis)
                                .color(colors::fg_secondary()),
                        ),
                ),
        )
        .maybe_child(
            flagged
                .explanation
                .clone()
                .map(|markdown| explanation_panel(markdown).into_element()),
        )
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(6.)
                .children(alternatives)
                .child(choice_option(
                    state,
                    key.clone(),
                    ModDecision::Keep,
                    current == ModDecision::Keep,
                    "Keep it",
                    "Import the mod as it is.",
                ))
                .child(choice_option(
                    state,
                    key,
                    ModDecision::Remove,
                    current == ModDecision::Remove,
                    "Leave it out",
                    "Import the instance without this mod.",
                )),
        )
        .into_element()
}

fn choice_option(
    mut state: Decisions,
    key: DecisionKey,
    decision: ModDecision,
    selected: bool,
    title: &str,
    subtitle: &str,
) -> Element {
    let id = format!("{}:{}:{title}", key.0.display(), key.1);
    let content = option_content(
        selected,
        None,
        title.to_string(),
        subtitle.to_string(),
        true,
    );
    SelectCard {
        id,
        selected,
        height: Some(OPTION_H),
        padding: Gaps::new_symmetric(0., 12.),
        content,
        on_press: (move |()| {
            state.write().insert(key.clone(), decision.clone());
        })
        .into(),
    }
    .into_element()
}

#[derive(PartialEq)]
struct AlternativeOption {
    decisions: Decisions,
    key: DecisionKey,
    alternative: ResolvedAlternative,
    already_installed: bool,
    mc_version: String,
    selected: bool,
}

impl Component for AlternativeOption {
    fn render(&self) -> impl IntoElement {
        let icon_query = use_cached_image(self.alternative.icon_url.clone(), 256);
        let icon = remote_icon(
            self.alternative.icon_url.as_deref(),
            &icon_query,
            OPTION_ICON,
        );

        let available = self.already_installed || self.alternative.version_number.is_some();
        let subtitle = match &self.alternative.version_number {
            _ if self.already_installed => "Already in this instance · use it instead".to_string(),
            Some(version) => format!("Use instead · {version}"),
            None => format!("No version for {}", self.mc_version),
        };
        let content = option_content(
            self.selected,
            Some(icon),
            self.alternative.name.clone(),
            subtitle,
            available,
        );

        if !available {
            return rect()
                .width(Size::fill())
                .height(Size::px(OPTION_H))
                .padding(Gaps::new_symmetric(0., 12.))
                .corner_radius(CornerRadius::new_all(12.))
                .background(colors::component_bg())
                .border(border_all_color(1., colors::component_border()))
                .child(content)
                .into_element();
        }

        let mut decisions = self.decisions;
        let key = self.key.clone();
        let decision = ModDecision::Replace {
            provider: self.alternative.provider,
            project_id: self.alternative.project_id.clone(),
        };
        SelectCard {
            id: format!(
                "{}:{}:{}",
                self.key.0.display(),
                self.key.1,
                self.alternative.project_id
            ),
            selected: self.selected,
            height: Some(OPTION_H),
            padding: Gaps::new_symmetric(0., 12.),
            content,
            on_press: (move |()| {
                decisions.write().insert(key.clone(), decision.clone());
            })
            .into(),
        }
        .into_element()
    }
}

fn option_content(
    selected: bool,
    icon: Option<Element>,
    title: String,
    subtitle: String,
    available: bool,
) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(10.)
        .child(marker(selected, false))
        .maybe_child(icon)
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(2.)
                .child(
                    label()
                        .text(title)
                        .font_size(13.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .max_lines(1)
                        .text_overflow(TextOverflow::Ellipsis)
                        .color(if available {
                            colors::fg_primary()
                        } else {
                            colors::fg_secondary()
                        }),
                )
                .child(
                    label()
                        .text(subtitle)
                        .font_size(11.)
                        .max_lines(1)
                        .text_overflow(TextOverflow::Ellipsis)
                        .color(colors::fg_secondary()),
                ),
        )
        .into_element()
}
