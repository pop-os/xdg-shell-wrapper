use slog::error;
use smithay::{
    delegate_xdg_shell,
    desktop::{Kind, PopupKind, Window},
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::{
            protocol::{wl_seat, wl_surface::WlSurface},
            DisplayHandle, Resource,
        },
    },
    wayland::{
        seat::{PointerGrabStartData, Seat},
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
        &mut self.embedded_server_state.xdg_shell_state
    }

    fn new_toplevel(&mut self, dh: &DisplayHandle, surface: ToplevelSurface) {
        let window = Window::new(Kind::Xdg(surface.clone()));
        // window.refresh();

        let wl_surface = surface.wl_surface();
        if self.desktop_client_state.kbd_focus {
            for s in &self.embedded_server_state.seats {
                if let Some(kbd) = s.server.get_keyboard() {
                    kbd.set_focus(dh, Some(wl_surface), SERIAL_COUNTER.next_serial());
                }
            }
        }

        self.space.add_window(window);
        surface.send_configure();
    }

    fn new_popup(
        &mut self,
        _dh: &DisplayHandle,
        surface: PopupSurface,
        positioner_state: PositionerState,
    ) {
        let positioner = self.desktop_client_state.xdg_wm_base.create_positioner();

        // let wl_surface = self.desktop_client_state.env_handle.create_surface().detach();
        // let xdg_surface = self.desktop_client_state.xdg_wm_base.get_xdg_surface(&wl_surface);

        self.space.add_popup(
            &self.desktop_client_state.env_handle,
            &self.desktop_client_state.xdg_wm_base,
            surface,
            positioner,
            positioner_state,
        );
    }

    fn move_request(
        &mut self,
        _dh: &DisplayHandle,
        surface: ToplevelSurface,
        seat: wl_seat::WlSeat,
        serial: Serial,
    ) {
    }

    fn resize_request(
        &mut self,
        _dh: &DisplayHandle,
        surface: ToplevelSurface,
        seat: wl_seat::WlSeat,
        serial: Serial,
        edges: xdg_toplevel::ResizeEdge,
    ) {
    }

    fn grab(
        &mut self,
        dh: &DisplayHandle,
        surface: PopupSurface,
        seat: wl_seat::WlSeat,
        _serial: Serial,
    ) {
        if self.desktop_client_state.kbd_focus {
            for s in &self.embedded_server_state.seats {
                if s.server.owns(&seat) {
                    if let Err(e) = self.space.popup_manager().grab_popup(
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
    }

    fn reposition_request(
        &mut self,
        dh: &smithay::reexports::wayland_server::DisplayHandle,
        surface: PopupSurface,
        positioner: PositionerState,
        token: u32,
    ) {
        let new_positioner = self.desktop_client_state.xdg_wm_base.create_positioner();

        let _ = self
            .space
            .reposition_popup(surface, new_positioner, positioner, token);
    }
}

// Xdg Shell
delegate_xdg_shell!(@<W: WrapperSpace + 'static> GlobalState<W>);

fn check_grab<W: WrapperSpace>(
    seat: &Seat<GlobalState<W>>,
    surface: &WlSurface,
    serial: Serial,
) -> Option<PointerGrabStartData> {
    let pointer = seat.get_pointer()?;

    // Check that this surface has a click grab.
    if !pointer.has_grab(serial) {
        return None;
    }

    let start_data = pointer.grab_start_data()?;

    let (focus, _) = start_data.focus.as_ref()?;
    // If the focus was for a different surface, ignore the request.
    if !focus.id().same_client_as(&surface.id()) {
        return None;
    }

    Some(start_data)
}
