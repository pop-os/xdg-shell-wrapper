use std::rc::Rc;

use sctk::{
    reexports::client::Proxy,
    shell::layer::{
        Anchor, KeyboardInteractivity, Layer as SctkLayer, LayerSurface as SctkLayerSurface,
    },
};
use smithay::{
    backend::{
        egl::{EGLDisplay, EGLSurface},
        renderer::damage::DamageTrackedRenderer,
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

        let exclusive_zone = match state.exclusive_zone {
            ExclusiveZone::Exclusive(area) => area as i32,
            ExclusiveZone::Neutral => 0,
            ExclusiveZone::DontCare => -1,
        };
        let layer = match layer {
            Layer::Background => SctkLayer::Background,
            Layer::Bottom => SctkLayer::Bottom,
            Layer::Top => SctkLayer::Top,
            Layer::Overlay => SctkLayer::Overlay,
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

        let mut layer_surface_builder = SctkLayerSurface::builder()
            .namespace(namespace)
            .exclusive_zone(exclusive_zone)
            .margin(
                state.margin.top,
                state.margin.right,
                state.margin.bottom,
                state.margin.left,
            )
            .keyboard_interactivity(interactivity)
            .size((size.w as u32, size.h as u32));
        if let Some(anchor) = anchor {
            layer_surface_builder = layer_surface_builder.anchor(anchor);
        }
        if let Some(output) = output {
            layer_surface_builder = layer_surface_builder.output(&output.0)
        }

        if let Ok(client_surface) = layer_surface_builder.map(
            &self.client_state.queue_handle,
            &self.client_state.layer_state,
            self.client_state
                .compositor_state
                .create_surface(&self.client_state.queue_handle),
            layer,
        ) {
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
            let egl_display = EGLDisplay::new(
                ClientEglDisplay {
                    display: self.client_state.connection.display(),
                },
                self.log.clone(),
            )
            .expect("Failed to create EGL display");

            let egl_surface = Rc::new(
                EGLSurface::new(
                    &egl_display,
                    renderer
                        .egl_context()
                        .pixel_format()
                        .expect("Failed to get pixel format from EGL context "),
                    renderer.egl_context().config_id(),
                    client_egl_surface,
                    self.log.clone(),
                )
                .expect("Failed to create EGL Surface"),
            );

            self.client_state.proxied_layer_surfaces.push((
                egl_surface,
                DamageTrackedRenderer::new(
                    (size.w.max(1), size.h.max(1)),
                    1.0,
                    Transform::Flipped180,
                ),
                server_surface,
                client_surface,
                SurfaceState::Waiting,
            ));
        }
    }
}
