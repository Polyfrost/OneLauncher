use super::*;

use crate::theme::colors;

const ONBOARDING_TIPS_BACKUP: &[&str] = &[
    "418 I'm a teapot",
    "Floors 8 through 10 of the Catacombs were originally planned for release, but portions of it were later merged into Floor 7 and its Master Mode counterpart.",
    "At one point, Feather Client was willing to pay $300 USD to add Skytils, a popular SkyBlock mod.",
    "Grass is a plant with narrow leaves growing from the base. [...]\nGrasses are monocotyledon, herbaceous plants.",
    "In Canada, Santa's postal code is H0H 0H0.",
    "Guvf vf abg na vzcbegnag fragrapr.",
    "Vs lbh qrpvcure guvf, lbh fubhyq yvfgra gb Mrgihr'f zhfvp.",
    "Try to do something creative! It could be music, art, coding...\nYou won't know how much you like it until you try it, so give it a shot :)",
    "At one point in Minecraft, there was a rare chance of Snow Golems naturally spawning.",
    "W[h]yvest",
    "RUMOUR. Open your ears; 9r\"5j5&?OWTY Z0d",
    "Make sure to drink water! You never know if you might be dehydrated.",
    "People asking you to \"test their mod\" are likely trying to hack you.\nThe only ones you can trust are ones widely trusted by the community.",
    "Have a great day!",
    "You can sleep through a thunderstorm, even during the day",
];

const FUNFACTS_URL: &str =
    "https://raw.githubusercontent.com/Polyfrost/DataStorage/main/oneclient/funfacts.txt";

fn normalize_line_breaks(value: &str) -> String {
    value.replace("/n", "\n").replace("\\n", "\n")
}

fn parse_funfacts(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(normalize_line_breaks)
        .collect()
}

/// Loads onboarding tips (remote with backup fallback) and returns a single
/// stable random tip for the lifetime of the component.
pub(super) fn use_onboarding_tip() -> String {
    let mut tips = use_state(|| {
        ONBOARDING_TIPS_BACKUP
            .iter()
            .map(|tip| normalize_line_breaks(tip))
            .collect::<Vec<String>>()
    });

    use_hook(move || {
        spawn(async move {
            let Ok(state) = oneclient_core::LauncherState::get() else {
                return;
            };
            match state
                .services
                .requester
                .http()
                .get(FUNFACTS_URL)
                .send()
                .await
            {
                Ok(res) if res.status().is_success() => match res.text().await {
                    Ok(text) => {
                        let parsed = parse_funfacts(&text);
                        if !parsed.is_empty() {
                            tips.set(parsed);
                        }
                    }
                    Err(err) => {
                        tracing::warn!("onboarding tips: reading body failed: {err}");
                    }
                },
                Ok(res) => {
                    tracing::warn!("onboarding tips: unexpected status {}", res.status());
                }
                Err(err) => {
                    tracing::warn!("onboarding tips: request failed, using backup facts: {err}");
                }
            }
        });
    });

    let seed = use_hook(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as usize)
            .unwrap_or(0)
    });

    let list = tips.read();
    if list.is_empty() {
        String::new()
    } else {
        list[seed % list.len()].clone()
    }
}

pub(super) fn tip_panel(tip: String) -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::px(320.))
        .spacing(4.)
        .cross_align(Alignment::End)
        .child(
            label()
                .text("TIP")
                .width(Size::fill())
                .font_size(11.)
                .font_weight(FontWeight::SEMI_BOLD)
                .text_align(TextAlign::Right)
                .color(colors::fg_secondary()),
        )
        .child(
            label()
                .text(tip)
                .width(Size::fill())
                .font_size(13.)
                .text_align(TextAlign::Right)
                .color(colors::fg_secondary()),
        )
}
