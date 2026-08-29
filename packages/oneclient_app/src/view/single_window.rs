use std::sync::{Mutex, MutexGuard};

use freya::prelude::RendererContext;
use freya::winit::window::WindowId;

enum WindowState {
    Closed,
    Opening,
    Open(WindowId),
}

pub enum Claim {
    Focus(WindowId),
    Launch(OpeningGuard),
    Busy,
}

pub struct SingleWindow(Mutex<WindowState>);

impl SingleWindow {
    pub const fn new() -> Self {
        Self(Mutex::new(WindowState::Closed))
    }

    fn state(&self) -> MutexGuard<'_, WindowState> {
        self.0.lock().unwrap_or_else(|err| err.into_inner())
    }

    pub fn claim(&'static self) -> Claim {
        let mut state = self.state();

        match *state {
            WindowState::Open(id) => Claim::Focus(id),
            WindowState::Opening => Claim::Busy,
            WindowState::Closed => {
                *state = WindowState::Opening;
                Claim::Launch(OpeningGuard(self))
            }
        }
    }

    pub fn opened(&self, id: WindowId) {
        *self.state() = WindowState::Open(id);
    }

    pub fn forget(&self) {
        *self.state() = WindowState::Closed;
    }

    pub fn take(&self) -> Option<WindowId> {
        let mut state = self.state();

        match *state {
            WindowState::Open(id) => {
                *state = WindowState::Closed;
                Some(id)
            }
            _ => None,
        }
    }

    pub fn is_open(&self) -> bool {
        matches!(*self.state(), WindowState::Open(_))
    }

    pub fn close_in(&self, ctx: &mut RendererContext<'_>) {
        if let Some(id) = self.take() {
            ctx.windows.remove(&id);
        }
    }
}

pub struct OpeningGuard(&'static SingleWindow);

impl Drop for OpeningGuard {
    fn drop(&mut self) {
        let mut state = self.0.state();

        if matches!(*state, WindowState::Opening) {
            *state = WindowState::Closed;
        }
    }
}
