use freya::prelude::*;

use crate::hooks::use_settings_snapshot;

#[derive(PartialEq)]
pub struct AnimationClockDriver;

impl Component for AnimationClockDriver {
    fn render(&self) -> impl IntoElement {
        let enabled = use_settings_snapshot().settings.animations_enabled;
        let clock = AnimationClock::get();

        use_side_effect_with_deps(&enabled, move |&enabled| {
            if enabled {
                clock.enable();
            } else {
                clock.disable();
            }
        });

        rect().into_element()
    }
}

pub fn animations_enabled() -> bool {
    AnimationClock::get().speed() <= AnimationClock::MAX_SPEED
}
