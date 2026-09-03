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
        self.0.set(self.0.get().saturating_sub(1));
    }
}

pub fn use_provide_overlay_claims() {
    use_provide_root_context(OverlayClaims::default);
}

pub fn use_overlay_claims() -> OverlayClaims {
    consume_root_context::<OverlayClaims>()
}

pub fn use_overlay_claim() {
    let claims = use_overlay_claims();
    let released = claims.clone();

    use_hook(move || claims.acquire());
    use_drop(move || released.release());
}

pub fn use_overlay_claim_when(active: bool) {
    let claims = use_overlay_claims();
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
