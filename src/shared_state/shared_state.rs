// SPDX-License-Identifier: GPL-3.0-only

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use sctk::output::OutputStatusListener;
use sctk::reexports::client::{
    self,
    protocol::{wl_keyboard, wl_output::WlOutput as c_WlOutput, wl_pointer, wl_surface},
    GlobalManager,
};
use sctk::seat::SeatListener;
use slog::Logger;
use smithay::{
    reexports::{
        calloop,
        wayland_server::{
            self,
            protocol::{
                wl_output::WlOutput as s_WlOutput, wl_pointer::AxisSource, wl_seat::WlSeat,
                wl_surface::WlSurface,
            },
            Global,
        },
    },
    wayland::{output::Output as s_Output, seat, shell::xdg::ShellState},
};

use crate::{CachedBuffers, Surface};

#[derive(Debug)]
pub struct Seat {
    pub name: String,
    pub client: ClientSeat,
    pub server: (seat::Seat, Global<WlSeat>),
}

#[derive(Debug)]
pub struct ClientSeat {
    pub kbd: Option<wl_keyboard::WlKeyboard>,
    pub ptr: Option<wl_pointer::WlPointer>,
}

pub type OutputGroup = (s_Output, Global<s_WlOutput>, u32, c_WlOutput);

#[derive(Debug, Default)]
pub struct AxisFrameData {
    pub frame: Option<seat::AxisFrame>,
    pub source: Option<AxisSource>,
    pub h_discrete: Option<i32>,
    pub v_discrete: Option<i32>,
}

#[derive(Debug)]
pub struct GlobalState {
    pub desktop_client_state: DesktopClientState,
    pub embedded_server_state: EmbeddedServerState,
    pub loop_signal: calloop::LoopSignal,
    pub outputs: Vec<OutputGroup>,
    pub log: Logger,
    pub start_time: std::time::Instant,
    pub cached_buffers: CachedBuffers,
}

#[derive(Debug)]
pub struct EmbeddedServerState {
    pub client: wayland_server::Client,
    pub shell_state: Arc<Mutex<ShellState>>,
    pub focused_surface: Option<WlSurface>,
}

#[derive(Debug)]
pub struct DesktopClientState {
    pub display: client::Display,
    pub seats: Vec<Seat>,
    pub seat_listener: SeatListener,
    pub output_listener: OutputStatusListener,
    pub surface: Rc<RefCell<Option<(u32, Surface)>>>,
    pub cursor_surface: wl_surface::WlSurface,
    pub axis_frame: AxisFrameData,
    pub kbd_focus: bool,
    pub globals: GlobalManager,
}
