use freya::{
    animation::{AnimNum, Ease, Function, OnCreation, use_animation},
    prelude::*,
};
use oneclient_core::notification::NotificationLevel;

use crate::{
    components::{Button, ButtonSize, Icon, IconType},
    hooks::{use_dispatch, use_notifications_snapshot},
    notifications::{InboxEntry, NotificationActionKind},
    theme::colors,
};

const TOAST_W: f32 = 300.;
const TRAVEL: f32 = 320.;
const TTL_MS: f32 = 5000.;

#[derive(Clone, Copy, PartialEq)]
enum Variant {
    Progress,
    Action,
    Message,
}

impl Variant {
    fn of(entry: &InboxEntry) -> Self {
        if entry.is_loading && entry.progress.is_some() {
            Self::Progress
        } else if !entry.actions.is_empty() {
            Self::Action
        } else {
            Self::Message
        }
    }

}

fn entry_icon(entry: &InboxEntry) -> (IconType, Color) {
    if entry.is_loading && entry.progress.is_some() {
        return (IconType::FolderDownload, colors::fg_primary());
    }

    match entry.level {
        NotificationLevel::Error => (IconType::AlertTriangle, colors::danger()),
        NotificationLevel::Info => (IconType::DownloadCloud02, colors::fg_primary()),
    }
}

#[derive(PartialEq)]
pub struct Toasts;

impl Component for Toasts {
    fn render(&self) -> impl IntoElement {
        let snapshot = use_notifications_snapshot();

        let entries: Vec<InboxEntry> = snapshot
            .active_toast_entry_ids
            .iter()
            .filter_map(|id| snapshot.inbox.iter().find(|e| e.id == *id).cloned())
            .collect();

        if entries.is_empty() {
            return rect().into_element();
        }

        let mut stack = rect()
            .vertical()
            .spacing(12.)
            .position(Position::new_global().top(64.).right(24.))
            .layer(Layer::RelativeOverlay(3));

        for entry in entries {
            let id = entry.id;
            stack = stack.child(
                ToastCard {
                    entry,
                    key: DiffKey::None,
                }
                .key(id),
            );
        }

        stack.into_element()
    }
}

#[derive(PartialEq)]
struct ToastCard {
    entry: InboxEntry,
    key: DiffKey,
}

impl KeyExt for ToastCard {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for ToastCard {
    fn render(&self) -> impl IntoElement {
        let entry = self.entry.clone();
        let id = entry.id;
        let variant = Variant::of(&entry);
        let dispatch = use_dispatch();

        let slide = use_animation(|conf| {
            conf.on_creation(OnCreation::Run);
            AnimNum::new(TRAVEL, 0.)
                .time(320)
                .function(Function::Cubic)
                .ease(Ease::Out)
        });
        let fade = use_animation(|conf| {
            conf.on_creation(OnCreation::Run);
            AnimNum::new(0., 1.).time(260).ease(Ease::Out)
        });

        let mut ttl_bar_anim = use_animation(|conf| {
            conf.on_creation(OnCreation::Run);
            AnimNum::new(100., 0.).time(TTL_MS as u64)
        });

        let bump_enter = dispatch.clone();
        let bump_move = dispatch.clone();

        let offset = slide.read().value();
        let opacity = fade.read().value();
        let ttl_pct = ttl_bar_anim.read().value();

        let card = match variant {
            Variant::Progress => progress_card(&entry),
            Variant::Action => action_card(&entry, dispatch.clone(), id, ttl_pct),
            Variant::Message => message_card(&entry, dispatch, id, ttl_pct),
        };

        rect()
            .width(Size::px(TOAST_W))
            .offset_x(offset)
            .opacity(opacity)
            .on_pointer_enter(move |_| {
                bump_enter.bump_toast(id);
                ttl_bar_anim.start();
            })
            .on_pointer_move(move |_| {
                bump_move.bump_toast(id);
                ttl_bar_anim.start();
            })
            .child(card)
    }
}

fn toast_shell(entry: &InboxEntry) -> Rect {
    let border_color = match entry.level {
        NotificationLevel::Error => colors::danger(),
        NotificationLevel::Info => colors::component_border(),
    };
    rect()
        .width(Size::px(TOAST_W))
        .background(colors::component_bg())
        .corner_radius(CornerRadius::new_all(12.))
        .overflow(Overflow::Clip)
        .border(
            Border::new()
                .fill(border_color)
                .width(1.)
                .alignment(BorderAlignment::Inner),
        )
}

fn icon_badge(entry: &InboxEntry) -> impl IntoElement {
    let (icon, color) = entry_icon(entry);
    rect()
        .width(Size::px(32.))
        .height(Size::px(32.))
        .corner_radius(CornerRadius::new_all(6.))
        .center()
        .child(Icon::new(entry.icon.unwrap_or(icon)).size(24.).color(color))
}

fn header(entry: &InboxEntry) -> impl IntoElement {
    let title_color = match entry.level {
        NotificationLevel::Error => colors::danger(),
        NotificationLevel::Info => colors::fg_primary(),
    };
    let body_color = colors::fg_secondary();

    rect()
        .vertical()
        .width(Size::flex(1.0))
        .main_align(Alignment::Center)
        .spacing(2.)
        .child(
            label()
                .text(entry.title.clone())
                .font_size(16.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(title_color),
        )
        .child(
            label()
                .text(entry.body.clone())
                .font_size(12.)
                .color(body_color),
        )
}

fn ttl_bar(entry: &InboxEntry, pct: f32) -> impl IntoElement {
    let color = match entry.level {
        NotificationLevel::Error => colors::danger(),
        NotificationLevel::Info => colors::brand(),
    };
    rect()
        .width(Size::fill())
        .height(Size::px(4.))
        .background(color.with_a(60))
        .child(
            rect()
                .width(Size::percent(pct.clamp(0.0, 100.0)))
                .height(Size::px(4.))
                .corner_radius(CornerRadius::new_all(60.))
                .background(color),
        )
}

fn close_button(dispatch: crate::BridgeDispatch, id: u64) -> impl IntoElement {
    rect()
        .position(Position::new_absolute().top(8.).right(8.))
        .child(
            Button::new()
                .ghost()
                .size(ButtonSize::Icon)
                .width(Size::px(28.))
                .height(Size::px(28.))
                .on_press(move |_| dispatch.dismiss_toast(id))
                .child(
                    Icon::new(IconType::XClose)
                        .size(16.)
                        .color(colors::fg_secondary()),
                ),
        )
}

fn message_card(
    entry: &InboxEntry,
    dispatch: crate::BridgeDispatch,
    id: u64,
    ttl_pct: f32,
) -> Rect {
    toast_shell(entry)
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .content(Content::Flex)
                .padding(Gaps::new(15., 40., 17., 15.))
                .spacing(16.)
                .cross_align(Alignment::Center)
                .child(icon_badge(entry))
                .child(header(entry)),
        )
        .child(close_button(dispatch, id))
        .child(ttl_bar(entry, ttl_pct))
}

fn action_card(entry: &InboxEntry, dispatch: crate::BridgeDispatch, id: u64, ttl_pct: f32) -> Rect {
    let action = entry.actions.first().cloned();

    toast_shell(entry)
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .padding(Gaps::new(15., 15., 12., 15.))
                .spacing(10.)
                .child(
                    rect()
                        .horizontal()
                        .width(Size::fill())
                        .content(Content::Flex)
                        .spacing(16.)
                        .cross_align(Alignment::Center)
                        .child(icon_badge(entry))
                        .child(header(entry)),
                )
                .child(divider())
                .child(action_row(action, dispatch, id)),
        )
        .child(ttl_bar(entry, ttl_pct))
}

