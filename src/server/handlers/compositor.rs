use std::{cell::RefMut, sync::Mutex};

use slog::{error, trace};
use smithay::{
    backend::renderer::{buffer_type, utils::on_commit_buffer_handler, BufferType},
    delegate_compositor, delegate_shm,
    input::pointer::CursorImageAttributes,
    reexports::wayland_server::protocol::{wl_buffer, wl_surface::WlSurface},
    utils::SERIAL_COUNTER,
    wayland::{
        buffer::BufferHandler,
        compositor::{
            get_role, with_states, BufferAssignment, CompositorHandler, CompositorState,
            SurfaceAttributes,
        },
        shm::{ShmHandler, ShmState},
    },
};

use crate::{
    server_state::SeatPair, shared_state::GlobalState, space::WrapperSpace,
    util::write_and_attach_buffer,
};

impl<W: WrapperSpace> CompositorHandler for GlobalState<W> {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.server_state.compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        let dh = self.server_state.display_handle.clone();
        let log = &mut self.log;
        let role = get_role(surface);
        trace!(log, "role: {:?} surface: {:?}", &role, &surface);

        if role == "xdg_toplevel".into() {
            on_commit_buffer_handler(surface);
            self.space.dirty_window(&dh, surface)
        } else if role == "xdg_popup".into() {
            on_commit_buffer_handler(surface);
            self.server_state.popup_manager.commit(surface);
            self.space.dirty_popup(&dh, surface);
        } else if role == "cursor_image".into() {
            let multipool = match &mut self.client_state.multipool {
                Some(m) => m,
                None => {
                    error!(log.clone(), "multipool is missing!");
                    return;
                }
            };
            let cursor_surface = match &mut self.client_state.cursor_surface {
                Some(m) => m,
                None => {
                    error!(log.clone(), "cursor surface is missing!");
                    return;
                }
            };

            // FIXME pass cursor image to parent compositor
            trace!(log, "received surface with cursor image");
            for SeatPair { client, .. } in &self.server_state.seats {
                if let Some(ptr) = client.ptr.as_ref() {
                    trace!(log, "updating cursor for pointer {:?}", &ptr);
                    let _ = with_states(surface, |data| {
                        let surface_attributes = data.cached_state.current::<SurfaceAttributes>();
                        let buf = RefMut::map(surface_attributes, |s| &mut s.buffer);
                        if let Some(BufferAssignment::NewBuffer(buffer)) = buf.as_ref() {
                            if let Some(BufferType::Shm) = buffer_type(buffer) {
                                trace!(log, "attaching buffer to cursor surface.");
                                let _ = write_and_attach_buffer::<W>(
                                    buf.as_ref().unwrap(),
                                    cursor_surface,
                                    multipool,
                                );

                                if let Some(hotspot) = data
                                    .data_map
                                    .get::<Mutex<CursorImageAttributes>>()
                                    .and_then(|m| m.lock().ok())
                                    .map(|attr| (*attr).hotspot)
                                {
                                    trace!(log, "requesting update");
                                    ptr.set_cursor(
                                        SERIAL_COUNTER.next_serial().into(),
                                        Some(cursor_surface),
                                        hotspot.x,
                                        hotspot.y,
                                    );
                                }
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
        &self.server_state.shm_state
    }
}

delegate_compositor!(@<W: WrapperSpace + 'static> GlobalState<W>);
delegate_shm!(@<W: WrapperSpace + 'static> GlobalState<W>);
