// SPDX-License-Identifier: GPL-3.0-only

use sctk::reexports::client::{
    self,
    protocol::{wl_keyboard, wl_pointer, wl_shm, wl_surface},
};
use sctk::seat::keyboard::{self, KeyState, ModifiersState};
use sctk::shm::AutoMemPool;
use sctk::window::{Event as WEvent, FallbackFrame, Window};
use smithay::reexports::{calloop, wayland_server};

pub type Seat = (
    Option<(wl_keyboard::WlKeyboard, calloop::RegistrationToken)>,
    Option<wl_pointer::WlPointer>,
);

#[derive(Debug)]
pub struct GlobalState {
    pub desktop_client_state: DesktopClientState,
    pub embedded_server_state: EmbeddedServerState,
    pub loop_signal: calloop::LoopSignal,
}

#[derive(Debug)]
pub struct DesktopClientState {
    pub next_wevent: Option<WEvent>,
    pub display: client::Display,
    pub seats: Vec<Seat>,
    pub window: Window<FallbackFrame>,
    pub dimensions: (u32, u32),
    pub pool: AutoMemPool,
}

#[derive(Debug)]
pub struct EmbeddedServerState {
    pub display: wayland_server::Display,
}
