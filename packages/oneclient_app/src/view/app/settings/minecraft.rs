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
use crate::hooks::{use_dispatch, use_settings_snapshot};
use crate::theme::colors;
use crate::view::app::settings::{resettable, resettable_all, section_header, settings_row};

#[derive(PartialEq)]
pub struct SettingsMinecraft;

impl Component for SettingsMinecraft {
    fn render(&self) -> impl IntoElement {
        let settings = use_settings_snapshot().settings;
        let profile = settings.global_game_settings.clone();
        let defaults = GameSettingsProfile::default_global_profile();
        let dispatch = use_dispatch();

        let fullscreen = use_state({
            let v = profile.force_fullscreen.unwrap_or(false);
            move || v
        });
        let width = use_state({
            let v = profile
                .resolution
                .map(|r| r.width.to_string())
                .unwrap_or_default();
            move || v
        });
        let height = use_state({
            let v = profile
                .resolution
                .map(|r| r.height.to_string())
                .unwrap_or_default();
            move || v
        });
        let memory = use_state({
            let v = profile.mem_max.map(|m| m.to_string()).unwrap_or_default();
            move || v
        });
        let jvm_args = use_state({
            let v = profile.launch_args.clone().unwrap_or_default();
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
        let update_mode = use_state({
            let v = profile.browser_update_mode.unwrap_or_default();
            move || v
        });

        #[cfg(windows)]
        let discrete_gpu = use_state({
            let v = settings.use_discrete_gpu;
            move || v
        });
        #[cfg(target_os = "linux")]
        let discrete_gpu = use_state({
            let v = profile
                .os_extra
                .as_ref()
                .and_then(|extra| extra.use_discrete_gpu)
                .unwrap_or(false);
            move || v
        });

        let mut first = use_state(|| true);
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

                dispatch.update_global_profile(update);
            });
        }

        #[cfg(windows)]
        let discrete_gpu_default = LauncherSettings::default().use_discrete_gpu;
        #[cfg(target_os = "linux")]
        let discrete_gpu_default = SettingsOsExtra::default().use_discrete_gpu.unwrap_or(false);

        let page = settings_page()
            .child(section_header("GAME"))
            .child(settings_row(
                IconType::Maximize01,
                "Force Fullscreen",
                "Force Minecraft to start in fullscreen mode.",
                resettable(
                    toggle(fullscreen),
                    fullscreen,
                    defaults.force_fullscreen.unwrap_or(false),
                ),
            ))
            .child(settings_row(
                IconType::LayoutTop,
                "Resolution",
                "The game window resolution in pixels.",
                resettable_all(
                    resolution_field(width, height),
                    vec![
                        (
                            width,
                            defaults
                                .resolution
                                .map(|r| r.width.to_string())
                                .unwrap_or_default(),
                        ),
                        (
                            height,
                            defaults
                                .resolution
                                .map(|r| r.height.to_string())
                                .unwrap_or_default(),
                        ),
                    ],
                ),
            ))
            .child(settings_row(
                IconType::Database01,
                "Memory",
                "The amount of memory in megabytes allocated for the game. Presets leave 2 GB for the system.",
                resettable(
                    memory_field(memory),
                    memory,
                    defaults.mem_max.map(|m| m.to_string()).unwrap_or_default(),
                ),
            ))
            .child(settings_row(
                IconType::Terminal,
                "JVM Arguments",
                "Extra arguments passed to Java. Separate them with spaces; quote values containing spaces.",
                resettable(
                    TextInput::new(jvm_args)
                        .placeholder("-XX:+UseG1GC")
                        .width(Size::px(220.)),
                    jvm_args,
                    defaults.launch_args.clone().unwrap_or_default(),
                ),
            ))
            .child(section_header("CONTENT"))
            .child(settings_row(
                IconType::RefreshCw01,
                "Browser Package Updates",
                "What to do when content you installed from the browser has a newer version. Packs from bundles are not affected.",
                resettable(
                    update_mode_field(update_mode),
                    update_mode,
                    defaults.browser_update_mode.unwrap_or_default(),
                ),
            ))
            .child(section_header("PROCESS"))
            .child(settings_row(
                IconType::FilePlus02,
                "Pre-Launch Command",
                "Command to run before launching the game.",
                resettable(
                    TextInput::new(pre_launch_command)
                        .placeholder("echo 'Game started'")
                        .width(Size::px(220.)),
                    pre_launch_command,
                    defaults.hook_pre.clone().unwrap_or_default(),
                ),
            ))
            .child(settings_row(
                IconType::ParagraphWrap,
                "Wrapper Command",
                "Command to run when launching the game.",
                resettable(
                    TextInput::new(wrapper_command)
                        .placeholder("gamescope")
                        .width(Size::px(220.)),
                    wrapper_command,
                    defaults.hook_wrapper.clone().unwrap_or_default(),
                ),
            ))
            .child(settings_row(
                IconType::FileX02,
                "Post-Exit Command",
                "Command to run after exiting the game.",
                resettable(
                    TextInput::new(post_exit_command)
                        .placeholder("echo 'Game exited'")
                        .width(Size::px(220.)),
                    post_exit_command,
                    defaults.hook_post.clone().unwrap_or_default(),
                ),
            ));

        #[cfg(windows)]
        let page = page.child(settings_row(
            IconType::Rocket02,
            "Prefer Dedicated GPU",
            "Ask Windows to run Java on the high-performance GPU.",
            resettable(toggle(discrete_gpu), discrete_gpu, discrete_gpu_default),
        ));

        #[cfg(target_os = "linux")]
        let page = page
            .child(section_header("GRAPHICS"))
            .child(settings_row(
                IconType::Rocket02,
                "Use Discrete GPU",
                "Render the game on the dedicated graphics card. Does nothing on a machine with only one GPU, and draws noticeably more power on a laptop.",
                resettable(toggle(discrete_gpu), discrete_gpu, discrete_gpu_default),
            ));

        page.into_element()
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
