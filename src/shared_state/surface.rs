// SPDX-License-Identifier: GPL-3.0-only

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use anyhow::Result;
use libc::{c_int, c_void};
use sctk::{
    reexports::{
        client::protocol::{
            wl_output::{self as c_wl_output},
            wl_surface as c_wl_surface,
        },
        client::{self, Attached, Main},
    },
    shm::AutoMemPool,
};
use slog::{error, info, trace, warn, Logger};
use smithay::{
    backend::{
        egl::{
            context::{EGLContext, GlAttributes},
            display::{EGLDisplay, EGLDisplayHandle},
            ffi::{
                self,
                egl::{GetConfigAttrib, SwapInterval},
            },
            native::{EGLNativeDisplay, EGLNativeSurface, EGLPlatform},
            surface::EGLSurface,
            wrap_egl_call, EGLError,
        },
        renderer::{
            gles2::Gles2Renderer, utils::draw_surface_tree, Bind, Frame, ImportEgl, Renderer,
            Unbind,
        },
    },
    desktop::{utils::send_frames_surface_tree, Kind, PopupKind, PopupManager, Window},
    egl_platform,
    reexports::{
        wayland_protocols::{
            wlr::unstable::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1},
            xdg_shell::client::{
                xdg_popup::XdgPopup,
                xdg_surface::{self, XdgSurface},
            },
        },
        wayland_server::{protocol::wl_surface::WlSurface as s_WlSurface, Display as s_Display},
    },
    wayland::shell::xdg::PopupSurface,
};

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
pub struct Popup {
    pub c_surface: c_wl_surface::WlSurface,
    pub s_surface: PopupSurface,
    pub egl_surface: Rc<EGLSurface>,
    pub egl_display: EGLDisplay,
}

#[derive(Debug)]
pub struct WrapperSurface {
    pub layer_surface: Main<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>,
    pub renderer: Gles2Renderer,
    pub next_render_event: Rc<Cell<Option<RenderEvent>>>,
    pub dimensions: (u32, u32),
    pub c_top_level: c_wl_surface::WlSurface,
    pub popups: Vec<Popup>,
    pub s_top_level: Rc<RefCell<smithay::desktop::Window>>,
    pub egl_display: EGLDisplay,
    pub egl_surface: Rc<EGLSurface>,
    pub dirty: bool,
    pub is_root: bool,
    pub log: Logger,
}

impl WrapperSurface {
    /// Handles any events that have occurred since the last call, redrawing if needed.
    /// Returns true if the surface should be dropped.
    pub fn handle_events(&mut self) -> bool {
        let mut dirty = false;
        let popups = self
            .popups
            .drain_filter(|_| match self.next_render_event.take() {
                Some(RenderEvent::Closed) => {
                    dirty = true;
                    false
                }
                Some(RenderEvent::Configure { width, height }) => {
                    self.egl_surface.resize(width as i32, height as i32, 0, 0);
                    self.dirty = true;
                    true
                }
                None => true,
            })
            .collect();
        self.popups = popups;

        match self.next_render_event.take() {
            Some(RenderEvent::Closed) => {
                dirty = true;
            }
            Some(RenderEvent::Configure { width, height }) => {
                if self.dimensions != (width, height) {
                    self.dimensions = (width, height);
                    self.egl_surface.resize(width as i32, height as i32, 0, 0);
                    self.dirty = true;
                }
            }
            None => (),
        }
        dirty
    }

