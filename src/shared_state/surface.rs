// SPDX-License-Identifier: GPL-3.0-only

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use anyhow::Result;
use libc::{c_int, c_void};
use sctk::{
    reexports::{
        client::protocol::{
            wl_output::{self as c_wl_output},
            wl_shm, wl_surface as c_wl_surface,
        },
        client::{self, Attached, Main},
    },
    shm::AutoMemPool,
};
use slog::{info, trace, Logger};
use smithay::backend::{
    egl::{
        context::{EGLContext, GlAttributes},
        display::{EGLDisplay, EGLDisplayHandle},
        ffi,
        native::{EGLNativeDisplay, EGLNativeSurface, EGLPlatform},
        surface::EGLSurface,
        wrap_egl_call, EGLError,
    },
    renderer::{
        gles2::Gles2Renderer,
        utils::{draw_surface_tree, on_commit_buffer_handler},
        Bind, Renderer,
    },
};
use smithay::egl_platform;
use smithay::reexports::wayland_protocols::wlr::unstable::layer_shell::v1::client::{
    zwlr_layer_shell_v1, zwlr_layer_surface_v1,
};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface as s_WlSurface;

use crate::XdgWrapperConfig;

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum RenderEvent {
    Configure { width: u32, height: u32 },
    Closed,
}

#[derive(Debug)]
pub struct ClientEglSurface {
    wl_egl_surface: wayland_egl::WlEglSurface,
    display: client::Display,
}

static SURFACE_ATTRIBUTES: [c_int; 3] = [
    ffi::egl::RENDER_BUFFER as c_int,
    ffi::egl::BACK_BUFFER as c_int,
    ffi::egl::NONE as c_int,
];

impl EGLNativeDisplay for ClientEglSurface {
    fn supported_platforms(&self) -> Vec<EGLPlatform<'_>> {
        let display: *mut c_void = self.display.c_ptr() as *mut _;
        vec![
            // see: https://www.khronos.org/registry/EGL/extensions/KHR/EGL_KHR_platform_wayland.txt
            egl_platform!(PLATFORM_WAYLAND_KHR, display, &["EGL_KHR_platform_wayland"]),
            // see: https://www.khronos.org/registry/EGL/extensions/EXT/EGL_EXT_platform_wayland.txt
            egl_platform!(PLATFORM_WAYLAND_EXT, display, &["EGL_EXT_platform_wayland"]),
        ]
    }
}

unsafe impl EGLNativeSurface for ClientEglSurface {
    fn create(
        &self,
        display: &Arc<EGLDisplayHandle>,
        config_id: ffi::egl::types::EGLConfig,
    ) -> Result<*const c_void, EGLError> {
        wrap_egl_call(|| unsafe {
            ffi::egl::CreatePlatformWindowSurfaceEXT(
                display.handle,
                config_id,
                self.wl_egl_surface.ptr() as *mut _,
                SURFACE_ATTRIBUTES.as_ptr(),
            )
        })
    }

    fn resize(&self, width: i32, height: i32, dx: i32, dy: i32) -> bool {
        wayland_egl::WlEglSurface::resize(&self.wl_egl_surface, width, height, dx, dy);
        true
    }
}

#[derive(Debug)]
pub struct Surface {
    pub egl_display: EGLDisplay,
    pub egl_surface: Rc<EGLSurface>,
    pub renderer: Gles2Renderer,
    pub surface: c_wl_surface::WlSurface,
    pub layer_surface: Main<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>,
    pub next_render_event: Rc<Cell<Option<RenderEvent>>>,
    pub pool: AutoMemPool,
    pub dimensions: (u32, u32),
    pub config: XdgWrapperConfig,
    pub log: Logger,
}

