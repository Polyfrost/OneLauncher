use freya::prelude::*;

use crate::components::Button;
use crate::hooks::{use_dispatch, use_installs_snapshot};
use crate::i18n::{t, tf};
use crate::view::app::cluster::dialog;

#[derive(PartialEq)]
pub struct IncompatibleInstallPopup;

impl Component for IncompatibleInstallPopup {
    fn render(&self) -> impl IntoElement {
        let installs = use_installs_snapshot();
        let dispatch = use_dispatch();

        let Some(prompt) = installs.incompatible.clone() else {
            return rect().into_element();
        };

        let close = dispatch.clone();
        let cancel = dispatch.clone();

        dialog(
            t("oneclient.components.incompatible_install.title").to_string(),
            tf(
                "oneclient.components.incompatible_install.body",
                &[&prompt.name],
            ),
            None,
            move || close.dismiss_incompatible_install(),
            [
                Button::new()
                    .secondary()
                    .on_press(move |_| dispatch.install_incompatible_anyway(prompt.clone()))
                    .text(t("oneclient.components.incompatible_install.install"))
                    .into_element(),
                Button::new()
                    .primary()
                    .on_press(move |_| cancel.dismiss_incompatible_install())
                    .text(t("oneclient.common.close"))
                    .into_element(),
            ],
        )
    }
}
