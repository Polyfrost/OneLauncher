use freya::prelude::*;

use oneclient_common::domain::{ContentType, ProviderId};
use oneclient_events::{CrashMod, CrashModAction, CrashModIssue, GameCrash};

use crate::components::package_row::{icon_box, remote_icon};
use crate::components::{Button, IconType};
use crate::hooks::{
    ClusterAction, cluster_content_items, use_cached_image, use_cluster_content,
    use_cluster_mutation, use_dispatch, use_installs_snapshot,
};
use crate::state::MissingModInstall;
use crate::theme::colors;
use crate::view::app::cluster::package_manager::use_content_meta;

use super::{CARD_PADDING, CARD_SPACING, card};

pub(super) const MODS_TITLE_H: f32 = 18.;
pub(super) const MOD_ROW_H: f32 = 30.;
pub(super) const MOD_ROW_SPACING: f32 = 6.;
pub(super) const MORE_LABEL_H: f32 = 16.;
pub(super) const MAX_MOD_ROWS: usize = 8;

const ICON_SIZE: f32 = 28.;
const NESTED_INDENT: f32 = 20.;
const ICON_DECODE_EDGE: u32 = 64;
const NAME_MAX_W: f32 = 150.;
const VERSION_MAX_W: f32 = 80.;
const NOTE_MAX_W: f32 = 150.;

#[derive(Clone, PartialEq)]
pub(super) struct ModLine {
    row: CrashMod,
    nested: bool,
}

pub(super) fn mod_lines(crash: &GameCrash) -> (Vec<ModLine>, usize) {
    let all: Vec<ModLine> = crash
        .mod_issues
        .iter()
        .flat_map(|issue: &CrashModIssue| {
            std::iter::once(ModLine {
                row: issue.target.clone(),
                nested: false,
            })
            .chain(issue.sources.iter().map(|source| ModLine {
                row: source.clone(),
                nested: true,
            }))
        })
        .collect();

    let hidden = all.len().saturating_sub(MAX_MOD_ROWS);
    (all.into_iter().take(MAX_MOD_ROWS).collect(), hidden)
}

pub(super) fn mods_height(crash: &GameCrash) -> f32 {
    let (lines, hidden) = mod_lines(crash);
    let rows = lines.len() as f32;

    let mut height = CARD_PADDING * 2.
        + MODS_TITLE_H
        + CARD_SPACING
        + rows * MOD_ROW_H
        + MOD_ROW_SPACING * (rows - 1.).max(0.);
    if hidden > 0 {
        height += CARD_SPACING + MORE_LABEL_H;
    }

    height
}

pub(super) fn mods_card(crash: &GameCrash) -> Element {
    let (lines, hidden) = mod_lines(crash);

    CrashModRows {
        cluster_id: crash.cluster_id,
        lines,
        hidden,
    }
    .into_element()
}

#[derive(PartialEq)]
struct CrashModRows {
    cluster_id: i64,
    lines: Vec<ModLine>,
    hidden: usize,
}

impl Component for CrashModRows {
    fn render(&self) -> impl IntoElement {
        let content = use_cluster_content(self.cluster_id, ContentType::Mod);
        let installed = cluster_content_items(&content);
        let shown: Vec<_> = installed
            .iter()
            .filter(|info| {
                self.lines
                    .iter()
                    .any(|line| line.row.hash.as_deref() == Some(info.hash.as_str()))
            })
            .cloned()
            .collect();
        let meta = use_content_meta(&shown, &[], ContentType::Mod);
        let installed_projects: Vec<String> = installed
            .iter()
            .filter_map(|info| info.project_id.clone())
            .collect();

        let rows = self.lines.iter().map(|line| {
            let found = line.row.hash.as_ref().and_then(|hash| {
                let info = installed.iter().find(|info| &info.hash == hash)?;
                let provider = info.provider.unwrap_or(ProviderId::Local);
                let key = info.project_id.clone().unwrap_or_else(|| info.hash.clone());
                let meta = meta.get(&(provider, key));

                Some((
                    meta.map(|meta| meta.name.clone())
                        .filter(|name| !name.is_empty())
                        .or_else(|| info.display_name.clone()),
                    meta.and_then(|meta| meta.icon_url.clone()),
                ))
            });

            let (name, icon_url) = found.unwrap_or_default();

            let enabled_since = match &line.row.action {
                Some(CrashModAction::Enable { hash }) => installed
                    .iter()
                    .any(|info| &info.hash == hash && info.enabled),
                _ => false,
            };

            CrashModRow {
                cluster_id: self.cluster_id,
                name: name.unwrap_or_else(|| line.row.name.clone()),
                version: line.row.version.clone(),
                note: if enabled_since {
                    "Enabled".to_string()
                } else {
                    line.row.note.clone()
                },
                icon_url,
                installed: line.row.hash.is_some(),
                nested: line.nested,
                resolved: enabled_since,
                action: line.row.action.clone().filter(|_| !enabled_since),
                installed_projects: installed_projects.clone(),
            }
            .into_element()
        });

        let mut body = card()
            .spacing(CARD_SPACING)
            .child(
                label()
                    .text("Mods involved")
                    .font_size(13.)
                    .max_lines(1)
                    .color(colors::danger()),
            )
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .spacing(MOD_ROW_SPACING)
                    .children(rows),
            );

