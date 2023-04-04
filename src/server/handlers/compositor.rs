use std::{cell::RefMut, sync::Mutex};

use sctk::shell::WaylandSurface;
use smithay::{
    backend::renderer::{
        buffer_type, damage::OutputDamageTracker, utils::on_commit_buffer_handler, BufferType,
    },
    delegate_compositor, delegate_shm,
    input::pointer::CursorImageAttributes,
    reexports::wayland_server::protocol::{wl_buffer, wl_surface::WlSurface},
    utils::{Transform, SERIAL_COUNTER},
    wayland::{
        buffer::BufferHandler,
        compositor::{
            get_role, with_states, BufferAssignment, CompositorHandler, CompositorState,
            SurfaceAttributes,
        },
        shm::{ShmHandler, ShmState},
    },
};
use tracing::{error, trace};

use crate::{
    client_state::SurfaceState, server_state::SeatPair, shared_state::GlobalState,
    space::WrapperSpace, util::write_and_attach_buffer,
};

impl<W: WrapperSpace> CompositorHandler for GlobalState<W> {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.server_state.compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        let dh = self.server_state.display_handle.clone();
        let role = get_role(surface);
        trace!("role: {:?} surface: {:?}", &role, &surface);

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
                    error!("multipool is missing!");
                    return;
                }
            };
            let cursor_surface = match &mut self.client_state.cursor_surface {
                Some(m) => m,
                None => {
                    error!("cursor surface is missing!");
                    return;
                }
            };

            // FIXME pass cursor image to parent compositor
            trace!("received surface with cursor image");
            for SeatPair { client, .. } in &self.server_state.seats {
                if let Some(ptr) = client.ptr.as_ref() {
                    trace!("updating cursor for pointer {:?}", &ptr);
                    let _ = with_states(surface, |data| {
                        let surface_attributes = data.cached_state.current::<SurfaceAttributes>();
                        let buf = RefMut::map(surface_attributes, |s| &mut s.buffer);
                        if let Some(BufferAssignment::NewBuffer(buffer)) = buf.as_ref() {
                            if let Some(BufferType::Shm) = buffer_type(buffer) {
                                trace!("attaching buffer to cursor surface.");
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
                                    trace!("requesting update");
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
        } else if role == "zwlr_layer_surface_v1".into() {
            if let Some((egl_surface, renderer, s_layer_surface, c_layer_surface, state)) = self
                .client_state
                .proxied_layer_surfaces
                .iter_mut()
                .find(|s| s.2.wl_surface() == surface)
            {
                let old_size = s_layer_surface.bbox().size;
                on_commit_buffer_handler(surface);

                // s_layer_surface.layer_surface().ensure_configured();
                let size = s_layer_surface.bbox().size;
                if size.w <= 0 || size.h <= 0 {
                    return;
                }
                match state {
                    SurfaceState::WaitingFirst => {
                        return;
                    }
                    _ => {}
                };
                *state = SurfaceState::Dirty;
                if old_size != size {
                    egl_surface.resize(size.w, size.h, 0, 0);
                    c_layer_surface.set_size(size.w as u32, size.h as u32);
                    *renderer = OutputDamageTracker::new(
                        (size.w.max(1), size.h.max(1)),
                        1.0,
                        Transform::Flipped180,
                    );
                    c_layer_surface.wl_surface().commit();
                }
            }
        } else {
            trace!("{:?}", surface);
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
