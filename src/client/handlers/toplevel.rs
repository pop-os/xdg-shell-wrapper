use cctk::sctk::registry::{ProvidesRegistryState, RegistryState};
use cctk::{
    sctk::{
        self,
        event_loop::WaylandSource,
        reexports::client::protocol::wl_seat::WlSeat,
        seat::{SeatHandler, SeatState},
    },
    toplevel_info::{ToplevelInfoHandler, ToplevelInfoState},
    toplevel_management::{ToplevelManagerHandler, ToplevelManagerState},
    wayland_client::{self, WEnum},
};
use cosmic_protocols::{
    toplevel_info::v1::client::zcosmic_toplevel_handle_v1,
    toplevel_management::v1::client::zcosmic_toplevel_manager_v1,
};
use wayland_client::{globals::registry_queue_init, Connection, QueueHandle};

use crate::{shared_state::GlobalState, space::WrapperSpace};

impl<W: WrapperSpace> ProvidesRegistryState for GlobalState<W> {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.client_state.registry_state
    }

    sctk::registry_handlers!();
}

impl<W: WrapperSpace> ToplevelManagerHandler for GlobalState<W> {
    fn toplevel_manager_state(&mut self) -> &mut cctk::toplevel_management::ToplevelManagerState {
        &mut self.client_state.toplevel_manager_state
    }

    fn capabilities(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: Vec<WEnum<zcosmic_toplevel_manager_v1::ZcosmicToplelevelManagementCapabilitiesV1>>,
    ) {
        // TODO capabilities could affect the options in the applet
    }
}

impl<W: WrapperSpace> ToplevelInfoHandler for GlobalState<W> {
    fn toplevel_info_state(&mut self) -> &mut ToplevelInfoState {
        &mut self.client_state.toplevel_info_state
    }

    fn new_toplevel(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        toplevel: &zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1,
    ) {
        todo!()
    }

    fn update_toplevel(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        toplevel: &zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1,
    ) {
        todo!()
    }

    fn toplevel_closed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        toplevel: &zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1,
    ) {
        todo!()
    }
}

cctk::delegate_toplevel_info!(@<W: WrapperSpace + 'static> GlobalState<W>);
cctk::delegate_toplevel_manager!(@<W: WrapperSpace + 'static> GlobalState<W>);
