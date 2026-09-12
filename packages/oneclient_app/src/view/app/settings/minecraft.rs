use freya::prelude::*;
use oneclient_common::Patch;
use oneclient_core::settings::{
    GameSettingsProfile, PackageUpdateMode, ProfileUpdate, Resolution,
};
#[cfg(windows)]
use oneclient_core::settings::LauncherSettings;
#[cfg(target_os = "linux")]
use oneclient_core::settings::SettingsOsExtra;

use super::settings_page;
use crate::components::{
    Dropdown, Icon, IconType, TextInput, memory_field, toggle, validate_number,
};
use crate::hooks::{ResetNotice, use_dispatch, use_game_active, use_settings_snapshot};
use crate::theme::colors;
use crate::view::app::settings::{reset_row, section_header, settings_row, take_notice};

const RESET_NOTICE: ResetNotice = ResetNotice {
    title: "Minecraft settings reset",
    body: "Window, memory, arguments, commands and the browser update mode are back to \
           their defaults. Your Java installations are untouched.",
};

#[derive(PartialEq)]
pub struct SettingsMinecraft;

impl Component for SettingsMinecraft {
    fn render(&self) -> impl IntoElement {
        let settings = use_settings_snapshot().settings;
        let profile = settings.global_game_settings.clone();
        let dispatch = use_dispatch();

        let mut fullscreen = use_state({
            let v = profile.force_fullscreen.unwrap_or(false);
            move || v
        });
        let mut width = use_state({
            let v = profile
                .resolution
                .map(|r| r.width.to_string())
                .unwrap_or_default();
            move || v
        });
        let mut height = use_state({
            let v = profile
                .resolution
                .map(|r| r.height.to_string())
                .unwrap_or_default();
            move || v
        });
        let mut memory = use_state({
            let v = profile.mem_max.map(|m| m.to_string()).unwrap_or_default();
            move || v
        });
        let mut jvm_args = use_state({
            let v = profile.launch_args.clone().unwrap_or_default();
            move || v
        });
        let mut pre_launch_command = use_state({
            let v = profile.hook_pre.clone().unwrap_or_default();
            move || v
        });
        let mut wrapper_command = use_state({
            let v = profile.hook_wrapper.clone().unwrap_or_default();
            move || v
        });
        let mut post_exit_command = use_state({
            let v = profile.hook_post.clone().unwrap_or_default();
            move || v
        });
        let mut update_mode = use_state({
            let v = profile.browser_update_mode.unwrap_or_default();
            move || v
        });

        #[cfg(windows)]
        let mut discrete_gpu = use_state({
            let v = settings.use_discrete_gpu;
            move || v
        });
        #[cfg(target_os = "linux")]
        let mut discrete_gpu = use_state({
            let v = profile
                .os_extra
                .as_ref()
                .and_then(|extra| extra.use_discrete_gpu)
                .unwrap_or(false);
            move || v
        });

        let mut first = use_state(|| true);
        let mut announce = use_state(|| false);
        {
            let dispatch = dispatch.clone();
            use_side_effect(move || {
                let update = build_update(
                    *fullscreen.read(),
                    &width.read(),
                    &height.read(),
                    &memory.read(),
                    &jvm_args.read(),
                    &pre_launch_command.read(),
                    &wrapper_command.read(),
                    &post_exit_command.read(),
                    *update_mode.read(),
                );
                #[cfg(any(windows, target_os = "linux"))]
                let gpu = *discrete_gpu.read();
                if *first.peek() {
                    first.set(false);
                    return;
                }

                #[cfg(windows)]
                dispatch.stage_settings(move |settings| settings.use_discrete_gpu = gpu);
                #[cfg(target_os = "linux")]
                let update = with_discrete_gpu(update, gpu);

                dispatch.update_global_profile_notifying(
                    update,
                    take_notice(announce, RESET_NOTICE),
                );
            });
        }

        let game_active = use_game_active();
        let confirming_reset = use_state(|| false);
        let reset = move |()| {
            let defaults = GameSettingsProfile::default_global_profile();
            announce.set(true);
            fullscreen.set(defaults.force_fullscreen.unwrap_or(false));
            width.set(
                defaults
                    .resolution
                    .map(|r| r.width.to_string())
                    .unwrap_or_default(),
            );
            height.set(
                defaults
                    .resolution
                    .map(|r| r.height.to_string())
                    .unwrap_or_default(),
            );
            memory.set(defaults.mem_max.map(|m| m.to_string()).unwrap_or_default());
            jvm_args.set(defaults.launch_args.clone().unwrap_or_default());
            pre_launch_command.set(defaults.hook_pre.clone().unwrap_or_default());
            wrapper_command.set(defaults.hook_wrapper.clone().unwrap_or_default());
            post_exit_command.set(defaults.hook_post.clone().unwrap_or_default());
            update_mode.set(defaults.browser_update_mode.unwrap_or_default());
            #[cfg(windows)]
            discrete_gpu.set(LauncherSettings::default().use_discrete_gpu);
            #[cfg(target_os = "linux")]
            discrete_gpu.set(
                SettingsOsExtra::default()
                    .use_discrete_gpu
                    .unwrap_or(false),
            );
        };

        let page = settings_page()
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
                "The amount of memory in megabytes allocated for the game. Presets leave 2 GB for the system.",
                memory_field(memory, "Default", oneclient_common::default_mem_max()),
            ))
            .child(settings_row(
                IconType::Terminal,
                "JVM Arguments",
                "Extra arguments passed to Java. Separate them with spaces; quote values containing spaces.",
                TextInput::new(jvm_args)
                    .placeholder("-XX:+UseG1GC")
                    .width(Size::px(220.)),
            ))
            .child(section_header("CONTENT"))
            .child(settings_row(
                IconType::RefreshCw01,
                "Browser Package Updates",
                "What to do when content you installed from the browser has a newer version. Packs from bundles are not affected.",
                update_mode_field(update_mode),
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
            ));

        #[cfg(windows)]
        let page = page.child(settings_row(
            IconType::Rocket02,
            "Prefer Dedicated GPU",
            "Ask Windows to run Java on the high-performance GPU.",
            toggle(discrete_gpu),
        ));

        #[cfg(target_os = "linux")]
        let page = page
            .child(section_header("GRAPHICS"))
            .child(settings_row(
                IconType::Rocket02,
                "Use Discrete GPU",
                "Render the game on the dedicated graphics card. Does nothing on a machine with only one GPU, and draws noticeably more power on a laptop.",
                toggle(discrete_gpu),
            ));

        page.child(reset_row(
            confirming_reset,
            game_active,
            "Put the options on this page back to their defaults.",
            "Force Fullscreen, Resolution, Memory, JVM Arguments, the three process commands, \
             Browser Package Updates and the GPU preference all go back to their defaults. This \
             is the global profile only: your Java installations, and any cluster that overrides \
             these values, are left alone.",
            reset.into(),
        ))
        .into_element()
    }
}

