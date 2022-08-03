use sctk::{
    delegate_xdg_popup, delegate_xdg_shell,
    shell::xdg::{popup::PopupHandler, XdgShellHandler, XdgShellState},
};

use crate::{shared_state::GlobalState, space::WrapperSpace};

impl<W: WrapperSpace> XdgShellHandler for GlobalState<W> {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.client_state.xdg_shell_state
    }
}

impl<W: WrapperSpace> PopupHandler for GlobalState<W> {
    fn configure(
        &mut self,
        conn: &sctk::reexports::client::Connection,
        qh: &sctk::reexports::client::QueueHandle<Self>,
        popup: &sctk::shell::xdg::popup::Popup,
        config: sctk::shell::xdg::popup::PopupConfigure,
    ) {
        todo!()
    }

    fn done(
        &mut self,
        conn: &sctk::reexports::client::Connection,
        qh: &sctk::reexports::client::QueueHandle<Self>,
        popup: &sctk::shell::xdg::popup::Popup,
    ) {
        todo!()
    }
}

delegate_xdg_shell!(@<W: WrapperSpace + 'static> GlobalState<W>);
delegate_xdg_popup!(@<W: WrapperSpace + 'static> GlobalState<W>);
