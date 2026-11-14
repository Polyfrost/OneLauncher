use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use freya::prelude::*;

/// Trailing-edge debounce updates only after `delay` with no further change
pub fn use_debounced<T>(value: T, delay: Duration) -> State<T>
where
    T: Clone + PartialEq + 'static,
{
    let debounced = use_state(|| value.clone());
    // Task commits only if its generation is still latest dropping superseded changes
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

pub fn use_debounced_save<T>(value: T, delay: Duration, save: impl Fn(T) + 'static)
where
    T: Clone + PartialEq + 'static,
{
    let pending = use_hook(|| Rc::new(Cell::new(0u64)));
    let first = use_hook(|| Rc::new(Cell::new(true)));
    let save = use_hook(|| Rc::new(save));
    use_side_effect_with_deps(&value, move |value| {
        if first.replace(false) {
            return;
        }
        let this = pending.get() + 1;
        pending.set(this);
        let (pending, save, value) = (pending.clone(), save.clone(), value.clone());
        spawn_in_window(async move {
            tokio::time::sleep(delay).await;
            if pending.get() == this {
                save(value);
            }
        });
    });
}