#[cfg(target_os = "linux")]
fn with_discrete_gpu(mut update: ProfileUpdate, on: bool) -> ProfileUpdate {
    let base = crate::launcher::state()
        .ok()
        .and_then(|state| state.settings.read().global_game_settings.os_extra.clone())
        .unwrap_or_default();

    update.os_extra = Patch::Set(SettingsOsExtra {
        use_discrete_gpu: Some(on),
        ..base
    });
    update
}

#[allow(clippy::too_many_arguments)]
fn build_update(
    fullscreen: bool,
    width: &str,
    height: &str,
    memory: &str,
    jvm_args: &str,
    pre: &str,
    wrapper: &str,
    post: &str,
    update_mode: PackageUpdateMode,
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
        launch_args: command_patch(jvm_args),
        hook_pre: command_patch(pre),
        hook_wrapper: command_patch(wrapper),
        hook_post: command_patch(post),
        browser_update_mode: Patch::Set(update_mode),
        ..Default::default()
    }
}

fn update_mode_field(mut selected: State<PackageUpdateMode>) -> impl IntoElement {
    let options: Vec<String> = PackageUpdateMode::ALL
        .iter()
        .map(|mode| mode.label().to_string())
        .collect();

    Dropdown::new(selected.read().label(), options)
        .width(Size::px(220.))
        .height(Size::px(34.))
        .on_select(move |idx: usize| {
            if let Some(mode) = PackageUpdateMode::ALL.get(idx).copied() {
                selected.set(mode);
            }
        })
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
        .child(
            Icon::new(IconType::X)
                .size(14.)
                .color(colors::fg_secondary()),
        )
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
