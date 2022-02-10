// SPDX-License-Identifier: GPL-3.0-only

use crate::client::DesktopClientState;
use sctk::reexports::client::protocol::{wl_keyboard, wl_pointer};
use slog::Logger;
use smithay::reexports::{calloop, wayland_server};
use smithay::wayland::shell::xdg::ShellState;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct Seat {
    pub name: String,
    pub kbd: Option<(wl_keyboard::WlKeyboard, calloop::RegistrationToken)>,
    pub ptr: Option<wl_pointer::WlPointer>,
}

#[derive(Debug)]
pub struct GlobalState {
    pub desktop_client_state: DesktopClientState,
    pub embedded_server_state: EmbeddedServerState,
    pub loop_signal: calloop::LoopSignal,
    pub log: Logger,
}

#[derive(Debug)]
pub struct EmbeddedServerState {
    pub display: wayland_server::Display,
    pub client: wayland_server::Client,
    pub shell_state: Arc<Mutex<ShellState>>,
}
