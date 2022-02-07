// SPDX-License-Identifier: GPL-3.0-only

use crate::client::DesktopClientState;
use sctk::reexports::client::protocol::{wl_keyboard, wl_pointer};
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
pub struct EmbeddedServerState {
    pub display: wayland_server::Display,
    pub client: wayland_server::Client,
}
