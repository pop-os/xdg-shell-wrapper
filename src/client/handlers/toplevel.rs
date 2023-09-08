use cctk::cosmic_protocols::{
    toplevel_info::v1::client::zcosmic_toplevel_handle_v1,
    toplevel_management::v1::client::zcosmic_toplevel_manager_v1,
};
use cctk::{
    toplevel_info::{ToplevelInfoHandler, ToplevelInfoState},
    toplevel_management::ToplevelManagerHandler,
    wayland_client::{self, WEnum},
};
use wayland_client::{Connection, QueueHandle};

use crate::space::{ToplevelInfoSpace, ToplevelManagerSpace};
use crate::{shared_state::GlobalState, space::WrapperSpace};

impl<W: WrapperSpace + ToplevelManagerSpace> ToplevelManagerHandler for GlobalState<W> {
    fn toplevel_manager_state(&mut self) -> &mut cctk::toplevel_management::ToplevelManagerState {
        self.client_state.toplevel_manager_state.as_mut().unwrap()
    }

    fn capabilities(
        &mut self,
        conn: &Connection,
        _: &QueueHandle<Self>,
        capabilities: Vec<
            WEnum<zcosmic_toplevel_manager_v1::ZcosmicToplelevelManagementCapabilitiesV1>,
        >,
    ) {
        self.space.capabilities(conn, capabilities);
    }
}

impl<W: WrapperSpace + ToplevelInfoSpace> ToplevelInfoHandler for GlobalState<W> {
    fn toplevel_info_state(&mut self) -> &mut ToplevelInfoState {
        self.client_state.toplevel_info_state.as_mut().unwrap()
    }

    fn new_toplevel(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        toplevel: &zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1,
    ) {
        let toplevel_state = if let Some(s) = self.client_state.toplevel_info_state.as_mut() {
            s
        } else {
            return;
        };
        let info = if let Some(info) = toplevel_state.info(toplevel) {
            info
        } else {
            return;
        };
        self.space.new_toplevel(_conn, toplevel, info);
    }

    fn update_toplevel(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        toplevel: &zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1,
    ) {
        let toplevel_state = if let Some(s) = self.client_state.toplevel_info_state.as_mut() {
            s
        } else {
            return;
        };
        let info = if let Some(info) = toplevel_state.info(toplevel) {
            info
        } else {
            return;
        };
        self.space.update_toplevel(_conn, toplevel, info);
    }

    fn toplevel_closed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        toplevel: &zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1,
    ) {
        self.space.toplevel_closed(_conn, toplevel);
    }
}

cctk::delegate_toplevel_info!(@<W: WrapperSpace + ToplevelInfoSpace + 'static> GlobalState<W>);
cctk::delegate_toplevel_manager!(@<W: WrapperSpace + ToplevelManagerSpace + 'static> GlobalState<W>);
