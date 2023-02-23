use sctk::{
    delegate_xdg_popup, delegate_xdg_shell, delegate_xdg_window,
    shell::xdg::{popup::PopupHandler, window::WindowHandler},
};

use crate::{shared_state::GlobalState, space::WrapperSpace};

impl<W: WrapperSpace> PopupHandler for GlobalState<W> {
    fn configure(
        &mut self,
        _conn: &sctk::reexports::client::Connection,
        _qh: &sctk::reexports::client::QueueHandle<Self>,
        popup: &sctk::shell::xdg::popup::Popup,
        config: sctk::shell::xdg::popup::PopupConfigure,
    ) {
        self.space.configure_popup(popup, config);
    }

    fn done(
        &mut self,
        _conn: &sctk::reexports::client::Connection,
        _qh: &sctk::reexports::client::QueueHandle<Self>,
        popup: &sctk::shell::xdg::popup::Popup,
    ) {
        self.space.close_popup(popup)
    }
}

impl<W: WrapperSpace> WindowHandler for GlobalState<W> {
    fn request_close(
        &mut self,
        _conn: &sctk::reexports::client::Connection,
        _qh: &sctk::reexports::client::QueueHandle<Self>,
        _window: &sctk::shell::xdg::window::Window,
    ) {
        // nothing to be done
    }

    fn configure(
        &mut self,
        _conn: &sctk::reexports::client::Connection,
        _qh: &sctk::reexports::client::QueueHandle<Self>,
        _window: &sctk::shell::xdg::window::Window,
        _configure: sctk::shell::xdg::window::WindowConfigure,
        _serial: u32,
    ) {
        // nothing to be done
    }
}

delegate_xdg_window!(@<W: WrapperSpace + 'static> GlobalState<W>);
delegate_xdg_shell!(@<W: WrapperSpace + 'static> GlobalState<W>);
delegate_xdg_popup!(@<W: WrapperSpace + 'static> GlobalState<W>);
