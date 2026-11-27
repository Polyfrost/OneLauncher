use freya::prelude::*;

use freya::router::RouterContext;

use crate::components::{Button, Icon, IconType, ScrollArea, centered_spinner};
use crate::hooks::{query_error, try_global_analytics, use_global_analytics};
use crate::i18n::t;
use crate::routes::Route;
use crate::theme::colors;
use crate::view::app::settings::retry_button;
use crate::view::app::{analytics_body, analytics_empty};

#[derive(PartialEq)]
pub struct Stats;

impl Component for Stats {
    fn render(&self) -> impl IntoElement {
        let query = use_global_analytics();
        let analytics = try_global_analytics(&query);
        // A failed query settles with no value; without this it would spin forever
        let failed = analytics.is_none() && query_error(&query).is_some();

        // Loading, failure and the empty state fill the page; only a real dashboard scrolls
        let analytics = match &analytics {
            None if failed => {
                return page(error_state((move |()| query.invalidate()).into()));
            }
            None => return page(loading_state()),
            Some(a) if a.playtime.session_count == 0 => return page(empty_state()),
            Some(a) => a,
        };

        page(
            ScrollArea::new()
                .width(Size::fill())
                .height(Size::fill())
                .auto_hide_scrollbar()
                .child(analytics_body(analytics))
                .into_element(),
        )
    }
}

fn page(body: Element) -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::fill())
        .overflow(Overflow::Clip)
        .child(body)
        .into_element()
}

fn empty_state() -> Element {
    analytics_empty(
        t("oneclient.stats.empty.title"),
        t("oneclient.stats.empty.hint"),
        Button::new()
            .primary()
            .on_press(|_| {
                let _ = RouterContext::get().push(Route::Clusters {});
            })
            .child(Icon::new(IconType::Play).size(15.))
            .text(t("oneclient.stats.empty.action"))
            .into_element(),
    )
}

fn error_state(retry: EventHandler<()>) -> Element {
    super::analytics::analytics_message(
        IconType::AlertTriangle,
        colors::code_warn(),
        t("oneclient.stats.error.title"),
        t("oneclient.stats.error.hint"),
        retry_button(retry),
    )
}

fn loading_state() -> Element {
    centered_spinner(t("oneclient.stats.loading"))
}