impl Surface {
    pub(crate) fn new(
        output: &c_wl_output::WlOutput,
        surface: c_wl_surface::WlSurface,
        layer_shell: &Attached<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
        pool: AutoMemPool,
        config: XdgWrapperConfig,
        log: Logger,
        display: client::Display,
    ) -> Self {
        let layer_surface = layer_shell.get_layer_surface(
            &surface,
            Some(output),
            config.layer.into(),
            "example".to_owned(),
        );

        layer_surface.set_anchor(config.anchor.into());
        layer_surface.set_keyboard_interactivity(config.keyboard_interactivity.into());
        let (x, y) = config.dimensions;
        layer_surface.set_size(x, y);
        // Anchor to the top left corner of the output

        let next_render_event = Rc::new(Cell::new(None::<RenderEvent>));
        let next_render_event_handle = Rc::clone(&next_render_event);
        let logger = log.clone();
        layer_surface.quick_assign(move |layer_surface, event, _| {
            match (event, next_render_event_handle.get()) {
                (zwlr_layer_surface_v1::Event::Closed, _) => {
                    info!(logger, "Received close event. closing.");
                    next_render_event_handle.set(Some(RenderEvent::Closed));
                }
                (
                    zwlr_layer_surface_v1::Event::Configure {
                        serial,
                        width,
                        height,
                    },
                    next,
                ) if next != Some(RenderEvent::Closed) => {
                    trace!(
                        logger,
                        "received configure event {:?} {:?} {:?}",
                        serial,
                        width,
                        height
                    );
                    layer_surface.ack_configure(serial);
                    next_render_event_handle.set(Some(RenderEvent::Configure { width, height }));
                    // TODO handle resize for egl surface here?
                }
                (_, _) => {}
            }
        });

        // Commit so that the server will send a configure event
        surface.commit();
        let client_egl_surface = ClientEglSurface {
            wl_egl_surface: wayland_egl::WlEglSurface::new(&surface, x as i32, y as i32),
            display: display,
        };

        let egl_display = EGLDisplay::new(&client_egl_surface, log.clone())
            .expect("Failed to initialize EGL display");
        let egl_context = EGLContext::new_with_config(
            &egl_display,
            GlAttributes {
                version: (3, 0),
                profile: None,
                debug: cfg!(debug_assertions),
                vsync: true,
            },
            Default::default(),
            log.clone(),
        )
        .expect("Failed to initialize EGL context");
        let egl_surface = Rc::new(
            EGLSurface::new(
                &egl_display,
                egl_context
                    .pixel_format()
                    .expect("Failed to get pixel format from EGL context "),
                egl_context.config_id(),
                client_egl_surface,
                log.clone(),
            )
            .expect("Failed to initialize EGL Surface"),
        );
        let mut renderer = unsafe {
            Gles2Renderer::new(egl_context, log.clone()).expect("Failed to initialize EGL Surface")
        };
        renderer
            .bind(egl_surface.clone())
            .expect("Failed to bind surface to GL");

        Self {
            egl_display,
            egl_surface,
            renderer,
            surface,
            layer_surface,
            next_render_event,
            pool,
            dimensions: (0, 0),
            config,
            log,
        }
    }

    /// Handles any events that have occurred since the last call, redrawing if needed.
    /// Returns true if the surface should be dropped.
    pub fn handle_events(&mut self) -> bool {
        match self.next_render_event.take() {
            Some(RenderEvent::Closed) => true,
            Some(RenderEvent::Configure { width, height }) => {
                if self.dimensions != (width, height) {
                    self.dimensions = (width, height);
                    self.draw();
                }
                false
            }
            None => false,
        }
    }

    pub fn draw(&mut self) {
        let stride = 4 * self.dimensions.0 as i32;
        let width = self.dimensions.0 as i32;
        let height = self.dimensions.1 as i32;

        // Note: unwrap() is only used here in the interest of simplicity of the example.
        // A "real" application should handle the case where both pools are still in use by the
        // compositor.
        let (canvas, buffer) = self
            .pool
            .buffer(width, height, stride, wl_shm::Format::Argb8888)
            .unwrap();

        for dst_pixel in canvas.chunks_exact_mut(4) {
            let pixel = 0xff00ff00u32.to_ne_bytes();
            dst_pixel[0] = pixel[0];
            dst_pixel[1] = pixel[1];
            dst_pixel[2] = pixel[2];
            dst_pixel[3] = pixel[3];
        }

        // Attach the buffer to the surface and mark the entire surface as damaged
        self.surface.attach(Some(&buffer), 0, 0);
        self.surface
            .damage_buffer(0, 0, width as i32, height as i32);

        // Finally, commit the surface
        self.surface.commit();
    }

    pub fn render(&mut self, surface: s_WlSurface) {
        let width = self.dimensions.0 as i32;
        let height = self.dimensions.1 as i32;
        let logger = self.log.clone();
        let egl_surface = &self.egl_surface;

        on_commit_buffer_handler(&surface);
        self.renderer
            .render(
                (width, height).into(),
                smithay::utils::Transform::Normal,
                move |self_: &mut Gles2Renderer, frame| {
                    let damage = [smithay::utils::Rectangle {
                        loc: (0, 0).into(),
                        size: (width, height).into(),
                    }];
                    draw_surface_tree(self_, frame, &surface, 1.0, (0, 0).into(), &damage, &logger)
                        .expect("Failed to draw surface tree");
                    let mut damage = [smithay::utils::Rectangle {
                        loc: (0, 0).into(),
                        size: (width, height).into(),
                    }];

                    egl_surface
                        .swap_buffers(Some(&mut damage))
                        .expect("Failed to swap buffers.");
                },
            )
            .expect("Failed to render to layer shell surface.");
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        self.layer_surface.destroy();
        self.surface.destroy();
    }
}
