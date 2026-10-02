use freya::prelude::*;
use oneclient_core::clusters::{ImportedModpack, ModpackSource, list_imported_modpacks};

use super::folder_list::dialog;
use crate::components::{Button, Icon, IconType, remote_icon};
use crate::hooks::{use_cached_image, use_dispatch, use_installs_snapshot};
use crate::launcher::{self, off_ui};
use crate::theme::colors;
use crate::view::app::settings::{section_header, settings_row};

const MODPACK_EXTENSIONS: [&str; 2] = ["mrpack", "zip"];
const PACK_ICON: f32 = 32.;

fn load_packs(cluster_id: i64, mut packs: State<Vec<ImportedModpack>>) {
    spawn(async move {
        let Ok(state) = launcher::state() else { return };
        let found = off_ui(async move { list_imported_modpacks(&state, cluster_id).await }).await;
        match found {
            Ok(found) => packs.set(found),
            Err(err) => {
                tracing::debug!(cluster_id, error = %err, "could not list the added modpacks")
            }
        }
    });
}

fn pick_modpack(cluster_id: i64, dispatch: crate::Actions) {
    spawn(async move {
        let Some(handle) = rfd::AsyncFileDialog::new()
            .set_title("Choose a modpack")
            .add_filter("Modpack", &MODPACK_EXTENSIONS)
            .pick_file()
            .await
        else {
            return;
        };
        dispatch.import_modpack(cluster_id, ModpackSource::File(handle.path().to_path_buf()));
    });
}

#[derive(PartialEq)]
pub struct ImportedModpacksSection {
    pub cluster_id: i64,
}

impl Component for ImportedModpacksSection {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let dispatch = use_dispatch();
        let installs = use_installs_snapshot();
        let packs = use_state(Vec::<ImportedModpack>::new);
        let pending_remove = use_state(|| None::<ImportedModpack>);
        let busy = installs.modpack_busy;

        use_hook(move || load_packs(cluster_id, packs));

        let mut was_busy = use_state(|| busy);
        if *was_busy.peek() != busy {
            was_busy.set(busy);
            if !busy {
                load_packs(cluster_id, packs);
            }
        }

        let import = dispatch.clone();
        let add_button = Button::new()
            .small()
            .secondary()
            .enabled(!busy)
            .maybe(!busy, |el| {
                el.on_press(move |_| pick_modpack(cluster_id, import.clone()))
            })
            .text(if busy { "Working..." } else { "Choose file" });

        let rows: Vec<Element> = packs
            .read()
            .iter()
            .map(|pack| {
                PackRow {
                    pack: pack.clone(),
                    busy,
                    pending_remove,
                }
                .into_element()
            })
            .collect();

        let confirm = pending_remove.read().clone().map(|pack| {
            let mut cancel = pending_remove;
            let mut close = pending_remove;
            let mut done = pending_remove;
            let dispatch = dispatch.clone();
            let name = pack.name.clone();
            dialog(
                format!("Remove {name}?"),
                "Removes the mods and files this pack added. Mods you added yourself, mods another pack also uses and config files you edited stay."
                    .to_string(),
                None,
                move || close.set(None),
                [
                    Button::new()
                        .secondary()
                        .on_press(move |_| cancel.set(None))
                        .text("Cancel")
                        .into_element(),
                    Button::new()
                        .danger()
                        .on_press(move |_| {
                            done.set(None);
                            dispatch.remove_imported_modpack(
                                cluster_id,
                                pack.bundle_name.clone(),
                                pack.name.clone(),
                            );
                        })
                        .child(Icon::new(IconType::Trash01).size(14.))
                        .text("Remove pack")
                        .into_element(),
                ],
            )
        });

        rect()
            .vertical()
            .width(Size::fill())
            .spacing(4.)
            .child(section_header("ADDED MODPACKS"))
            .child(settings_row(
                IconType::FilePlus02,
                "Add a Modpack",
                "Add the mods from a Modrinth .mrpack or CurseForge .zip to this instance. The pack must be for the same Minecraft version and loader.",
                add_button,
            ))
            .children(rows)
            .maybe_child(confirm)
    }
}

#[derive(PartialEq)]
struct PackRow {
    pack: ImportedModpack,
    busy: bool,
    pending_remove: State<Option<ImportedModpack>>,
}

impl Component for PackRow {
    fn render(&self) -> impl IntoElement {
        let icon_query = use_cached_image(self.pack.icon_url.clone(), 256);
        let icon = remote_icon(self.pack.icon_url.as_deref(), &icon_query, PACK_ICON);
        let description = match &self.pack.provider {
            Some(provider) if !self.pack.version.trim().is_empty() => {
                format!("Version {} from {provider}", self.pack.version)
            }
            Some(provider) => format!("From {provider}"),
            None if !self.pack.version.trim().is_empty() => {
                format!("Version {} from a file", self.pack.version)
            }
            None => "Added from a file".to_string(),
        };

        let mut pending = self.pending_remove;
        let pack = self.pack.clone();
        let busy = self.busy;

        rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(16.)
            .padding(Gaps::new_symmetric(12., 16.))
            .corner_radius(CornerRadius::new_all(12.))
            .background(colors::page_elevated())
            .child(icon)
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(2.)
                    .child(
                        label()
                            .text(self.pack.name.clone())
                            .font_size(16.)
                            .font_weight(FontWeight::MEDIUM)
                            .max_lines(1)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        label()
                            .text(description)
                            .font_size(12.)
                            .color(colors::fg_secondary()),
                    ),
            )
            .child(
                Button::new()
                    .small()
                    .secondary()
                    .enabled(!busy)
                    .maybe(!busy, |el| {
                        el.on_press(move |_| pending.set(Some(pack.clone())))
                    })
                    .child(Icon::new(IconType::Trash01).size(14.))
                    .text("Remove pack"),
            )
    }
}
