use std::time::Duration;

use freya::prelude::*;

/// Returns a debounced mirror of `value`. The returned state only updates to a
/// new `value` once `delay` has elapsed without any further change. Rapid
/// changes (e.g. per-keystroke) collapse into a single trailing update.
pub fn use_debounced<T>(value: T, delay: Duration) -> State<T>
where
    T: Clone + PartialEq + 'static,
{
    let debounced = use_state(|| value.clone());
    // Bumped on every change; the async task commits only if its generation is
    // still the latest when the delay expires, dropping superseded changes.
    let mut generation = use_state(|| 0u64);

    use_side_effect_with_deps(&value, move |value| {
        let value = value.clone();
        let this_gen = *generation.peek() + 1;
        generation.set(this_gen);
        let mut debounced = debounced;
        spawn(async move {
            tokio::time::sleep(delay).await;
            if *generation.peek() == this_gen {
                debounced.set(value);
            }
        });
    });

    debounced
}
