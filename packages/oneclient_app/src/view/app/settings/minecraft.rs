use freya::prelude::*;
use oneclient_core::Patch;
use oneclient_core::settings::{ProfileUpdate, Resolution};

use crate::components::{Icon, IconType, TextInput, toggle, validate_number};
use crate::hooks::{use_dispatch, use_settings_snapshot};
use super::settings_page;
use crate::theme::colors;
use crate::view::app::settings::{section_header, settings_row};

#[derive(PartialEq)]
pub struct SettingsMinecraft;

impl Component for SettingsMinecraft {
    fn render(&self) -> impl IntoElement {
        let profile = use_settings_snapshot().settings.global_game_settings;
        let dispatch = use_dispatch();

        let fullscreen = use_state({
            let v = profile.force_fullscreen.unwrap_or(false);
            move || v
        });
        let width = use_state({
            let v = profile.resolution.map(|r| r.width.to_string()).unwrap_or_default();
            move || v
        });
        let height = use_state({
            let v = profile.resolution.map(|r| r.height.to_string()).unwrap_or_default();
            move || v
        });
        let memory = use_state({
            let v = profile.mem_max.map(|m| m.to_string()).unwrap_or_default();
            move || v
        });
        let pre_launch_command = use_state({
            let v = profile.hook_pre.clone().unwrap_or_default();
            move || v
        });
        let wrapper_command = use_state({
            let v = profile.hook_wrapper.clone().unwrap_or_default();
            move || v
        });
        let post_exit_command = use_state({
            let v = profile.hook_post.clone().unwrap_or_default();
            move || v
        });

        let mut first = use_state(|| true);
        use_side_effect(move || {
            let update = build_update(
                *fullscreen.read(),
                &width.read(),
                &height.read(),
                &memory.read(),
                &pre_launch_command.read(),
                &wrapper_command.read(),
                &post_exit_command.read(),
            );
            if *first.peek() {
                first.set(false);
                return;
            }
            dispatch.update_global_profile(update);
        });

        settings_page()
            .child(section_header("GAME"))
            .child(settings_row(
                IconType::Maximize01,
                "Force Fullscreen",
                "Force Minecraft to start in fullscreen mode.",
                toggle(fullscreen),
            ))
            .child(settings_row(
                IconType::LayoutTop,
                "Resolution",
                "The game window resolution in pixels.",
                resolution_field(width, height),
            ))
            .child(settings_row(
                IconType::Database01,
                "Memory",
                "The amount of memory in megabytes allocated for the game.",
                memory_field(memory),
            ))
            .child(section_header("PROCESS"))
            .child(settings_row(
                IconType::FilePlus02,
                "Pre-Launch Command",
                "Command to run before launching the game.",
                TextInput::new(pre_launch_command)
                    .placeholder("echo 'Game started'")
                    .width(Size::px(220.)),
            ))
            .child(settings_row(
                IconType::ParagraphWrap,
                "Wrapper Command",
                "Command to run when launching the game.",
                TextInput::new(wrapper_command)
                    .placeholder("gamescope")
                    .width(Size::px(220.)),
            ))
            .child(settings_row(
                IconType::FileX02,
                "Post-Exit Command",
                "Command to run after exiting the game.",
                TextInput::new(post_exit_command)
                    .placeholder("echo 'Game exited'")
                    .width(Size::px(220.)),
            ))
            .into_element()
    }
}

fn build_update(
    fullscreen: bool,
    width: &str,
    height: &str,
    memory: &str,
    pre: &str,
    wrapper: &str,
    post: &str,
) -> ProfileUpdate {
    let resolution = match (width.trim(), height.trim()) {
        ("", "") => Patch::Clear,
        (w, h) => match (w.parse::<u32>(), h.parse::<u32>()) {
            (Ok(w), Ok(h)) => Patch::Set(Resolution::new(w, h)),
            _ => Patch::Unchanged,
        },
    };

    let mem_max = match memory.trim() {
        "" => Patch::Clear,
        m => m.parse::<u32>().map(Patch::Set).unwrap_or(Patch::Unchanged),
    };

    ProfileUpdate {
        force_fullscreen: Patch::Set(fullscreen),
        resolution,
        mem_max,
        hook_pre: command_patch(pre),
        hook_wrapper: command_patch(wrapper),
        hook_post: command_patch(post),
        ..Default::default()
    }
}

fn command_patch(value: &str) -> Patch<String> {
    match value.trim() {
        "" => Patch::Clear,
        v => Patch::Set(v.to_string()),
    }
}

fn resolution_field(width: State<String>, height: State<String>) -> impl IntoElement {
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(8.)
        .child(
            TextInput::new(width)
                .placeholder("854")
                .on_validate(validate_number)
                .text_align(TextAlign::Center)
                .width(Size::px(70.))
                .text_align(TextAlign::Center),
        )
        .child(Icon::new(IconType::X).size(14.).color(colors::fg_secondary()))
        .child(
            TextInput::new(height)
                .placeholder("480")
                .on_validate(validate_number)
                .text_align(TextAlign::Center)
                .width(Size::px(70.))
                .text_align(TextAlign::Center),
        )
        .into_element()
}

fn memory_field(memory: State<String>) -> impl IntoElement {
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(8.)
        .child(
            TextInput::new(memory)
                .width(Size::px(90.))
                .placeholder("4096")
                .on_validate(validate_number)
                .trailing(
                    label()
                        .text("MB")
                        .font_size(12.)
                        .color(colors::fg_secondary()),
                ),
        )
}
