use cstk::{
    delegate_toplevel_info, delegate_toplevel_management,
    toplevel_info::{ToplevelInfoHandler, ToplevelInfoState},
    toplevel_management::ToplevelManagementHandler,
};

use crate::{shared_state::GlobalState, space::WrapperSpace};

impl<W: WrapperSpace + 'static> ToplevelInfoHandler for GlobalState<W> {
    fn toplevel_info_state(&self) -> &ToplevelInfoState<Self> {
        &self.server_state.toplevel_info_state
    }

    fn toplevel_info_state_mut(&mut self) -> &mut ToplevelInfoState<Self> {
        &mut self.server_state.toplevel_info_state
    }
}

impl<W: WrapperSpace + 'static> ToplevelManagementHandler for GlobalState<W> {
    fn toplevel_management_state(
        &mut self,
    ) -> &mut cstk::toplevel_management::ToplevelManagementState {
        &mut self.server_state.toplevel_management_state
    }
}

delegate_toplevel_info!(@<W: WrapperSpace + 'static> GlobalState<W>);
delegate_toplevel_management!(@<W: WrapperSpace + 'static> GlobalState<W>);
