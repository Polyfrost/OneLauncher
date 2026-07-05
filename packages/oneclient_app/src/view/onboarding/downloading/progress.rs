use super::*;

use crate::components::Button;
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::utils::{format_duration_hms, format_size};

pub(super) fn failure_panel(
    failures: &[InstallFailure],
    done: usize,
    total: usize,
    running: bool,
    on_retry: impl FnMut(Event<PressEventData>) + 'static,
    on_continue: impl FnMut(Event<PressEventData>) + 'static,
) -> impl IntoElement {
    let lines: Vec<Element> = failures
        .iter()
        .take(8)
        .map(|failure| {
            label()
                .text(format!(
                    "{} (cluster {}): {}",
                    failure.plan.mc_version, failure.plan.cluster_id, failure.reason
                ))
                .font_size(12.)
                .color(colors::fg_primary())
                .into_element()
        })
        .collect();
    let extra = failures.len().saturating_sub(8);

    rect()
        .vertical()
        .width(Size::px(640.))
        .margin(Gaps::new(24., 0., 0., 0.))
        .padding(Gaps::new_all(20.))
        .spacing(10.)
        .corner_radius(CornerRadius::new_all(12.))
        .background(colors::danger().with_a(26))
        .border(border_all_color(1., colors::danger().with_a(128)))
        .child(
            label()
                .text("Some versions could not be prepared.")
                .font_size(16.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_primary()),
        )
        .child(
            label()
                .text(format!("Processed: {done} / {total}"))
                .font_size(12.)
                .color(colors::fg_secondary()),
        )
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(4.)
                .padding(Gaps::new_all(12.))
                .corner_radius(CornerRadius::new_all(8.))
                .background(Color::BLACK.with_a(51))
                .children(lines)
                .maybe_child((extra > 0).then(|| {
                    label()
                        .text(format!("+{extra} more failures"))
                        .font_size(12.)
                        .color(colors::fg_secondary())
                        .into_element()
                })),
        )
        .child(
            rect()
                .horizontal()
                .spacing(10.)
                .child(
                    Button::new()
                        .primary()
                        .enabled(!running)
                        .on_press(on_retry)
                        .text("Retry Failed Versions"),
                )
                .child(
                    Button::new()
                        .secondary()
                        .on_press(on_continue)
                        .text("Continue Anyway"),
                ),
        )
        .into_element()
}

pub(super) struct ProgressView<'a> {
    pub(super) global: f32,
    pub(super) stage: &'a DownloadStage,
    pub(super) agg: &'a GroupedAgg,
    pub(super) activity: &'a str,
    pub(super) speed_bps: f64,
    pub(super) total_estimate: u64,
    pub(super) elapsed_secs: Option<u64>,
    pub(super) done: usize,
    pub(super) total: usize,
    pub(super) predownload: bool,
    pub(super) running: bool,
}

pub(super) fn progress_panel(view: ProgressView) -> impl IntoElement {
    let ProgressView {
        global,
        stage,
        agg,
        activity,
        speed_bps,
        total_estimate,
        elapsed_secs,
        done,
        total,
        predownload,
        running,
    } = view;

    let percentage = global.round() as i32;

    let position = if !running && done >= total && total > 0 {
        "Done".to_string()
    } else if predownload && stage.total > 0 {
        format!(
            "Version {} of {} — {}",
            (stage.index + 1).min(stage.total),
            stage.total,
            stage.label
        )
    } else if total > 0 {
        format!("Step {done} of {total}")
    } else {
        "Preparing...".to_string()
    };

    let tasks = agg.task_list();
    let live_count = tasks.len();
    let shown = live_count.min(MAX_TASK_ROWS);
    let overflow = live_count.saturating_sub(shown);
    let task_rows: Vec<Element> = tasks.into_iter().take(shown).map(task_row).collect();

    let activity_text = agg.summary().unwrap_or_else(|| {
        if activity.is_empty() {
            position.clone()
        } else {
            activity.to_string()
        }
    });

    let stats = build_stats(StatsView {
        global,
        speed_bps,
        total_estimate,
        elapsed_secs,
        running,
        done,
        total,
    });

    rect()
        .vertical()
        .width(Size::px(460.))
        .spacing(10.)
        .cross_align(Alignment::Start)
        .main_align(Alignment::End)
        .maybe_child((!task_rows.is_empty()).then(|| {
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(8.)
                .children(task_rows)
                .maybe_child((overflow > 0).then(|| {
                    label()
                        .text(format!("+{overflow} more"))
                        .font_size(11.)
                        .color(colors::fg_secondary())
                        .into_element()
                }))
                .into_element()
        }))
        .child(
            ActivityIndicator {
                running,
                text: activity_text,
            }
            .into_element(),
        )
        .maybe_child(stats)
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .cross_align(Alignment::Center)
                .content(Content::Flex)
                .child(
                    label()
                        .text(format!("{percentage}%"))
                        .width(Size::flex(1.0))
                        .font_size(24.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(position)
                        .font_size(11.)
                        .max_lines(1)
                        .color(colors::fg_secondary()),
                ),
        )
        .child(progress_track(
            global,
            4.,
            Color::WHITE,
            Color::WHITE.with_a(51),
        ))
        .into_element()
}

