use std::cell::RefMut;

use slog::trace;
use smithay::{wayland::{compositor::{CompositorHandler, CompositorState, get_role, with_states, SurfaceAttributes, BufferAssignment}, buffer::BufferHandler, shm::{ShmState, ShmHandler}, SERIAL_COUNTER}, reexports::wayland_server::{DisplayHandle, protocol::{wl_surface::WlSurface, wl_buffer}}, backend::renderer::{utils::on_commit_buffer_handler, BufferType, buffer_type}, delegate_compositor, delegate_shm};

use crate::{shared_state::GlobalState, space::WrapperSpace, server_state::SeatPair};

// let DesktopClientState {
//     cursor_surface,
//     space,
//     seats,
//     shm,
//     ..
// } = &mut state.desktop_client_state;
// let EmbeddedServerState {
//     popup_manager,
//     shell_state,
//     ..
// } = &mut state.embedded_server_state;
impl<W: WrapperSpace> CompositorHandler for GlobalState<W> {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.embedded_server_state.compositor_state
    }

    fn commit(&mut self, dh: &DisplayHandle, surface: &WlSurface) {
        let mut popup_manager = self.embedded_server_state.popup_manager.borrow_mut();
        let cached_buffers = &mut self.cached_buffers;
        let log = &mut self.log;

        let role = get_role(&surface);
        trace!(log, "role: {:?} surface: {:?}", &role, &surface);
        if role == "xdg_toplevel".into() {
            on_commit_buffer_handler(&dh, &surface);
        } else if role == "xdg_popup".into() {
            // println!("dirtying popup");
            let popup = popup_manager.find_popup(&surface);
            on_commit_buffer_handler(&dh, &surface);
            popup_manager.commit(&surface);
        } else if role == "cursor_image".into() {
            // pass cursor image to parent compositor
            trace!(log, "received surface with cursor image");
            for SeatPair { client, .. } in &self.embedded_server_state.seats {
                if let Some(ptr) = client.ptr.as_ref() {
                    trace!(log, "updating cursor for pointer {:?}", &ptr);
                    let _ = with_states(&surface, |data| {
                        let surface_attributes =
                            data.cached_state.current::<SurfaceAttributes>();
                        let buf = RefMut::map(surface_attributes, |s| &mut s.buffer);
                        if let Some(BufferAssignment::NewBuffer(buffer)) = buf.as_ref() {
                            if let Some(BufferType::Shm) = buffer_type(&dh, buffer) {
                                trace!(log, "attaching buffer to cursor surface.");
                                let _ = cached_buffers.write_and_attach_buffer(
                                    &dh,
                                    buf.as_ref().unwrap(),
                                    &self.desktop_client_state.cursor_surface,
                                    &self.desktop_client_state.shm,
                                );

                                trace!(log, "requesting update");
                                ptr.set_cursor(
                                    SERIAL_COUNTER.next_serial().into(),
                                    Some(&self.desktop_client_state.cursor_surface),
                                    0,
                                    0,
                                );
                            }
                        } else {
                            ptr.set_cursor(SERIAL_COUNTER.next_serial().into(), None, 0, 0);
                        }
                    });
                }
            }
        } else {
            trace!(log, "{:?}", surface);
        }
    }
}

impl<W: WrapperSpace> BufferHandler for GlobalState<W> {
    fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
}

impl<W: WrapperSpace> ShmHandler for GlobalState<W> {
    fn shm_state(&self) -> &ShmState {
        &self.embedded_server_state.shm_state
    }
}

delegate_compositor!(@<W: WrapperSpace + 'static> GlobalState<W>);
delegate_shm!(@<W: WrapperSpace + 'static> GlobalState<W>);
