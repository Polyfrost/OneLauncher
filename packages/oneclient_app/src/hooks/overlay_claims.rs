use std::cell::Cell;
use std::rc::Rc;

use freya::prelude::*;

#[derive(Clone, Default)]
pub struct OverlayClaims(Rc<Cell<usize>>);

impl OverlayClaims {
    /// Whether anything on screen currently owns Escape and mouse-back.
    pub fn any(&self) -> bool {
        self.0.get() > 0
    }

    fn acquire(&self) {
        self.0.set(self.0.get() + 1);
    }

    fn release(&self) {
        let held = self.0.get();
        debug_assert!(held > 0, "overlay claim released without a matching acquire");
        self.0.set(held.saturating_sub(1));
    }
}

pub fn use_provide_overlay_claims() {
    use_provide_root_context(OverlayClaims::default);
}

/// Falls back to a detached counter outside `RootLayout` so a claim raised there
/// degrades to "nothing claimed" instead of taking the app down.
pub fn use_overlay_claims() -> OverlayClaims {
    try_consume_root_context::<OverlayClaims>().unwrap_or_default()
}

pub fn use_overlay_claim() {
    let claims = use_hook(use_overlay_claims);
    let released = claims.clone();

    use_hook(move || claims.acquire());
    use_drop(move || released.release());
}

pub fn use_overlay_claim_when(active: bool) {
    let claims = use_hook(use_overlay_claims);
    let held = use_hook(|| Rc::new(Cell::new(false)));

    if held.get() != active {
        held.set(active);
        if active {
            claims.acquire();
        } else {
            claims.release();
        }
    }

    let released = claims;
    let held_at_drop = held;
    use_drop(move || {
        if held_at_drop.get() {
            released.release();
        }
    });
}
