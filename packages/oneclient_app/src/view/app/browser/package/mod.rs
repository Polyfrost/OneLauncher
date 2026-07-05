use freya::prelude::*;
use oneclient_core::packages::{ContentType, ProviderId};

use crate::components::ScrollArea;
use crate::hooks::{
    content_type_for_slug, project_detail, use_browser_compat, use_dispatch,
    use_link_confirm, use_package_project, use_package_versions, version_list, versions_total,
};
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::view::app::cluster::load_cluster;

use super::{PackageBanner, Thumbnail};
use crate::utils::abbreviate_number;

mod panels;
mod sidebar;
use panels::{about_panel, gallery_panel, loading_body, tabs, versions_panel};
use sidebar::sidebar;

const PANEL_BG: Color = Color::from_rgb(21, 28, 34);
const SIDEBAR_W: f32 = 280.;
const SCROLLBAR_GUTTER: f32 = 18.;

fn decode_package_id(package_id: &str) -> (ProviderId, String) {
    match package_id.split_once(':') {
        Some((repr, id)) => {
            let provider = repr
                .parse::<u8>()
                .ok()
                .and_then(ProviderId::from_repr)
                .unwrap_or(ProviderId::Modrinth);
            (provider, id.to_string())
        }
        None => (ProviderId::Modrinth, package_id.to_string()),
    }
}

fn pill(text: String) -> impl IntoElement {
    rect()
        .center()
        .padding(Gaps::new_symmetric(2., 8.))
        .corner_radius(CornerRadius::new_all(999.))
        .background(colors::component_bg())
        .border(border_all_color(1., colors::component_border()))
        .child(
            label()
                .text(text)
                .font_size(10.)
                .max_lines(1)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

fn pill_flow(items: &[String], per_row: usize, max: usize) -> impl IntoElement {
    let shown: Vec<String> = items.iter().take(max).cloned().collect();
    let overflow = items.len().saturating_sub(shown.len());
    let mut all = shown;
    if overflow > 0 {
        all.push(format!("+{overflow}"));
    }

    rect()
        .vertical()
        .spacing(4.)
        .children(all.chunks(per_row.max(1)).map(|chunk| {
            rect()
                .horizontal()
                .spacing(4.)
                .children(chunk.iter().map(|t| pill(t.clone()).into_element()))
                .into_element()
        }))
        .into_element()
}

fn pill_detail(key: &str, items: &[String]) -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(6.)
        .child(
            label()
                .text(key.to_string())
                .font_size(12.)
                .color(colors::fg_secondary()),
        )
        .child(pill_flow(items, 4, 18))
        .into_element()
}

#[derive(PartialEq)]
pub struct BrowserPackage {
    pub cluster_id: i64,
    pub package_type: String,
    pub package_id: String,
}

impl Component for BrowserPackage {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let (provider, project_id) = decode_package_id(&self.package_id);
        let content_type = content_type_for_slug(&self.package_type);

        let active_tab = use_state(|| 0usize);
        let versions_page = use_state(|| 0usize);
        let compatible_only = use_browser_compat();
        let dispatch = use_dispatch();
        let confirm = use_link_confirm();

        let cluster = load_cluster(cluster_id);
        let compat = *compatible_only.read();
        let (game_version, loader) = match (compat, &cluster) {
            (true, Some(c)) => (
                Some(c.mc_version.clone()),
                (content_type == ContentType::Mod).then_some(c.mc_loader),
            ),
            _ => (None, None),
        };

        let mut versions_page_state = versions_page;
        let mut last_compat = use_state(|| compat);
        if *last_compat.peek() != compat {
            last_compat.set(compat);
            if *versions_page_state.peek() != 0 {
                versions_page_state.set(0);
            }
        }

        let project_query = use_package_project(provider, project_id.clone());
        let versions_query = use_package_versions(
            provider,
            project_id.clone(),
            game_version,
            loader,
            *versions_page.read(),
        );

        let project = project_detail(&project_query);
        let versions = version_list(&versions_query);
        let total_versions = versions_total(&versions_query);
        let latest_version = versions.first().map(|v| v.version_id.clone());

        let gallery = project
            .as_ref()
            .map(|p| p.gallery.clone())
            .unwrap_or_default();
        let has_gallery = !gallery.is_empty();
        let max_tab = if has_gallery { 2 } else { 1 };
        let current = (*active_tab.read()).min(max_tab);

        let body = match (&project, current) {
            (None, _) => loading_body().into_element(),
            (Some(project), 0) => about_panel(project).into_element(),
            (Some(project), 1) => versions_panel(
                versions,
                total_versions,
                versions_page,
                provider,
                project.id.clone(),
                cluster_id,
                dispatch.clone(),
            )
            .into_element(),
            (Some(_), _) => gallery_panel(gallery).into_element(),
        };

        let row = rect()
            .horizontal()
            .width(Size::fill())
            .cross_align(Alignment::Start)
            .spacing(24.)
            .content(Content::Flex)
            .child(sidebar(
                project,
                latest_version,
                provider,
                cluster_id,
                dispatch,
                confirm,
            ))
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(12.)
                    .child(tabs(current, active_tab, has_gallery))
                    .child(body),
            )
            .into_element();

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .overflow(Overflow::Clip)
            .padding(Gaps::new(0., 40., 40., 40.))
            .child(
                ScrollArea::new()
                    .width(Size::fill())
                    .height(Size::fill())
                    .reset_key(current as u64)
                    .padding(Gaps::new(0., SCROLLBAR_GUTTER, 0., 0.))
                    .children([row]),
            )
            .into_element()
    }
}

