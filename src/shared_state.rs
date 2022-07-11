// SPDX-License-Identifier: MPL-2.0-only

use std::cell::RefCell;
use std::rc::Rc;

use once_cell::sync::OnceCell;
use sctk::{
    environment::Environment,
    reexports::client::{
        Attached,
        protocol::{wl_output as c_wl_output, wl_seat as c_wl_seat},
    },
};
use slog::Logger;
use smithay::{
    backend::renderer::{ImportDma, ImportEgl},
    reexports::wayland_server::{backend::GlobalId, DisplayHandle},
    wayland::dmabuf::DmabufState,
};
use smithay::{
    reexports::{calloop, wayland_server::protocol::wl_pointer::AxisSource},
    wayland::{output::Output, seat},
};

use crate::CachedBuffers;
use crate::client_state::{DesktopClientState, Env};
use crate::server_state::EmbeddedServerState;
use crate::space::WrapperSpace;

pub type OutputGroup = (Output, GlobalId, String, c_wl_output::WlOutput);

#[derive(Debug, Default)]
pub struct AxisFrameData {
    pub(crate) frame: Option<seat::AxisFrame>,
    pub(crate) source: Option<AxisSource>,
    pub(crate) h_discrete: Option<i32>,
    pub(crate) v_discrete: Option<i32>,
}

#[derive(Debug)]
pub struct GlobalState<W: WrapperSpace + 'static> {
    pub space: W,
    pub(crate) desktop_client_state: DesktopClientState,
    pub(crate) embedded_server_state: EmbeddedServerState<W>,
    pub(crate) _loop_signal: calloop::LoopSignal,
    pub log: Logger,
    pub(crate) start_time: std::time::Instant,
    pub(crate) cached_buffers: CachedBuffers,
}

impl<W: WrapperSpace + 'static> GlobalState<W> {
    pub fn bind_display(&mut self, dh: &DisplayHandle) {
        if let Some(renderer) = self.space.renderer() {
            if renderer.bind_wl_display(dh).is_ok() {
                let dmabuf_formats = renderer.dmabuf_formats().cloned().collect::<Vec<_>>();
                let mut state = DmabufState::new();
                let global =
                    state.create_global::<GlobalState<W>, _>(dh, dmabuf_formats, self.log.clone());
                self.embedded_server_state
                    .dmabuf_state
                    .replace((state, global));
            }
        }
    }

    pub fn env_handle(&mut self) -> &Environment<Env> {
        &self.desktop_client_state.env_handle
    }
}

#[derive(Debug)]
pub struct SelectedDataProvider {
    pub(crate) seat: Rc<RefCell<Option<Attached<c_wl_seat::WlSeat>>>>,
    pub(crate) env_handle: Rc<OnceCell<Environment<Env>>>,
}
