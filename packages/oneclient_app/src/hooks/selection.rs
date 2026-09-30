use std::collections::HashSet;
use std::hash::Hash;

use freya::prelude::*;

#[derive(PartialEq)]
pub struct Selection<K: 'static> {
    keys: State<HashSet<K>>,
    anchor: State<Option<K>>,
    modifiers: State<Modifiers>,
    active: State<bool>,
}

impl<K> Clone for Selection<K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K> Copy for Selection<K> {}

pub fn use_selection<K: Clone + Eq + Hash + 'static>() -> Selection<K> {
    Selection {
        keys: use_state(HashSet::new),
        anchor: use_state(|| None),
        modifiers: use_state(Modifiers::empty),
        active: use_state(|| false),
    }
}

fn held_after(e: &KeyboardEventData, down: bool) -> Modifiers {
    let key = match e.code {
        Code::ShiftLeft | Code::ShiftRight => Modifiers::SHIFT,
        Code::ControlLeft | Code::ControlRight => Modifiers::CONTROL,
        Code::MetaLeft | Code::MetaRight => Modifiers::META,
        _ => Modifiers::empty(),
    };
    let mut held = e.modifiers;
    held.set(key, down);
    held
}

impl<K: Clone + Eq + Hash + 'static> Selection<K> {
    pub fn track_modifiers<E: EventHandlersExt>(self, el: E) -> E {
        let mut down = self.modifiers;
        let mut up = self.modifiers;
        el.on_global_key_down(move |e: Event<KeyboardEventData>| {
            down.set_if_modified(held_after(&e, true));
        })
        .on_global_key_up(move |e: Event<KeyboardEventData>| {
            up.set_if_modified(held_after(&e, false));
        })
    }

    pub fn modifier_held(&self) -> bool {
        self.modifiers
            .read()
            .intersects(Modifiers::SHIFT | Modifiers::CONTROL | Modifiers::META)
    }

    pub fn is_active(&self) -> bool {
        *self.active.read()
    }

    pub fn enter(mut self) {
        self.active.set_if_modified(true);
    }

    pub fn is_selected(&self, key: &K) -> bool {
        self.keys.read().contains(key)
    }

    pub fn selected_in(&self, order: &[K]) -> Vec<K> {
        let keys = self.keys.read();
        order.iter().filter(|k| keys.contains(k)).cloned().collect()
    }

    pub fn toggle(mut self, key: K) {
        if !self.keys.write().remove(&key) {
            self.keys.write().insert(key.clone());
        }
        self.anchor.set(Some(key));
        self.enter();
    }

    pub fn click(mut self, key: K, order: &[K]) {
        let anchor = self.anchor.peek().clone();
        let range = anchor
            .filter(|_| self.modifiers.peek().contains(Modifiers::SHIFT))
            .and_then(|anchor| order.iter().position(|k| *k == anchor))
            .zip(order.iter().position(|k| *k == key));

        let Some((from, to)) = range else {
            return self.toggle(key);
        };

        let select = !self.keys.peek().contains(&key);
        let mut keys = self.keys.write();
        for k in &order[from.min(to)..=from.max(to)] {
            if select {
                keys.insert(k.clone());
            } else {
                keys.remove(k);
            }
        }
        drop(keys);
        self.enter();
    }

    pub fn toggle_all(mut self, order: &[K]) {
        let all = order.iter().all(|k| self.keys.peek().contains(k));
        if all {
            self.clear();
        } else {
            self.keys.write().extend(order.iter().cloned());
        }
    }

    pub fn clear(mut self) {
        self.keys.write().clear();
        self.anchor.set(None);
    }

    pub fn exit(mut self) {
        self.clear();
        self.active.set_if_modified(false);
    }
}
