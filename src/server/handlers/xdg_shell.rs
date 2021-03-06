use slog::error;
use smithay::{
    delegate_xdg_shell,
    desktop::{Kind, PopupKind, Window},
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::{protocol::wl_seat, DisplayHandle},
    },
    wayland::{
        shell::xdg::{
            PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
        },
        Serial, SERIAL_COUNTER,
    },
};

use crate::{shared_state::GlobalState, space::WrapperSpace};

// let DesktopClientState {
//     seats,
//     kbd_focus,
//     env_handle,
//     space,
//     xdg_wm_base,
//     ..
// } = &mut state.desktop_client_state;

// let EmbeddedServerState {
//     focused_surface,
//     popup_manager,
//     root_window,
//     ..
// } = &mut state.embedded_server_state;
impl<W: WrapperSpace> XdgShellHandler for GlobalState<W> {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.server_state.xdg_shell_state
    }

    fn new_toplevel(&mut self, _dh: &DisplayHandle, surface: ToplevelSurface) {
        let window = Window::new(Kind::Xdg(surface.clone()));
        // window.refresh();

        // TODO move to space implementation
        // let wl_surface = surface.wl_surface();
        // if self.desktop_client_state.kbd_focus {
        //     for s in &self.embedded_server_state.seats {
        //         if let Some(kbd) = s.server.get_keyboard() {
        //             kbd.set_focus(dh, Some(wl_surface), SERIAL_COUNTER.next_serial());
        //         }
        //     }
        // }

        self.space.add_window(window);
        surface.send_configure();
    }

    fn new_popup(
        &mut self,
        _dh: &DisplayHandle,
        surface: PopupSurface,
        positioner_state: PositionerState,
    ) {
        let positioner = self.client_state.xdg_wm_base.create_positioner();

        // let wl_surface = self.desktop_client_state.env_handle.create_surface().detach();
        // let xdg_surface = self.desktop_client_state.xdg_wm_base.get_xdg_surface(&wl_surface);

        self.space.add_popup(
            &self.client_state.env_handle,
            &self.client_state.xdg_wm_base,
            surface.clone(),
            positioner,
            positioner_state,
        );
        self.server_state
            .popup_manager
            .track_popup(PopupKind::Xdg(surface.clone()))
            .unwrap();
        self.server_state.popup_manager.commit(surface.wl_surface());
    }

    fn move_request(
        &mut self,
        _dh: &DisplayHandle,
        _surface: ToplevelSurface,
        _seat: wl_seat::WlSeat,
        _serial: Serial,
    ) {
    }

    fn resize_request(
        &mut self,
        _dh: &DisplayHandle,
        _surface: ToplevelSurface,
        _seat: wl_seat::WlSeat,
        _serial: Serial,
        _edges: xdg_toplevel::ResizeEdge,
    ) {
    }

    fn grab(
        &mut self,
        dh: &DisplayHandle,
        surface: PopupSurface,
        seat: wl_seat::WlSeat,
        _serial: Serial,
    ) {
        for s in &self.server_state.seats {
            if s.server.owns(&seat) {
                if let Err(e) = self.server_state.popup_manager.grab_popup(
                    dh,
                    PopupKind::Xdg(surface),
                    &s.server,
                    SERIAL_COUNTER.next_serial(),
                ) {
                    error!(self.log.clone(), "{}", e);
                }
                break;
            }
        }
    }

    fn reposition_request(
        &mut self,
        _dh: &smithay::reexports::wayland_server::DisplayHandle,
        surface: PopupSurface,
        positioner: PositionerState,
        token: u32,
    ) {
        let new_positioner = self.client_state.xdg_wm_base.create_positioner();

        let _ = self
            .space
            .reposition_popup(surface.clone(), new_positioner, positioner, token);
        self.server_state.popup_manager.commit(surface.wl_surface());
    }
}

// Xdg Shell
delegate_xdg_shell!(@<W: WrapperSpace + 'static> GlobalState<W>);
