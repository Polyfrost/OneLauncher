use std::collections::{HashMap, HashSet};

use freya::prelude::*;
use oneclient_content::modpacks::{SettingFile, share_path};
use oneclient_content::packages::{ContentType, LinkedArtifactInfo, ProviderId};

use crate::components::{FileChoice, LazySection, ScrollArea, file_choice_header, remote_icon};
use crate::hooks::{cluster_content_items, use_cached_image, use_cluster_content};
use crate::i18n::t;
use crate::theme::{colors, rounded, text_size};
use crate::ui::border_all_color;
use crate::utils::format_size;
use crate::view::app::cluster::package_manager::use_content_meta;

const LIST_MAX_HEIGHT: f32 = 380.;
const ICON_SIZE: f32 = 36.;
const ICON_DECODE_EDGE: u32 = 128;
const OFF_OPACITY: f32 = 0.45;
const ROW_HEIGHT: f32 = 52.;
const ROW_GAP: f32 = 4.;
const HEADER_HEIGHT: f32 = 28.;

const SECTIONS: [(ContentType, &str); 3] = [
    (
        ContentType::Mod,
        "oneclient.clusters.export.files.section.mods",
    ),
    (
        ContentType::ResourcePack,
        "oneclient.clusters.export.files.section.resource_packs",
    ),
    (
        ContentType::Shader,
        "oneclient.clusters.export.files.section.shaders",
    ),
];

#[derive(PartialEq)]
pub struct EmbeddedFiles {
    cluster_id: i64,
    choice: FileChoice,
}

impl EmbeddedFiles {
    pub fn new(cluster_id: i64, choice: FileChoice) -> Self {
        Self { cluster_id, choice }
    }
}

fn shared_path(info: &LinkedArtifactInfo) -> String {
    share_path(info.content_type, &info.cluster_file_name)
}

impl Component for EmbeddedFiles {
    fn render(&self) -> impl IntoElement {
        let files = self.choice.files.read().clone().unwrap_or_default();
        let mut sections = Vec::new();
        let mut titles = Vec::new();
        let mut rows = Vec::new();
        for (content_type, title) in SECTIONS {
            let installed =
                cluster_content_items(&use_cluster_content(self.cluster_id, content_type));
            let prefix = format!("{}/", content_type.folder_name());
            let shown: Vec<&SettingFile> = files
                .iter()
                .filter(|file| file.path.starts_with(&prefix))
                .collect();
            let shown_paths: HashSet<&str> = shown.iter().map(|file| file.path.as_str()).collect();
            let mut infos = Vec::new();
            let mut by_path = HashMap::new();
            for info in installed {
                let path = shared_path(&info);
                if shown_paths.contains(path.as_str()) {
                    by_path.insert(path, infos.len());
                    infos.push(info);
                }
            }
            let meta = use_content_meta(&infos, &[], content_type);
            if shown.is_empty() {
                continue;
            }

            sections.push(LazySection {
                header: true,
                count: shown.len(),
            });
            titles.push(title);
            rows.extend(shown.into_iter().map(|file| {
                let file_name = file.path[prefix.len()..].to_string();
                let info = by_path.get(&file.path).map(|&index| &infos[index]);
                let meta = info.and_then(|info| {
                    let provider = info.provider.unwrap_or(ProviderId::Local);
                    let key = info.project_id.clone().unwrap_or_else(|| info.hash.clone());
                    meta.get(&(provider, key))
                });
                EmbeddedRow {
                    path: file.path.clone(),
                    name: meta
                        .map(|meta| meta.name.clone())
                        .filter(|name| !name.is_empty())
                        .or_else(|| info.and_then(|info| info.display_name.clone()))
                        .unwrap_or_else(|| file_name.clone()),
                    description: meta
                        .map(|meta| meta.summary.clone())
                        .filter(|summary| !summary.is_empty())
                        .unwrap_or(file_name),
                    size: file.size,
                    icon_url: meta.and_then(|meta| meta.icon_url.clone()),
                    choice: self.choice,
                }
            }));
        }

        rect()
            .vertical()
            .width(Size::fill())
            .spacing(12.)
            .child(file_choice_header(self.choice))
            .child(
                rect()
                    .width(Size::fill())
                    .corner_radius(rounded(10.))
                    .background(colors::page())
                    .border(border_all_color(1., colors::component_border()))
                    .overflow(Overflow::Clip)
                    .child(
                        ScrollArea::new()
                            .width(Size::fill())
                            .height(Size::Inner)
                            .max_height(Size::px(LIST_MAX_HEIGHT))
                            .padding(Gaps::new_all(10.))
                            .lazy_sections(
                                sections,
                                ROW_HEIGHT,
                                ROW_GAP,
                                0.,
                                1,
                                HEADER_HEIGHT,
                                move |index| rows[index].clone().into_element(),
                                move |section| {
                                    rect()
                                        .width(Size::fill())
                                        .height(Size::fill())
                                        .main_align(Alignment::End)
                                        .padding(Gaps::new(0., 8., 2., 8.))
                                        .child(
                                            label()
                                                .text(t(titles[section]))
                                                .font_size(text_size(12.5))
                                                .font_weight(FontWeight::SEMI_BOLD)
                                                .color(colors::fg_secondary()),
                                        )
                                        .into_element()
                                },
                            ),
                    ),
            )
    }
}

#[derive(Clone, PartialEq)]
struct EmbeddedRow {
    path: String,
    name: String,
    description: String,
    size: u64,
    icon_url: Option<String>,
    choice: FileChoice,
}

impl Component for EmbeddedRow {
    fn render(&self) -> impl IntoElement {
        let icon_query = use_cached_image(self.icon_url.clone(), ICON_DECODE_EDGE);
        let selected = self.choice.is_chosen(&self.path);
        let (choice, path) = (self.choice, self.path.clone());

        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::fill())
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(12.)
            .padding(Gaps::new(8., 12., 8., 10.))
            .corner_radius(rounded(8.))
            .background(if selected {
                colors::component_bg()
            } else {
                Color::TRANSPARENT
            })
            .opacity(if selected { 1. } else { OFF_OPACITY })
            .cursor(CursorIcon::Pointer)
            .on_press(move |_| choice.toggle(&path))
            .child(remote_icon(
                self.icon_url.as_deref(),
                &icon_query,
                ICON_SIZE,
            ))
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.))
                    .spacing(2.)
                    .child(
                        label()
                            .text(self.name.clone())
                            .font_size(text_size(13.5))
                            .font_weight(FontWeight::MEDIUM)
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .color(if selected {
                                colors::fg_primary()
                            } else {
                                colors::fg_secondary()
                            }),
                    )
                    .child(
                        label()
                            .text(self.description.clone())
                            .font_size(text_size(12.))
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .color(colors::fg_secondary()),
                    ),
            )
            .child(
                label()
                    .text(format_size(self.size))
                    .font_size(text_size(12.))
                    .color(colors::fg_secondary()),
            )
    }
}