    pub fn render(&mut self, time: u32) {
        // render top level surface
        {
            let width = self.dimensions.0 as i32;
            let height = self.dimensions.1 as i32;
            let logger = self.log.clone();
            let egl_surface = &self.egl_surface;
            let s_top_level = self.s_top_level.borrow();
            let server_surface = match s_top_level.toplevel() {
                Kind::Xdg(xdg_surface) => match xdg_surface.get_surface() {
                    Some(s) => s,
                    _ => return,
                },
                _ => return,
            };
            let loc = s_top_level.geometry().loc;

            let _ = self.renderer.unbind();
            self.renderer
                .bind(egl_surface.clone())
                .expect("Failed to bind surface to GL");
            self.renderer
                .render(
                    (width, height).into(),
                    smithay::utils::Transform::Flipped180,
                    |self_: &mut Gles2Renderer, frame| {
                        let damage = smithay::utils::Rectangle::<i32, smithay::utils::Logical> {
                            loc: loc.clone(),
                            size: (width, height).into(),
                        };

                        let loc = (-loc.x, -loc.y);
                        frame
                            .clear([1.0, 1.0, 1.0, 0.0], &[damage.to_physical(1)])
                            .expect("Failed to clear frame.");
                        draw_surface_tree(
                            self_,
                            frame,
                            server_surface,
                            1.0,
                            loc.into(),
                            &[damage],
                            &logger,
                        )
                        .expect("Failed to draw surface tree");
                    },
                )
                .expect("Failed to render to layer shell surface.");

            let mut damage = [smithay::utils::Rectangle {
                loc: (0, 0).into(),
                size: (width, height).into(),
            }];

            egl_surface
                .swap_buffers(Some(&mut damage))
                .expect("Failed to swap buffers.");

            send_frames_surface_tree(server_surface, time);
        }
        // render popups
        for Popup {
            c_surface,
            s_surface,
            egl_surface,
            ..
        } in &self.popups
        {
            let geometry = PopupKind::Xdg(s_surface.clone()).geometry();
            let loc = geometry.loc;
            let (width, height) = geometry.size.into();
            let wl_surface = match s_surface.get_surface() {
                Some(s) => s,
                _ => return,
            };

            let logger = self.log.clone();
            let _ = self.renderer.unbind();
            self.renderer
                .bind(egl_surface.clone())
                .expect("Failed to bind surface to GL");
            self.renderer
                .render(
                    (width, height).into(),
                    smithay::utils::Transform::Flipped180,
                    |self_: &mut Gles2Renderer, frame| {
                        let damage = smithay::utils::Rectangle::<i32, smithay::utils::Logical> {
                            loc: loc.clone(),
                            size: (width, height).into(),
                        };

                        let loc = (-loc.x, -loc.y);
                        frame
                            .clear([1.0, 1.0, 1.0, 0.0], &[damage.to_physical(1)])
                            .expect("Failed to clear frame.");
                        draw_surface_tree(
                            self_,
                            frame,
                            wl_surface,
                            1.0,
                            loc.into(),
                            &[damage],
                            &logger,
                        )
                        .expect("Failed to draw surface tree");
                    },
                )
                .expect("Failed to render to layer shell surface.");

            let mut damage = [smithay::utils::Rectangle {
                loc: (0, 0).into(),
                size: (width, height).into(),
            }];

            egl_surface
                .swap_buffers(Some(&mut damage))
                .expect("Failed to swap buffers.");

            send_frames_surface_tree(wl_surface, time);
        }
        self.dirty = false;
    }
}

#[derive(Debug)]
pub struct WrapperRenderer {
    pub surfaces: Vec<WrapperSurface>,
    pub pool: AutoMemPool,
    pub layer_shell: Attached<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    pub output: c_wl_output::WlOutput,
    pub output_id: u32,
    pub c_display: client::Display,
    pub config: XdgWrapperConfig,
    pub log: Logger,
    pub needs_update: bool,
}

impl WrapperRenderer {
    pub(crate) fn new(
        output: c_wl_output::WlOutput,
        output_id: u32,
        pool: AutoMemPool,
        config: XdgWrapperConfig,
        c_display: client::Display,
        layer_shell: Attached<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
        log: Logger,
    ) -> Self {
        Self {
            surfaces: Default::default(),
            layer_shell,
            output,
            output_id,
            c_display,
            pool,
            config,
            log,
            needs_update: false,
        }
    }

    pub fn handle_events(&mut self, time: u32) -> bool {
        let mut updated = false;
        let mut surfaces = self
            .surfaces
            .drain(..)
            .filter_map(|mut s| {
                let remove = s.handle_events();
                if remove {
                    if s.is_root {
                        trace!(self.log, "root window removed, exiting...");
                        std::process::exit(0);
                    }
                    return None;
                } else if s.dirty {
                    updated = true;
                    s.render(time);
                }
                return Some(s);
            })
            .collect();
        self.surfaces.append(&mut surfaces);
        updated
    }

    pub fn apply_display(&mut self, s_display: &s_Display) {
        if !self.needs_update {
            return;
        };

        for s in &mut self.surfaces {
            if let Err(_err) = s.renderer.bind_wl_display(s_display) {
                warn!(
                    self.log.clone(),
                    "Failed to bind display to Egl renderer. Hardware acceleration will not be used."
                );
            }
        }
        self.needs_update = false;
    }

