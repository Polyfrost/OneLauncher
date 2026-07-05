use freya::prelude::*;
use oneclient_core::ScreenshotInfo;

use crate::components::{Button, Icon, IconType, LocalImage, OverlayPopup};
use crate::hooks::{ScreenshotAction, use_dispatch, use_screenshot_action};
use crate::theme::colors;
use crate::ui::fmt_date;
use crate::utils::{format_res, format_size};

#[derive(PartialEq)]
pub struct ScreenshotViewer {
    shots: Vec<ScreenshotInfo>,
    start: usize,
    on_close: EventHandler<()>,
}

impl ScreenshotViewer {
    pub fn new(
        shots: Vec<ScreenshotInfo>,
        start: usize,
        on_close: impl Into<EventHandler<()>>,
    ) -> Self {
        Self {
            shots,
            start,
            on_close: on_close.into(),
        }
    }
}

impl Component for ScreenshotViewer {
    fn render(&self) -> impl IntoElement {
        let shots = self.shots.clone();
        let len = shots.len();

        let mut index = use_state({
            let start = self.start.min(len.saturating_sub(1));
            move || start
        });

        if len == 0 {
            return rect().into_element();
        }
        let idx = (*index.read()).min(len - 1);
        let info = shots[idx].clone();

        let action = use_screenshot_action();
        let dispatch = use_dispatch();

        let close = self.on_close.clone();
        let scrim_close = self.on_close.clone();
        let delete_close = self.on_close.clone();

        let has_prev = idx > 0;
        let has_next = idx + 1 < len;

        let preview = LocalImage::new(info.path.clone(), 0, false).skeleton(true);

        let open_path = info.path.clone();
        let copy_path = info.path.clone();
        let copy_dispatch = dispatch.clone();
        let delete_path = info.path.clone();

        OverlayPopup::new()
            .on_close(move |_| scrim_close.call(()))
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .on_global_key_down(move |e: Event<KeyboardEventData>| match &e.key {
                        Key::Named(NamedKey::ArrowLeft) if idx > 0 => index.set(idx - 1),
                        Key::Named(NamedKey::ArrowRight) if idx + 1 < len => index.set(idx + 1),
                        _ => {}
                    })
                    .child(
                        rect()
                            .vertical()
                            .width(Size::window_percent(88.))
                            .height(Size::window_percent(90.))
                            .spacing(12.)
                            .padding(Gaps::new_all(16.))
                            .content(Content::Flex)
                            .child(header_row(&info, idx, len, move |_| close.call(())))
                            .child(
                                rect()
                                    .width(Size::fill())
                                    .height(Size::flex(1.0))
                                    .horizontal()
                                    .cross_align(Alignment::Center)
                                    .content(Content::Flex)
                                    .spacing(12.)
                                    .overflow(Overflow::Clip)
                                    .child(chevron_btn(IconType::ArrowLeft, has_prev, move |_| {
                                        if idx > 0 {
                                            index.set(idx - 1);
                                        }
                                    }))
                                    .child(
                                        rect()
                                            .width(Size::flex(1.0))
                                            .height(Size::fill())
                                            .child(preview),
                                    )
                                    .child(chevron_btn(IconType::ArrowRight, has_next, move |_| {
                                        if idx + 1 < len {
                                            index.set(idx + 1);
                                        }
                                    })),
                            )
                            .child(
                                rect()
                                    .horizontal()
                                    .width(Size::fill())
                                    .main_align(Alignment::End)
                                    .spacing(8.)
                                    .child(
                                        Button::new()
                                            .secondary()
                                            .on_press(move |_| {
                                                if let Some(dir) = open_path.parent() {
                                                    crate::platform::open_url(&dir.to_string_lossy());
                                                }
                                            })
                                            .child(Icon::new(IconType::Folder).size(14.))
                                            .text("Open in folder"),
                                    )
                                    .child(
                                        Button::new()
                                            .secondary()
                                            .on_press(move |_| {
                                                crate::platform::copy_image_to_clipboard(
                                                    copy_path.clone(),
                                                );
                                                copy_dispatch
                                                    .notify("Copied to clipboard")
                                                    .body("Screenshot copied to your clipboard.")
                                                    .info()
                                                    .icon(IconType::ClipboardCheck)
                                                    .send();
                                            })
                                            .child(Icon::new(IconType::Copy01).size(14.))
                                            .text("Copy"),
                                    )
                                    .child(
                                        Button::new()
                                            .danger()
                                            .on_press(move |_| {
                                                action.mutate(ScreenshotAction::Delete {
                                                    path: delete_path.clone(),
                                                });
                                                delete_close.call(());
                                            })
                                            .child(Icon::new(IconType::Trash01).size(14.))
                                            .text("Delete"),
                                    ),
                            ),
                    ),
            )
            .into_element()
    }
}

fn chevron_btn(
    icon: IconType,
    enabled: bool,
    on_press: impl Into<EventHandler<Event<PressEventData>>>,
) -> impl IntoElement {
    let base = rect()
        .width(Size::px(44.))
        .height(Size::px(44.))
        .center()
        .corner_radius(CornerRadius::new_all(22.));

    if !enabled {
        return base
            .background(Color::from_argb(70, 0, 0, 0))
            .on_press(|_| {})
            .child(
                Icon::new(icon)
                    .size(26.)
                    .color(colors::fg_secondary().with_a(90)),
            )
            .into_element();
    }

    base.background(Color::from_argb(140, 0, 0, 0))
        .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
        .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
        .on_press(on_press)
        .child(Icon::new(icon).size(26.).color(colors::fg_primary()))
        .into_element()
}

fn header_row(
    info: &ScreenshotInfo,
    idx: usize,
    len: usize,
    on_close: impl Into<EventHandler<Event<PressEventData>>>,
) -> impl IntoElement {
    let mut meta = format!("{} • {}", format_size(info.size_bytes), fmt_date(info.created));
    
    if let Some(res) = info.resolution {
        meta = format!("{} • {}", format_res(res), meta);
    }

    if len > 1 {
        meta = format!("{} of {} • {meta}", idx + 1, len);
    }

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(2.)
                .child(
                    label()
                        .text(info.name.clone())
                        .font_size(15.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(meta)
                        .font_size(11.)
                        .color(colors::fg_secondary()),
                ),
        )
        .child(
            Button::new()
                .ghost()
                .icon()
                .on_press(on_close)
                .child(Icon::new(IconType::XClose).size(18.)),
        )
        .into_element()
}
