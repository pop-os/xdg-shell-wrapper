// SPDX-License-Identifier: MPL-2.0

use itertools::Itertools;
use sctk::reexports::client::protocol::wl_output as c_wl_output;
use smithay::{
    backend::renderer::{ImportDma, ImportEgl},
    output::Output,
    reexports::wayland_server::{backend::GlobalId, DisplayHandle},
    wayland::dmabuf::DmabufState,
};
use tracing::error;

use crate::client_state::ClientState;
use crate::server_state::ServerState;
use crate::space::WrapperSpace;

/// group of info for an output
pub type OutputGroup = (Output, GlobalId, String, c_wl_output::WlOutput);

/// the  global state for the embedded server state
#[allow(missing_debug_implementations)]
pub struct GlobalState<W: WrapperSpace + 'static> {
    /// the implemented space
    pub space: W,
    /// desktop client state
    pub client_state: ClientState<W>,
    /// embedded server state
    pub server_state: ServerState<W>,
    /// instant that the panel was started
    pub start_time: std::time::Instant,
}

impl<W: WrapperSpace + 'static> GlobalState<W> {
    pub(crate) fn new(
        client_state: ClientState<W>,
        server_state: ServerState<W>,
        space: W,
        start_time: std::time::Instant,
    ) -> Self {
        Self {
            space,
            client_state,
            server_state,
            start_time,
        }
    }
}

impl<W: WrapperSpace + 'static> GlobalState<W> {
    /// bind the display for the space
    pub fn bind_display(&mut self, dh: &DisplayHandle) {
        if let Some(renderer) = self.space.renderer() {
            let res = renderer.bind_wl_display(dh);
            if let Err(err) = res {
                error!("{:?}", err);
            } else {
                let dmabuf_formats = renderer.dmabuf_formats().into_iter().collect_vec();
                let mut state = DmabufState::new();
                let global = state.create_global::<GlobalState<W>>(dh, dmabuf_formats);
                self.server_state.dmabuf_state.replace((state, global));
            }
        }
    }
}

// TODO
#[derive(Debug)]
pub(crate) struct SelectedDataProvider {
    //     pub(crate) _seat: Rc<RefCell<Option<Attached<c_wl_seat::WlSeat>>>>,
    //     pub(crate) env_handle: Rc<OnceCell<Environment<Env>>>,
}
