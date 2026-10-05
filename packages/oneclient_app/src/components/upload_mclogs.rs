use freya::prelude::*;
use freya::query::MutationStateData;
use freya::text_edit::Clipboard;

use crate::components::{Button, Icon, IconType};
use crate::hooks::{UseUploadLog, use_dispatch};
use crate::view::app::cluster::logs::Confirm;

#[derive(PartialEq)]
pub struct UploadToMclogs {
    has_log: bool,
    confirm: State<Option<Confirm>>,
}

impl UploadToMclogs {
    pub fn new(has_log: bool, confirm: State<Option<Confirm>>) -> Self {
        Self { has_log, confirm }
    }
}

impl Component for UploadToMclogs {
    fn render(&self) -> impl IntoElement {
        let mut confirm = self.confirm;

        Button::new()
            .secondary()
            .enabled(self.has_log)
            .on_press(move |_| confirm.set(Some(Confirm::Upload)))
            .child(Icon::new(IconType::LinkExternal01).size(15.))
            .text("Upload to mclo.gs")
    }
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
                    .notify("Uploaded to mclo.gs")
                    .body(format!("{} (copied to clipboard)", result.url))
                    .info()
                    .icon(IconType::LinkExternal01)
                    .send();
            }
            Err(err) => {
                dispatch
                    .notify("Upload failed")
                    .body(err.to_string())
                    .error()
                    .send();
            }
        }
    });
}
