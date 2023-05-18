use std::rc::Rc;

use sctk::{
    reexports::client::Proxy,
    shell::{
        wlr_layer::{self, Anchor, KeyboardInteractivity},
        WaylandSurface,
    },
};
use smithay::{
    backend::{
        egl::{EGLDisplay, EGLSurface},
        renderer::damage::OutputDamageTracker,
    },
    delegate_layer_shell,
    desktop::LayerSurface as SmithayLayerSurface,
    utils::Transform,
    wayland::shell::wlr_layer::{ExclusiveZone, Layer, WlrLayerShellHandler},
};
use wayland_egl::WlEglSurface;

use crate::{
    client_state::SurfaceState,
    shared_state::GlobalState,
    space::{ClientEglDisplay, ClientEglSurface, WrapperSpace},
};
delegate_layer_shell!(@<W: WrapperSpace + 'static> GlobalState<W>);
impl<W: WrapperSpace> WlrLayerShellHandler for GlobalState<W> {
    fn shell_state(&mut self) -> &mut smithay::wayland::shell::wlr_layer::WlrLayerShellState {
        &mut self.server_state.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface: smithay::wayland::shell::wlr_layer::LayerSurface,
        output: Option<smithay::reexports::wayland_server::protocol::wl_output::WlOutput>,
        layer: Layer,
        namespace: String,
    ) {
        // layer created by client
        // request received here
        // layer created in compositor & tracked by xdg-shell-wrapper in its own space that spans all outputs
        // get renderer from wrapper space and draw to it
        let renderer = match self.space.renderer() {
            Some(r) => r,
            None => return,
        };
        let mut size = surface.with_pending_state(|s| s.size).unwrap_or_default();
        let server_surface = SmithayLayerSurface::new(surface, namespace.clone());
        let state = server_surface.cached_state();
        let anchor = Anchor::from_bits(state.anchor.bits());

        if !state.anchor.anchored_horizontally() {
            size.w = 1.max(size.w);
        }
        if !state.anchor.anchored_vertically() {
            size.h = 1.max(size.h);
        }

        let output = self.client_state.outputs.iter().find(|o| {
            output
                .as_ref()
                .map(|output| o.1.owns(&output))
                .unwrap_or_default()
        });
        let surface = self
            .client_state
            .compositor_state
            .create_surface(&self.client_state.queue_handle);

        let exclusive_zone = match state.exclusive_zone {
            ExclusiveZone::Exclusive(area) => area as i32,
            ExclusiveZone::Neutral => 0,
            ExclusiveZone::DontCare => -1,
        };
        let layer = match layer {
            Layer::Background => wlr_layer::Layer::Background,
            Layer::Bottom => wlr_layer::Layer::Bottom,
            Layer::Top => wlr_layer::Layer::Top,
            Layer::Overlay => wlr_layer::Layer::Overlay,
        };
        let interactivity = match state.keyboard_interactivity {
            smithay::wayland::shell::wlr_layer::KeyboardInteractivity::None => {
                KeyboardInteractivity::None
            }
            smithay::wayland::shell::wlr_layer::KeyboardInteractivity::Exclusive => {
                KeyboardInteractivity::Exclusive
            }
            smithay::wayland::shell::wlr_layer::KeyboardInteractivity::OnDemand => {
                KeyboardInteractivity::OnDemand
            }
        };
        let client_surface = self.client_state.layer_state.create_layer_surface(
            &self.client_state.queue_handle,
            surface,
            layer,
            Some(namespace),
            output.as_ref().map(|o| &o.0),
        );
        client_surface.set_margin(
            state.margin.top,
            state.margin.right,
            state.margin.bottom,
            state.margin.left,
        );
        client_surface.set_keyboard_interactivity(interactivity);
        client_surface.set_size(size.w as u32, size.h as u32);
        client_surface.set_exclusive_zone(exclusive_zone);
        if let Some(anchor) = anchor {
            client_surface.set_anchor(anchor);
        }

        client_surface.commit();
        let client_egl_surface = unsafe {
            ClientEglSurface::new(
                WlEglSurface::new(
                    client_surface.wl_surface().id(),
                    size.w.max(1),
                    size.h.max(1),
                )
                .unwrap(), // TODO remove unwrap
                client_surface.wl_surface().clone(),
            )
        };

        let egl_surface = Rc::new(
            EGLSurface::new(
                &renderer.egl_context().display(),
                renderer
                    .egl_context()
                    .pixel_format()
                    .expect("Failed to get pixel format from EGL context "),
                renderer.egl_context().config_id(),
                client_egl_surface,
            )
            .expect("Failed to create EGL Surface"),
        );

        self.client_state.proxied_layer_surfaces.push((
            egl_surface,
            OutputDamageTracker::new((size.w.max(1), size.h.max(1)), 1.0, Transform::Flipped180),
            server_surface,
            client_surface,
            SurfaceState::Waiting,
        ));
    }
}