        if self.hidden > 0 {
            body = body.child(
                label()
                    .text(format!("And {} more", self.hidden))
                    .font_size(11.)
                    .max_lines(1)
                    .color(colors::fg_secondary()),
            );
        }

        body
    }
}

#[derive(PartialEq)]
struct CrashModRow {
    cluster_id: i64,
    name: String,
    version: Option<String>,
    note: String,
    icon_url: Option<String>,
    installed: bool,
    nested: bool,
    resolved: bool,
    action: Option<CrashModAction>,
    installed_projects: Vec<String>,
}

impl Component for CrashModRow {
    fn render(&self) -> impl IntoElement {
        let icon_query = use_cached_image(self.icon_url.clone(), ICON_DECODE_EDGE);
        let cluster = use_cluster_mutation();
        let dispatch = use_dispatch();
        let installs = use_installs_snapshot();

        let cluster_id = self.cluster_id;
        let install = match &self.action {
            Some(CrashModAction::Install { mod_id, .. }) => {
                installs.missing_mod_install(cluster_id, mod_id)
            }
            _ => MissingModInstall::Idle,
        };
        let installed_now = matches!(
            &install,
            MissingModInstall::Installing(project_id) | MissingModInstall::Finished(project_id)
                if self.installed_projects.contains(project_id)
        );
        let installing = !installed_now
            && matches!(
                install,
                MissingModInstall::Searching | MissingModInstall::Installing(_)
            );
        let resolved = self.resolved || installed_now;
        let note = if installed_now {
            "Installed".to_string()
        } else {
            self.note.clone()
        };

        let icon = if self.installed {
            remote_icon(self.icon_url.as_deref(), &icon_query, ICON_SIZE)
        } else {
            icon_box(IconType::HelpCircle, ICON_SIZE)
        };

        let note_color = match (resolved, self.nested) {
            (true, _) => colors::success(),
            (false, true) => colors::fg_secondary(),
            (false, false) => colors::danger(),
        };

        let action = self.action.clone().filter(|_| !installed_now);
        let button = action.map(|action| match action {
            CrashModAction::Enable { hash } => Button::new()
                .secondary()
                .small()
                .on_press(move |_| {
                    cluster.mutate(ClusterAction::SetArtifactEnabled {
                        cluster_id,
                        hash: hash.clone(),
                        enabled: true,
                    });
                })
                .text("Enable")
                .into_element(),
            CrashModAction::Install { mod_id, range } => {
                let name = self.name.clone();
                Button::new()
                    .secondary()
                    .small()
                    .disabled(installing)
                    .on_press(move |_| {
                        dispatch.install_missing_mod(
                            cluster_id,
                            mod_id.clone(),
                            name.clone(),
                            range.clone(),
                        );
                    })
                    .text(if installing { "Installing" } else { "Install" })
                    .into_element()
            }
        });

        let mut title = rect()
            .horizontal()
            .width(Size::flex(1.0))
            .cross_align(Alignment::Center)
            .spacing(6.)
            .child(
                label()
                    .text(self.name.clone())
                    .font_size(13.)
                    .font_weight(FontWeight::MEDIUM)
                    .max_width(Size::px(NAME_MAX_W))
                    .max_lines(1)
                    .text_overflow(TextOverflow::Ellipsis)
                    .color(colors::fg_primary()),
            );

        if let Some(version) = &self.version {
            title = title.child(
                label()
                    .text(version.clone())
                    .font_size(12.)
                    .max_width(Size::px(VERSION_MAX_W))
                    .max_lines(1)
                    .text_overflow(TextOverflow::Ellipsis)
                    .color(colors::fg_secondary()),
            );
        }

        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::px(MOD_ROW_H))
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(10.)
            .padding(Gaps::new(
                0.,
                0.,
                0.,
                if self.nested { NESTED_INDENT } else { 0. },
            ))
            .child(icon)
            .child(title)
            .child(
                label()
                    .text(note)
                    .font_size(11.)
                    .max_width(Size::px(NOTE_MAX_W))
                    .max_lines(1)
                    .text_overflow(TextOverflow::Ellipsis)
                    .color(note_color),
            )
            .maybe_child(button)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oneclient_events::CrashModIssueKind;

