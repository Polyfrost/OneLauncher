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

/// Read from the setting rather than the clock so a component re-renders when the preference is
/// toggled, and so it never reads the one frame of stale speed before the driver above applies it
pub fn use_animations_enabled() -> bool {
    use_settings_snapshot().settings.animations_enabled
}