    pub fn add_top_level(
        &mut self,
        c_surface: c_wl_surface::WlSurface,
        s_top_level: Rc<RefCell<Window>>,
    ) {
        self.needs_update = true;
        let layer_surface = self.layer_shell.get_layer_surface(
            &c_surface,
            Some(&self.output),
            self.config.layer.into(),
            "example".to_owned(),
        );
        layer_surface.set_anchor(self.config.anchor.into());
        layer_surface.set_keyboard_interactivity(self.config.keyboard_interactivity.into());
        let (x, y) = self.config.dimensions;
        layer_surface.set_size(x, y);

        // Commit so that the server will send a configure event
        c_surface.commit();

        let client_egl_surface = ClientEglSurface {
            wl_egl_surface: wayland_egl::WlEglSurface::new(&c_surface, x as i32, y as i32),
            display: self.c_display.clone(),
        };
        let egl_display = EGLDisplay::new(&client_egl_surface, self.log.clone())
            .expect("Failed to initialize EGL display");

        let egl_context = EGLContext::new_with_config(
            &egl_display,
            GlAttributes {
                version: (3, 0),
                profile: None,
                debug: cfg!(debug_assertions),
                vsync: false,
            },
            Default::default(),
            self.log.clone(),
        )
        .expect("Failed to initialize EGL context");

        let mut min_interval_attr = 23239;
        unsafe {
            GetConfigAttrib(
                egl_display.get_display_handle().handle,
                egl_context.config_id(),
                ffi::egl::MIN_SWAP_INTERVAL as c_int,
                &mut min_interval_attr,
            );
        }

        dbg!(min_interval_attr);
        let egl_surface = Rc::new(
            EGLSurface::new(
                &egl_display,
                egl_context
                    .pixel_format()
                    .expect("Failed to get pixel format from EGL context "),
                egl_context.config_id(),
                client_egl_surface,
                self.log.clone(),
            )
            .expect("Failed to initialize EGL Surface"),
        );

        let mut renderer = unsafe {
            Gles2Renderer::new(egl_context, self.log.clone())
                .expect("Failed to initialize EGL Surface")
        };
        renderer
            .bind(egl_surface.clone())
            .expect("Failed to bind surface to GL");
        dbg!(unsafe { SwapInterval(egl_display.get_display_handle().handle, 0) });

        let next_render_event = Rc::new(Cell::new(None::<RenderEvent>));

        //let egl_surface_clone = egl_surface.clone();
        let next_render_event_handle = next_render_event.clone();
        let logger = self.log.clone();
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
                }
                (_, _) => {}
            }
        });
        let is_root = self.surfaces.len() == 0;
        let top_level = WrapperSurface {
            renderer,
            dimensions: self.config.dimensions,
            egl_display,
            egl_surface,
            layer_surface,
            is_root,
            next_render_event,
            s_top_level,
            popups: Default::default(),
            c_top_level: c_surface,
            log: self.log.clone(),
            dirty: true,
        };
        self.surfaces.push(top_level);
    }

    pub fn add_popup(
        &mut self,
        c_surface: c_wl_surface::WlSurface,
        c_xdg_surface: Main<XdgSurface>,
        c_popup: Main<XdgPopup>,
        s_surface: PopupSurface,
        parent: s_WlSurface,
    ) {
        for s in &mut self.surfaces {
            let top_level = s.s_top_level.borrow();
            let wl_s = match top_level.toplevel() {
                Kind::Xdg(wl_s) => wl_s.get_surface(),
                _ => None,
            };
            if wl_s == Some(&parent) {
                s.layer_surface.get_popup(&c_popup);
                c_surface.commit();
                c_xdg_surface.quick_assign(|c_xdg_surface, e, _| {
                    if let xdg_surface::Event::Configure { serial, .. } = e {
                        c_xdg_surface.ack_configure(serial);
                    } // TODO set render event
                });

                c_popup.quick_assign(|c_popup, e, _| {
                    // TODO handle popup events and update render events
                });
                let (width, height) = PopupKind::Xdg(s_surface.clone()).geometry().size.into();
                self.needs_update = true;
                let client_egl_surface = ClientEglSurface {
                    wl_egl_surface: wayland_egl::WlEglSurface::new(&c_surface, width, height),
                    display: self.c_display.clone(),
                };
                let egl_display = EGLDisplay::new(&client_egl_surface, self.log.clone())
                    .expect("Failed to initialize EGL display");

                let egl_context = EGLContext::new_with_config(
                    &egl_display,
                    GlAttributes {
                        version: (3, 0),
                        profile: None,
                        debug: cfg!(debug_assertions),
                        vsync: false,
                    },
                    Default::default(),
                    self.log.clone(),
                )
                .expect("Failed to initialize EGL context");

                let mut min_interval_attr = 23239;
                unsafe {
                    GetConfigAttrib(
                        egl_display.get_display_handle().handle,
                        egl_context.config_id(),
                        ffi::egl::MIN_SWAP_INTERVAL as c_int,
                        &mut min_interval_attr,
                    );
                }

                dbg!(min_interval_attr);
                let egl_surface = Rc::new(
                    EGLSurface::new(
                        &egl_display,
                        egl_context
                            .pixel_format()
                            .expect("Failed to get pixel format from EGL context "),
                        egl_context.config_id(),
                        client_egl_surface,
                        self.log.clone(),
                    )
                    .expect("Failed to initialize EGL Surface"),
                );

                s.popups.push(Popup {
                    c_surface,
                    s_surface,
                    egl_surface,
                    egl_display,
                });
                break;
            }
        }
    }

    pub fn dirty(&mut self, other_top_level_surface: &s_WlSurface) {
        for s in &mut self.surfaces {
            if let Kind::Xdg(surface) = s.s_top_level.borrow().toplevel() {
                if let Some(surface) = surface.get_surface() {
                    if other_top_level_surface == surface {
                        s.dirty = true;
                    }
                }
            }
        }
    }
}

impl Drop for WrapperSurface {
    fn drop(&mut self) {
        self.layer_surface.destroy();
        self.c_top_level.destroy();
    }
}