    fn row(name: &str, note: &str) -> CrashMod {
        CrashMod {
            name: name.to_string(),
            id: None,
            version: Some("1.0".to_string()),
            note: note.to_string(),
            hash: None,
            bundled_in: None,
            action: None,
        }
    }

    fn crash(issues: Vec<CrashModIssue>) -> GameCrash {
        GameCrash {
            cluster_id: 1,
            cluster_name: "Skyblock".to_string(),
            title: "Iris needs sodium".to_string(),
            exit: String::new(),
            played_secs: 0,
            cause: None,
            suspects: Vec::new(),
            remedy: None,
            fixes: Vec::new(),
            excerpt: Vec::new(),
            game_dir: None,
            mod_issues: issues,
        }
    }

    fn issue(target: &str, sources: &[&str]) -> CrashModIssue {
        CrashModIssue {
            kind: CrashModIssueKind::Missing,
            target: row(target, "Not installed"),
            sources: sources
                .iter()
                .map(|name| row(name, &format!("Needs {target}")))
                .collect(),
        }
    }

    #[test]
    fn each_issue_lists_its_mod_then_the_mods_that_need_it() {
        let (lines, hidden) = mod_lines(&crash(vec![
            issue("sodium", &["Iris", "Reese's Sodium Options"]),
            issue("fabric-api", &["Indium"]),
        ]));

        let shown: Vec<(&str, bool)> = lines
            .iter()
            .map(|line| (line.row.name.as_str(), line.nested))
            .collect();
        assert_eq!(
            shown,
            vec![
                ("sodium", false),
                ("Iris", true),
                ("Reese's Sodium Options", true),
                ("fabric-api", false),
                ("Indium", true),
            ]
        );
        assert_eq!(hidden, 0);
    }

    #[test]
    fn a_long_list_is_capped_and_the_rest_counted() {
        let sources: Vec<String> = (0..12).map(|n| format!("Mod {n}")).collect();
        let names: Vec<&str> = sources.iter().map(String::as_str).collect();
        let crash = crash(vec![issue("fabric-api", &names)]);

        let (lines, hidden) = mod_lines(&crash);
        assert_eq!(lines.len(), MAX_MOD_ROWS);
        assert_eq!(hidden, 13 - MAX_MOD_ROWS);

        let capped = mods_height(&crash);
        let full_rows = CARD_PADDING * 2.
            + MODS_TITLE_H
            + CARD_SPACING
            + MAX_MOD_ROWS as f32 * MOD_ROW_H
            + (MAX_MOD_ROWS - 1) as f32 * MOD_ROW_SPACING;
        assert_eq!(capped, full_rows + CARD_SPACING + MORE_LABEL_H);
    }

    #[test]
    fn an_incompatible_pair_takes_two_rows() {
        let pair = CrashModIssue {
            kind: CrashModIssueKind::Incompatible,
            target: row("OptiFabric", "Incompatible"),
            sources: vec![row("Sodium", "Incompatible with OptiFabric")],
        };
        let crash = crash(vec![pair]);

        let (lines, _) = mod_lines(&crash);
        assert_eq!(lines.len(), 2);
        assert_eq!(
            mods_height(&crash),
            CARD_PADDING * 2. + MODS_TITLE_H + CARD_SPACING + 2. * MOD_ROW_H + MOD_ROW_SPACING
        );
    }
}
