use freya::{
    animation::{AnimNum, Ease, OnCreation, use_animation},
    prelude::*,
    router::RouterContext,
};
use oneclient_core::notification::NotificationLevel;

use crate::{
    Route,
    components::{Button, ButtonVariant, Icon, IconType, OverlayPopup, ScrollArea},
    hooks::{use_dispatch, use_notifications_snapshot},
    notifications::{InboxEntry, NotificationActionKind, TransferStats},
    theme::colors,
    utils::{format_duration_hms, format_size},
};

#[derive(PartialEq)]
pub struct NotificationCenter;

impl Component for NotificationCenter {
    fn render(&self) -> impl IntoElement {
        let open = use_notifications_snapshot().center_open;
        let dispatch = use_dispatch();

        if !open {
            return rect().into_element();
        }

        OverlayPopup::new()
            .position(Position::new_global().top(72.).right(40.))
            .on_close(move |()| dispatch.close_notification_center())
            .child(NotificationPanel)
            .into_element()
    }
}

#[derive(PartialEq)]
struct NotificationPanel;

impl Component for NotificationPanel {
    fn render(&self) -> impl IntoElement {
        let inbox = use_notifications_snapshot().inbox;

        let intro = use_animation(|conf| {
            conf.on_creation(OnCreation::Run);
            AnimNum::new(0., 1.).time(200).ease(Ease::Out)
        });

        let progress = intro.read().value();

        let entries = inbox.len();
        let mut rows: Vec<Element> = Vec::new();
        if entries == 0 {
            rows.push(
                label()
                    .text("No notifications")
                    .font_size(13.)
                    .color(colors::fg_secondary())
                    .into_element(),
            );
        } else {
            let last = entries - 1;
            for (i, entry) in inbox.into_iter().enumerate() {
                let id = entry.id;
                rows.push(NotifEntryRow::new(entry, i != last).key(id).into_element());
            }
        }

        let content = ScrollArea::new()
            .width(Size::fill())
            .height(Size::flex(1.0))
            .spacing(8.)
            .children(rows);

        rect()
            .vertical()
            .content(Content::Flex)
            .width(Size::px(368.))
            .height(Size::px(480.))
            .padding(Gaps::new_all(12.))
            .spacing(8.)
            .opacity(progress)
            .margin(Gaps::new((1.0 - progress) * -8.0, 0., 0., 0.))
            .background(colors::page_elevated().with_a(220))
            .blur(12.)
            .corner_radius(CornerRadius::new_all(12.))
            .border(
                Border::new()
                    .fill(colors::component_border())
                    .width(1.)
                    .alignment(BorderAlignment::Inner),
            )
            .shadow(Shadow::from((
                0.,
                8.,
                32.,
                0.,
                Color::from_argb(120, 0, 0, 0),
            )))
            .child(
                label()
                    .text("Notifications")
                    .font_size(18.)
                    .font_weight(FontWeight::MEDIUM)
                    .a11y_role(AccessibilityRole::TitleBar),
            )
            .child(divider())
            .child(content)
            .child(divider())
            .child(Footer)
    }
}

#[derive(PartialEq)]
struct NotifEntryRow {
    entry: InboxEntry,
    show_divider: bool,
    key: DiffKey,
}

