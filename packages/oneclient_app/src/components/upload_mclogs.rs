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
        Self {
            has_log,
            confirm
        }
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
    let mut handled = use_state(|| None::<String>);

    use_side_effect(move || match &*upload.read().state() {
        MutationStateData::Settled {
            res: Ok(result), ..
        } => {
            if handled.peek().as_deref() == Some(result.url.as_str()) {
                return;
            }

            handled.set(Some(result.url.clone()));
            let _ = Clipboard::set(result.url.clone());

            dispatch
                .notify("Uploaded to mclo.gs")
                .body(format!("{} (copied to clipboard)", result.url))
                .info()
                .icon(IconType::LinkExternal01)
                .send();
        }
        MutationStateData::Settled { res: Err(err), .. } => {
            let msg = err.to_string();
            if handled.peek().as_deref() == Some(msg.as_str()) {
                return;
            }

            handled.set(Some(msg.clone()));
            dispatch.notify("Upload failed").body(msg).error().send();
        }
        _ => {}
    });
}
