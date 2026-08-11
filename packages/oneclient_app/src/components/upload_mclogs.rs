use freya::{elements::extensions::ChildrenExt, prelude::{Component, State, WritableUtils}};

use crate::components::{Button, Icon, IconType};
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
    fn render(&self) -> impl freya::prelude::IntoElement {
        let mut clone = self.confirm.clone();

        return Button::new()
            .secondary()
            .enabled(self.has_log)
            .on_press(move |_| clone.set(Some(Confirm::Upload)))
            .child(Icon::new(IconType::LinkExternal01).size(15.))
            .text("Upload to mclo.gs")
    }
}