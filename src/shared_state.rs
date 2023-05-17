// SPDX-License-Identifier: MPL-2.0

use std::time::Duration;

use itertools::Itertools;
use sctk::reexports::client::protocol::wl_output as c_wl_output;
use smithay::{
    backend::renderer::{
        element::surface::{render_elements_from_surface_tree, WaylandSurfaceRenderElement},
        gles::GlesRenderer,
        Bind, ImportDma, ImportEgl, Unbind,
    },
    desktop::utils::send_frames_surface_tree,
    output::Output,
    reexports::wayland_server::{backend::GlobalId, DisplayHandle},
    wayland::dmabuf::DmabufState,
};
use tracing::error;

use crate::client_state::ClientState;
use crate::server_state::ServerState;
use crate::space::WrapperSpace;

/// group of info for an output
pub type OutputGroup = (Output, GlobalId, String, c_wl_output::WlOutput);

/// the  global state for the embedded server state
#[allow(missing_debug_implementations)]
pub struct GlobalState<W: WrapperSpace + 'static> {
    /// the implemented space
    pub space: W,
    /// desktop client state
    pub client_state: ClientState<W>,
    /// embedded server state
    pub server_state: ServerState<W>,
    /// instant that the panel was started
    pub start_time: std::time::Instant,
}

impl<W: WrapperSpace + 'static> GlobalState<W> {
    pub(crate) fn new(
        client_state: ClientState<W>,
        server_state: ServerState<W>,
        space: W,
        start_time: std::time::Instant,
    ) -> Self {
        Self {
            space,
            client_state,
            server_state,
            start_time,
        }
    }
}

impl<W: WrapperSpace + 'static> GlobalState<W> {
    /// bind the display for the space
    pub fn bind_display(&mut self, dh: &DisplayHandle) {
        if let Some(renderer) = self.space.renderer() {
            let res = renderer.bind_wl_display(dh);
            if let Err(err) = res {
                error!("{:?}", err);
            } else {
                let dmabuf_formats = renderer.dmabuf_formats().into_iter().collect_vec();
                let mut state = DmabufState::new();
                let global = state.create_global::<GlobalState<W>>(dh, dmabuf_formats);
                self.server_state.dmabuf_state.replace((state, global));
            }
        }
    }

    /// draw the dnd icon if it exists and is ready
    pub fn draw_dnd_icon(&mut self) {
        // TODO proxied layer surfaces
        if let Some(dnd_icon) = self
            .server_state
            .seats
            .iter_mut()
            .find(|s| s.client.dnd_icon.is_some() && s.server.dnd_icon.is_some())
        {
            let (egl_surface, wl_surface, ref mut dmg_tracked_renderer, is_dirty, has_frame) =
                dnd_icon.client.dnd_icon.as_mut().unwrap();
            if !*is_dirty || !has_frame.is_some() {
                return;
            }
            *is_dirty = false;
            let time = has_frame.take().unwrap();
            let clear_color = &[0.0, 0.0, 0.0, 0.0];
            let renderer = match self.space.renderer() {
                Some(r) => r,
                None => {
                    error!("no renderer");
                    return;
                }
            };
            let s_icon = dnd_icon.server.dnd_icon.as_ref().unwrap();
            let _ = renderer.unbind();
            let _ = renderer.bind(egl_surface.clone());
            let elements: Vec<WaylandSurfaceRenderElement<GlesRenderer>> =
                render_elements_from_surface_tree(renderer, s_icon, (1, 1), 1.0, 1.0);
            dmg_tracked_renderer
                .render_output(
                    renderer,
                    egl_surface.buffer_age().unwrap_or_default() as usize,
                    &elements,
                    *clear_color,
                )
                .unwrap();
            egl_surface.swap_buffers(None).unwrap();
            // FIXME: damage tracking issues on integrated graphics but not nvidia
            // self.egl_surface
            //     .as_ref()
            //     .unwrap()
            //     .swap_buffers(res.0.as_deref_mut())?;

            renderer.unbind().unwrap();
            // // TODO what if there is "no output"?
            for o in &self.client_state.outputs {
                let output = &o.1;
                send_frames_surface_tree(
                    s_icon,
                    &o.1,
                    Duration::from_millis(time as u64),
                    None,
                    move |_, _| Some(output.clone()),
                );
            }
            wl_surface.frame(&self.client_state.queue_handle, wl_surface.clone());
            wl_surface.commit();
        }
    }
}
