//! OneClient installs Java from Microsoft now but an install that predates the
//! switch keeps whatever vendor it was given back then and nothing would ever
//! replace it So anyone without a Microsoft runtime on record is offered one
//! once and either takes it declines it for this launch or silences the offer

use std::time::Duration;

use freya::prelude::spawn_forever;
use freya::radio::RadioStation;
use oneclient_events::{Choice, Prompt};
use oneclient_java::JavaVendor;

use crate::hooks::Actions;
use crate::launcher;
use crate::state::{AppChannel, AppState};

/// Front-ends match on these
pub const MICROSOFT_JAVA_CHOICE_INSTALL: &str = "java.microsoft.install";
pub const MICROSOFT_JAVA_CHOICE_NEVER: &str = "java.microsoft.never";

const MICROSOFT_JAVA_MAJOR: u32 = 25;

const SLOT_POLL: Duration = Duration::from_millis(500);

/// Long enough to outlast a first sync on a slow connection
const SLOT_ATTEMPTS: usize = 120;

enum MicrosoftJavaAnswer {
    Install,
    Never,
}

fn offer_prompt() -> Prompt<MicrosoftJavaAnswer> {
    Prompt::new(
        "Microsoft Java runtime",
        format!(
            "OneClient now installs Java from the Microsoft Build of OpenJDK, and none of your \
             recorded runtimes come from it. \nDownload Microsoft Java {MICROSOFT_JAVA_MAJOR} now? \
             It is about 200 MB, and your existing runtimes are kept exactly as they are."
        ),
    )
    .option(
        Choice::new(MICROSOFT_JAVA_CHOICE_NEVER, "Don't ask again"),
        MicrosoftJavaAnswer::Never,
    )
    .option(
        Choice::primary(MICROSOFT_JAVA_CHOICE_INSTALL, "Proceed"),
        MicrosoftJavaAnswer::Install,
    )
    .dismiss("Cancel")
}

pub fn spawn_offer(actions: &Actions) {
    let actions = actions.clone();
    spawn_forever(async move { run(actions).await });
}

async fn run(actions: Actions) {
    let Ok(state) = launcher::state() else {
        return;
    };

    // Read before the wait below so a first run finishes onboarding without this landing on top of it
    {
        let settings = state.settings.read();
        if settings.skip_microsoft_java_prompt || !settings.seen_onboarding {
            return;
        }
    }

    match state.java.has_vendor_runtime(&JavaVendor::Microsoft).await {
        Ok(true) => return,
        Ok(false) => {}
        Err(err) => {
            tracing::warn!("could not check for a Microsoft Java runtime: {err:#}");
            return;
        }
    }

    if !wait_for_prompt_slot(&actions.station()).await {
        tracing::debug!("prompt slot stayed busy; offering the Microsoft runtime next launch");
        return;
    }

    match state
        .java
        .latest_package(&JavaVendor::Microsoft, MICROSOFT_JAVA_MAJOR)
        .await
    {
        Ok(Some(_)) => {}
        Ok(None) => {
            tracing::info!(
                major = MICROSOFT_JAVA_MAJOR,
                "Microsoft publishes no build for this host; not offering one"
            );
            return;
        }
        Err(err) => {
            tracing::warn!("could not reach Microsoft's downloads: {err:#}");
            return;
        }
    }

    match actions.events().ask(offer_prompt()).await {
        Ok(Some(chosen)) => match chosen.value {
            MicrosoftJavaAnswer::Install => {
                actions.install_java_runtime(JavaVendor::Microsoft, MICROSOFT_JAVA_MAJOR);
            }
            MicrosoftJavaAnswer::Never => actions.skip_microsoft_java_prompt(),
        },
        Ok(None) => tracing::info!("Microsoft Java offer dismissed"),
        Err(err) => tracing::warn!("Microsoft Java offer failed: {err:#}"),
    }
}

/// Waits for an open slot on main screen. For a minute (60sec)
async fn wait_for_prompt_slot(station: &RadioStation<AppState, AppChannel>) -> bool {
    for _ in 0..SLOT_ATTEMPTS {
        {
            let state = station.peek();
            if !state.launcher.fetching && state.prompt.is_none() {
                return true;
            }
        }

        tokio::time::sleep(SLOT_POLL).await;
    }

    false
}
