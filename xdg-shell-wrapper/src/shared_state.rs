// SPDX-License-Identifier: MPL-2.0-only

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use sctk::data_device::DataDeviceHandler;
use sctk::seat::SeatHandler;
use sctk::{
    environment::Environment,
    output::OutputStatusListener,
    reexports::{
        client::{
            self,
            protocol::{
                wl_keyboard as c_wl_keyboard, wl_output as c_wl_output, wl_pointer as c_wl_pointer, wl_shm as c_wl_shm,
                wl_surface as c_wl_surface, wl_seat as c_wl_seat
            },
            Attached, GlobalManager,
        },
        protocols::xdg_shell::client::xdg_wm_base::XdgWmBase,
    },
    seat::SeatListener,
};
use slog::Logger;
use smithay::{
    desktop::{PopupManager, Window},
    reexports::{
        calloop,
        wayland_server::{
            self,
            protocol::{
                wl_output, wl_pointer::AxisSource, wl_seat::WlSeat,
                wl_surface::WlSurface,
            },
            Global,
        },
    },
    wayland::{output::Output, seat, shell::xdg::ShellState},
};

use crate::render::WrapperRenderer;
use crate::{client::Env, CachedBuffers};

#[derive(Debug)]
pub struct Seat {
    pub(crate) name: String,
    pub(crate) client: ClientSeat,
    pub(crate) server: (seat::Seat, Global<WlSeat>),
}

#[derive(Debug)]
pub struct ClientSeat {
    pub(crate) seat: Attached<c_wl_seat::WlSeat>,
    pub(crate) kbd: Option<c_wl_keyboard::WlKeyboard>,
    pub(crate) ptr: Option<c_wl_pointer::WlPointer>,
}

pub type OutputGroup = (
    Output,
    Global<wl_output::WlOutput>,
    u32,
    c_wl_output::WlOutput,
);

#[derive(Debug, Default)]
pub struct AxisFrameData {
    pub frame: Option<seat::AxisFrame>,
    pub source: Option<AxisSource>,
    pub h_discrete: Option<i32>,
    pub v_discrete: Option<i32>,
}

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
    pub root_window: Option<Rc<RefCell<Window>>>,
    pub focused_surface: Option<WlSurface>,
    pub popup_manager: Rc<RefCell<PopupManager>>,
    pub(crate) selected_data_provider_seat: RefCell<Option<Attached<c_wl_seat::WlSeat>>>
}

pub struct DesktopClientState {
    pub display: client::Display,
    pub seats: Vec<Seat>,
    pub output_listener: OutputStatusListener,
    pub renderer: Option<WrapperRenderer>,
    pub cursor_surface: c_wl_surface::WlSurface,
    pub axis_frame: AxisFrameData,
    pub kbd_focus: bool,
    pub shm: Attached<c_wl_shm::WlShm>,
    pub xdg_wm_base: Attached<XdgWmBase>,
    pub env_handle: Environment<Env>,
    pub last_input_serial: Option<u32>,
    pub focused_surface: Option<c_wl_surface::WlSurface>,
}