fn action_row(
    action: Option<crate::notifications::NotificationAction>,
    dispatch: crate::BridgeDispatch,
    id: u64,
) -> impl IntoElement {
    let dismiss = dispatch.clone();
    let review_label = action
        .as_ref()
        .map(|a| a.label.clone())
        .unwrap_or_else(|| "Review".to_string());
    let review_kind = action.map(|a| a.kind);

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(8.)
        .child(
            Button::new()
                .small()
                .secondary()
                .width(Size::flex(1.0))
                .on_press(move |_| dismiss.dismiss_toast(id))
                .child(Icon::new(IconType::XClose).size(16.))
                .text("Dismiss"),
        )
        .child(
            Button::new()
                .small()
                .primary()
                .width(Size::flex(1.0))
                .on_press(move |_| match &review_kind {
                    Some(kind) => run_action(&dispatch, kind),
                    None => dispatch.mark_notification_read(id),
                })
                .child(Icon::new(IconType::Eye).size(16.))
                .text(review_label),
        )
}

fn run_action(dispatch: &crate::BridgeDispatch, kind: &NotificationActionKind) {
    match kind {
        NotificationActionKind::OpenClusterUpdate(summary) => {
            dispatch.open_cluster_update(summary.clone());
        }
    }
}

fn progress_card(entry: &InboxEntry) -> Rect {
    let frac = entry
        .progress
        .map(|(cur, total)| cur as f32 / total.max(1) as f32)
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);

    toast_shell(entry).child(
        rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .padding(Gaps::new(12., 14., 12., 14.))
            .spacing(13.)
            .cross_align(Alignment::Start)
            .child(icon_badge(entry))
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(2.)
                    .child(
                        label()
                            .text(entry.title.clone())
                            .font_size(14.)
                            .font_weight(FontWeight::SEMI_BOLD)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        label()
                            .text(entry.body.clone())
                            .font_size(12.)
                            .color(colors::fg_secondary()),
                    )
                    .child(
                        rect()
                            .width(Size::fill())
                            .height(Size::px(6.))
                            .margin(Gaps::new(8., 0., 0., 0.))
                            .corner_radius(CornerRadius::new_all(50.))
                            .background(colors::brand().with_a(128))
                            .child(
                                rect()
                                    .width(Size::percent(frac * 100.0))
                                    .height(Size::px(6.))
                                    .corner_radius(CornerRadius::new_all(50.))
                                    .background(colors::brand()),
                            ),
                    ),
            ),
    )
}

fn divider() -> impl IntoElement {
    rect()
        .width(Size::fill())
        .height(Size::px(1.))
        .background(colors::component_border())
}
