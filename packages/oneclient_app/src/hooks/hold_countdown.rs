use std::time::{Duration, Instant};

use freya::prelude::*;

pub fn use_hold_countdown<K>(seconds: u8, key: Option<K>) -> u8
where
    K: Clone + PartialEq + 'static,
{
    let remaining = use_state(|| seconds);
    let started = use_state(|| None::<Instant>);

    use_side_effect_with_deps(&key, move |key| {
        let mut remaining = remaining;
        let mut started = started;
        remaining.set(seconds);

        if key.is_none() {
            started.set(None);
            return;
        }

        let start = Instant::now();
        started.set(Some(start));

        spawn(async move {
            let mut remaining = remaining;
            for step in (0..seconds).rev() {
                let deadline = start + Duration::from_secs(u64::from(seconds - step));
                tokio::time::sleep_until(deadline.into()).await;

                if *started.peek() != Some(start) {
                    return;
                }

                remaining.set(step);
            }
        });
    });

    *remaining.read()
}
