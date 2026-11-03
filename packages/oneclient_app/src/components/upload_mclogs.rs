use std::path::PathBuf;

use freya::prelude::*;
use freya::query::MutationStateData;
use freya::text_edit::Clipboard;

use crate::components::{Button, Icon, IconType};
use crate::hooks::{UseUploadLog, use_dispatch};
use crate::i18n::{t, tf};
use crate::launcher::off_ui;
use crate::view::app::cluster::logs::Confirm;

#[derive(PartialEq)]
pub struct UploadToMclogs {
    path: Option<PathBuf>,
    confirm: State<Option<Confirm>>,
}

impl UploadToMclogs {
    pub fn new(path: Option<PathBuf>, confirm: State<Option<Confirm>>) -> Self {
        Self { path, confirm }
    }
}

impl Component for UploadToMclogs {
    fn render(&self) -> impl IntoElement {
        let mut confirm = self.confirm;
        let path = self.path.clone();
        let current_path = use_reactive(&self.path);

        Button::new()
            .secondary()
            .enabled(path.is_some())
            .on_press(move |_| {
                let Some(path) = path.clone() else {
                    return;
                };
                spawn(async move {
                    let checked = path.clone();
                    let check =
                        off_ui(
                            async move { oneclient_core::log_exceeds_upload_limit(&checked).await },
                        )
                        .await;
                    if *current_path.peek() != Some(path) {
                        return;
                    }
                    let too_long = match check {
                        Ok(too_long) => too_long,
                        Err(err) => {
                            tracing::warn!("log size check failed: {err:?}");
                            false
                        }
                    };
                    confirm.set(Some(if too_long {
                        Confirm::UploadTooLong
                    } else {
                        Confirm::Upload
                    }));
                });
            })
            .child(Icon::new(IconType::LinkExternal01).size(15.))
            .text(t("oneclient.components.upload_mclogs.upload"))
    }
}

pub fn notify_log_truncated(dispatch: &crate::Actions) {
    dispatch
        .notify(t("oneclient.components.upload_mclogs.truncated.title"))
        .body(t("oneclient.components.upload_mclogs.truncated.body"))
        .info()
        .icon(IconType::AlertTriangle)
        .send();
}

pub fn use_mclogs_feedback(upload: UseUploadLog) {
    let dispatch = use_dispatch();
    let mut handled = use_state(|| match &*upload.peek().state() {
        MutationStateData::Settled {
            settlement_instant, ..
        } => Some(*settlement_instant),
        _ => None,
    });

    use_side_effect(move || {
        let reader = upload.read();
        let state = reader.state();
        let MutationStateData::Settled {
            res,
            settlement_instant,
        } = &*state
        else {
            return;
        };

        if *handled.peek() == Some(*settlement_instant) {
            return;
        }
        handled.set(Some(*settlement_instant));

        match res {
            Ok(result) => {
                if let Err(err) = Clipboard::set(result.url.clone()) {
                    tracing::warn!("clipboard copy failed: {err:?}");
                }

                dispatch
                    .notify(t("oneclient.components.upload_mclogs.uploaded.title"))
                    .body(tf(
                        "oneclient.components.upload_mclogs.uploaded.body",
                        &[&result.url],
                    ))
                    .info()
                    .icon(IconType::LinkExternal01)
                    .send();

                if result.truncated {
                    notify_log_truncated(&dispatch);
                }
            }
            Err(err) => {
                dispatch
                    .notify(t("oneclient.components.upload_mclogs.failed"))
                    .body(err.to_string())
                    .error()
                    .send();
            }
        }
    });
}