struct StatsView {
    global: f32,
    speed_bps: f64,
    total_estimate: u64,
    elapsed_secs: Option<u64>,
    running: bool,
    done: usize,
    total: usize,
}

fn build_stats(view: StatsView) -> Option<Element> {
    let StatsView {
        global,
        speed_bps,
        total_estimate,
        elapsed_secs,
        running,
        done,
        total,
    } = view;

    let elapsed = elapsed_secs?;
    let finished = !running && done >= total && total > 0;

    let mut parts: Vec<String> = vec![format!("Elapsed {}", format_duration_hms(elapsed as i64))];

    if finished {
        parts.push("Complete".to_string());
    } else {
        if global > 2.0 && global < 100.0 {
            let remaining = (elapsed as f64) * ((100.0 - global as f64) / global as f64);
            parts.push(format!(
                "~{} left",
                format_duration_hms(remaining.round() as i64)
            ));
        } else {
            parts.push("~— left".to_string());
        }

        if speed_bps >= 1.0 {
            parts.push(format!("{}/s", format_size(speed_bps as u64)));
        }
    }

    if total_estimate > 0 {
        let downloaded = ((global as f64 / 100.0) * total_estimate as f64) as u64;
        parts.push(format!(
            "{} / ~{}",
            format_size(downloaded),
            format_size(total_estimate)
        ));
    }

    Some(
        label()
            .text(parts.join("   ·   "))
            .font_size(11.)
            .max_lines(1)
            .color(colors::fg_secondary())
            .into_element(),
    )
}

fn task_row(task: TaskLine) -> Element {
    let is_bytes = task.total > 1;
    let pct = if task.total > 0 {
        (task.current as f32 / task.total as f32 * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    let detail = if is_bytes {
        format!(
            "{} / {}",
            format_size(task.current),
            format_size(task.total)
        )
    } else {
        task.phase.to_string()
    };

    let track_pct = if is_bytes { pct } else { 100.0 };
    let fill = if is_bytes {
        colors::brand()
    } else {
        colors::brand().with_a(120)
    };

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(4.)
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .content(Content::Flex)
                .cross_align(Alignment::Center)
                .spacing(10.)
                .child(
                    label()
                        .text(format!("{} {}", task.phase, task.label))
                        .width(Size::flex(1.0))
                        .font_size(12.)
                        .max_lines(1)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(detail)
                        .font_size(11.)
                        .max_lines(1)
                        .color(colors::fg_secondary()),
                ),
        )
        .child(progress_track(track_pct, 3., fill, Color::WHITE.with_a(28)))
        .into_element()
}

#[derive(PartialEq)]
struct ActivityIndicator {
    running: bool,
    text: String,
}

impl Component for ActivityIndicator {
    fn render(&self) -> impl IntoElement {
        let pulse = use_animation(|conf| {
            conf.on_creation(OnCreation::Run);
            conf.on_finish(OnFinish::reverse());
            AnimNum::new(0.3, 1.0)
                .time(650)
                .ease(Ease::InOut)
                .function(Function::Cubic)
        });

        let dot_opacity = if self.running {
            pulse.read().value()
        } else {
            0.9
        };
        let color = if self.running {
            colors::brand()
        } else {
            colors::fg_secondary()
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .cross_align(Alignment::Center)
            .spacing(8.)
            .content(Content::Flex)
            .child(
                rect()
                    .width(Size::px(8.))
                    .height(Size::px(8.))
                    .corner_radius(CornerRadius::new_all(4.))
                    .background(color)
                    .opacity(dot_opacity),
            )
            .child(
                label()
                    .text(self.text.clone())
                    .width(Size::flex(1.0))
                    .font_size(12.)
                    .max_lines(1)
                    .color(colors::fg_primary()),
            )
    }
}

fn progress_track(pct: f32, height: f32, fill: Color, bg: Color) -> impl IntoElement {
    rect()
        .width(Size::fill())
        .height(Size::px(height))
        .corner_radius(CornerRadius::new_all(height / 2.))
        .background(bg)
        .child(
            rect()
                .width(Size::percent(pct.clamp(0.0, 100.0)))
                .height(Size::fill())
                .corner_radius(CornerRadius::new_all(height / 2.))
                .background(fill),
        )
        .into_element()
}