impl KeyExt for NotifEntryRow {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl NotifEntryRow {
    fn new(entry: InboxEntry, show_divider: bool) -> Self {
        Self {
            entry,
            show_divider,
            key: DiffKey::None,
        }
    }
}

impl Component for NotifEntryRow {
    fn render(&self) -> impl IntoElement {
        let entry = self.entry.clone();
        let id = entry.id;
        let dispatch = use_dispatch();
        let enter_dispatch = dispatch.clone();
        let body_dispatch = dispatch.clone();
        let click_dismissable = entry.click_dismissable();
        let has_actions = !entry.actions.is_empty();

        let mut hovered = use_state(|| false);
        let expanded = use_state(|| false);

        let content = rect()
            .horizontal()
            .width(Size::fill())
            .spacing(13.)
            .cross_align(Alignment::Start)
            .padding(Gaps::new_all(8.))
            .corner_radius(CornerRadius::new_all(8.))
            .maybe(*hovered.read() && !has_actions, |el| {
                el.background(colors::ghost_overlay_hover())
            })
            .on_pointer_enter(move |_| {
                hovered.set(true);
                enter_dispatch.mark_notification_read(id);
            })
            .on_pointer_leave(move |_| hovered.set(false))
            .maybe(click_dismissable, |el| {
                el.on_press(move |_| dispatch.dismiss_notification(id))
            })
            .child(
                rect()
                    .width(Size::px(32.))
                    .height(Size::px(32.))
                    .corner_radius(CornerRadius::new_all(6.))
                    .center()
                    .child(
                        Icon::new(icon_for(&entry))
                            .size(24.)
                            .color(level_color(&entry.level)),
                    ),
            )
            .child(row_body(&entry, body_dispatch));

        rect()
            .vertical()
            .width(Size::fill())
            .spacing(8.)
            .child(content)
            .maybe(!entry.tasks.is_empty(), |el| {
                el.child(tasks_section(&entry, expanded))
            })
            .maybe(self.show_divider, |el| el.child(divider()))
    }
}

const TASK_LIMIT: usize = 12;

fn tasks_section(entry: &InboxEntry, mut expanded: State<bool>) -> impl IntoElement {
    let is_expanded = *expanded.read();
    let count = entry.tasks.len();

    let mut section = rect()
        .vertical()
        .width(Size::fill())
        .spacing(6.)
        .margin(Gaps::new(2., 0., 0., 44.))
        .child(
            rect()
                .horizontal()
                .cross_align(Alignment::Center)
                .spacing(6.)
                .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
                .on_press(move |_| {
                    let now = !*expanded.peek();
                    expanded.set(now);
                })
                .child(ChevronToggle { expanded: is_expanded })
                .child(
                    label()
                        .text(if is_expanded {
                            "Hide tasks".to_string()
                        } else {
                            format!("Show {count} tasks")
                        })
                        .font_size(11.)
                        .color(colors::fg_secondary()),
                ),
        );

    if is_expanded {
        let mut list = rect().vertical().width(Size::fill()).spacing(4.);
        for task in entry.tasks.iter().take(TASK_LIMIT) {
            list = list.child(task_row(task));
        }
        if count > TASK_LIMIT {
            list = list.child(
                label()
                    .text(format!("+{} more", count - TASK_LIMIT))
                    .font_size(10.)
                    .color(colors::fg_secondary()),
            );
        }
        section = section.child(list);
    }

    section.into_element()
}

fn task_row(task: &crate::notifications::TaskView) -> impl IntoElement {
    let percent = ((task.current as f32 / task.total.max(1) as f32) * 100.0).round() as u64;
    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .spacing(8.)
        .content(Content::Flex)
        .child(phase_badge(task.phase))
        .child(
            label()
                .text(task.label.clone())
                .font_size(11.)
                .max_lines(1)
                .width(Size::flex(1.0))
                .color(colors::fg_primary()),
        )
        .maybe(task.total_count > 1, |el| {
            el.child(
                label()
                    .text(format!("{}/{}", task.done_count, task.total_count))
                    .font_size(10.)
                    .color(colors::fg_secondary()),
            )
        })
        .child(
            label()
                .text(format!("{percent}%"))
                .font_size(10.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

/// A right-facing chevron that rotates 90° down when `expanded`, animated.
#[derive(PartialEq)]
struct ChevronToggle {
    expanded: bool,
}

impl Component for ChevronToggle {
    fn render(&self) -> impl IntoElement {
        let expanded = self.expanded;
        let anim = use_animation(|_| AnimNum::new(0., 90.).time(160).ease(Ease::Out));

        use_side_effect_with_deps(&expanded, move |&expanded| {
            let mut anim = anim;
            if expanded {
                anim.start();
            } else {
                anim.reverse();
            }
        });

        let deg = anim.get().value();
        rect().rotate(deg).child(
            Icon::new(IconType::ChevronRight)
                .size(14.)
                .color(colors::fg_secondary()),
        )
    }
}

fn phase_badge(phase: &str) -> impl IntoElement {
    let color = match phase {
        "Verifying" => colors::code_warn(),
        "Extracting" => colors::success(),
        "Installing" => colors::brand(),
        _ => colors::brand(),
    };
    rect()
        .center()
        .padding(Gaps::new_symmetric(1., 7.))
        .corner_radius(CornerRadius::new_all(999.))
        .background(color.with_a(40))
        .child(
            label()
                .text(phase.to_string())
                .font_size(9.)
                .font_weight(FontWeight::MEDIUM)
                .color(color),
        )
        .into_element()
}

fn row_body(entry: &InboxEntry, dispatch: crate::BridgeDispatch) -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::flex(1.0))
        .spacing(2.)
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .content(Content::Flex)
                .cross_align(Alignment::Start)
                .child(
                    label()
                        .text(entry.title.clone())
                        .font_size(14.)
                        .font_weight(FontWeight::MEDIUM)
                        .color(level_color(&entry.level))
                        .width(Size::flex(1.0)),
                )
                .child(
                    rect()
                        .horizontal()
                        .cross_align(Alignment::Center)
                        .spacing(6.)
                        .child(
                            label()
                                .text(relative_time(entry))
                                .font_size(10.)
                                .color(colors::fg_secondary()),
                        )
                        .maybe(!entry.read, |el| {
                            el.child(
                                rect()
                                    .width(Size::px(6.))
                                    .height(Size::px(6.))
                                    .corner_radius(CornerRadius::new_all(3.))
                                    .background(colors::brand()),
                            )
                        }),
                ),
        )
        .child(
            label()
                .text(entry.body.clone())
                .font_size(12.)
                .color(colors::fg_secondary()),
        )
        .child(row_extra(entry, dispatch))
}

fn row_extra(entry: &InboxEntry, dispatch: crate::BridgeDispatch) -> impl IntoElement {
    if let Some((current, total)) = entry.progress.filter(|_| entry.is_loading) {
        let frac = (current as f32 / total.max(1) as f32).clamp(0.0, 1.0);
        let bar = rect()
            .width(Size::fill())
            .height(Size::px(6.))
            .corner_radius(CornerRadius::new_all(50.))
            .background(colors::brand().with_a(128))
            .child(
                rect()
                    .width(Size::percent(frac * 100.0))
                    .height(Size::px(6.))
                    .corner_radius(CornerRadius::new_all(50.))
                    .background(colors::brand()),
            );

        return rect()
            .vertical()
            .width(Size::fill())
            .spacing(5.)
            .margin(Gaps::new(8., 0., 0., 0.))
            .child(bar)
            .maybe_child(entry.transfer.map(transfer_footer))
            .into_element();
    }

    if !entry.actions.is_empty() {
        let id = entry.id;
        let dismiss_dispatch = dispatch.clone();

        let mut row = rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(8.)
            .margin(Gaps::new(8., 0., 0., 0.))
            .child(
                Button::new()
                    .small()
                    .secondary()
                    .width(Size::flex(1.0))
                    .text("Dismiss")
                    .on_press(move |_| dismiss_dispatch.dismiss_notification(id)),
            );

        for (i, action) in entry.actions.iter().enumerate() {
            let action_dispatch = dispatch.clone();
            let kind = action.kind.clone();
            row = row.child(
                Button::new()
                    .small()
                    .width(Size::flex(1.0))
                    .text(action.label.clone())
                    .variant(if i == 0 {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::Secondary
                    })
                    .on_press(move |_| run_action(&action_dispatch, &kind)),
            );
        }

        return row.into_element();
    }

    rect().into_element()
}

fn transfer_footer(stats: TransferStats) -> Element {
    let speed = format!("{}/s", format_size(stats.speed_bps as u64));
    let eta = stats
        .eta_secs
        .map(|secs| format!("{} left", format_duration_hms(secs as i64)));

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .main_align(Alignment::SpaceBetween)
        .child(
            label()
                .text(speed)
                .font_size(10.)
                .color(colors::fg_secondary()),
        )
        .maybe_child(eta.map(|eta| {
            label()
                .text(eta)
                .font_size(10.)
                .color(colors::fg_secondary())
                .into_element()
        }))
        .into_element()
}

fn run_action(dispatch: &crate::BridgeDispatch, kind: &NotificationActionKind) {
    match kind {
        NotificationActionKind::OpenClusterUpdate(summary) => {
            dispatch.open_cluster_update(summary.clone());
        }
    }
}

fn divider() -> impl IntoElement {
    rect()
        .width(Size::fill())
        .height(Size::px(1.))
        .background(colors::component_border())
}

#[derive(PartialEq)]
struct Footer;

impl Component for Footer {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let clear_dispatch = dispatch.clone();
        let open_settings = move |_| {
            dispatch.close_notification_center();
            let _ = RouterContext::get().push(Route::SettingsAppearance {});
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .main_align(Alignment::SpaceBetween)
            .child(
                Button::new()
                    .ghost()
                    .small()
                    .on_press(move |_| clear_dispatch.clear_notification_inbox())
                    .child(
                        Icon::new(IconType::Trash01)
                            .size(16.)
                            .color(colors::fg_primary()),
                    )
                    .text("Clear all"),
            )
            .child(
                Button::new()
                    .ghost()
                    .icon()
                    .on_press(open_settings)
                    .child(Icon::new(IconType::Settings02).size(18.)),
            )
    }
}

fn icon_for(entry: &InboxEntry) -> IconType {
    if let Some(icon) = entry.icon {
        return icon;
    }
    if entry.is_loading && entry.progress.is_some() {
        return IconType::DownloadCloud02;
    }
    match entry.level {
        NotificationLevel::Error => IconType::AlertTriangle,
        NotificationLevel::Info => IconType::InfoCircle,
    }
}

fn level_color(level: &NotificationLevel) -> Color {
    match level {
        NotificationLevel::Info => colors::fg_primary(),
        NotificationLevel::Error => colors::danger(),
    }
}

fn relative_time(entry: &InboxEntry) -> String {
    let secs = entry.created_at.elapsed().as_secs();
    if secs < 60 {
        "Just now".to_string()
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86400)
    }
}
