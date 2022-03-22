// SPDX-License-Identifier: GPL-3.0-only

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::{sync::Arc, time::Instant};

use anyhow::Result;
use libc::{c_int, c_void};
use sctk::{
    reexports::{
        client::protocol::{
            wl_callback as c_wl_callback,
            wl_output::{self as c_wl_output},
            wl_surface as c_wl_surface,
        },
        client::{self, Attached, Main},
    },
    shm::AutoMemPool,
};
use slog::{info, trace, warn, Logger};
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
    desktop::{utils::send_frames_surface_tree, Kind, PopupKind, Window},
    egl_platform,
    reexports::{
        wayland_protocols::{
            wlr::unstable::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1},
            xdg_shell::client::{
                xdg_popup::{self, XdgPopup},
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
    WaitConfigure,
    Configure { width: u32, height: u32 },
    Closed,
}

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum PopupRenderEvent {
    WaitConfigure,
    Configure {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    },
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
        let ptr = self.wl_egl_surface.ptr();
        if ptr.is_null() {
            panic!("recieved a null pointer for the wl_egl_surface.");
        }
        wrap_egl_call(|| unsafe {
            ffi::egl::CreatePlatformWindowSurfaceEXT(
                display.handle,
                config_id,
                ptr as *mut _,
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
    pub c_popup: Main<XdgPopup>,
    pub c_xdg_surface: Main<XdgSurface>,
    pub c_surface: c_wl_surface::WlSurface,
    pub s_surface: PopupSurface,
    pub egl_surface: Rc<EGLSurface>,
    pub next_render_event: Rc<Cell<Option<PopupRenderEvent>>>,
    pub dirty: bool,
}

impl Drop for Popup {
    fn drop(&mut self) {
        // dbg!(Rc::strong_count(&self.egl_surface));
        drop(&mut self.egl_surface);
        self.c_popup.destroy();
        self.c_xdg_surface.destroy();
        self.c_surface.destroy();
    }
}

#[derive(Debug)]
pub struct WrapperSurface {
    pub layer_surface: Main<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>,
    pub next_render_event: Rc<Cell<Option<RenderEvent>>>,
    pub s_top_level: Rc<RefCell<smithay::desktop::Window>>,
    pub egl_surface: Rc<EGLSurface>,
    pub dirty: bool,
    pub dimensions: (u32, u32),
    pub c_top_level: Attached<c_wl_surface::WlSurface>,
    pub popups: Vec<Popup>,
    pub is_root: bool,
    pub log: Logger,
}

impl WrapperSurface {
    /// Handles any events that have occurred since the last call, redrawing if needed.
    /// Returns true if the surface should be dropped.
    pub fn handle_events(&mut self) -> bool {
        if self.s_top_level.borrow().toplevel().get_surface().is_none() {
            return true;
        }
        let mut remove_surface = false;
        let popups = self
            .popups
            .drain_filter(|p| {
                // dbg!(p.s_surface.alive());
                if !p.s_surface.alive() {
                    return false;
                }
                match p.next_render_event.take() {
                    Some(PopupRenderEvent::Closed) => false,
                    Some(PopupRenderEvent::Configure { width, height, .. }) => {
                        p.egl_surface.resize(width as i32, height as i32, 0, 0);
                        p.dirty = true;
                        true
                    }
                    Some(PopupRenderEvent::WaitConfigure) => {
                        p.next_render_event
                            .replace(Some(PopupRenderEvent::WaitConfigure));
                        true
                    }
                    None => true,
                }
            })
            .collect();
        self.popups = popups;
        // dbg!(&self.popups);

        match self.next_render_event.take() {
            Some(RenderEvent::Closed) => {
                remove_surface = true;
            }
            Some(RenderEvent::Configure { width, height }) => {
                if self.dimensions != (width, height) {
                    dbg!((width, height));
                    self.dimensions = (width, height);
                    self.egl_surface.resize(width as i32, height as i32, 0, 0);
                    self.dirty = true;
                }
            }
            Some(RenderEvent::WaitConfigure) => {
                self.next_render_event
                    .replace(Some(RenderEvent::WaitConfigure));
            }
            None => (),
        }
        remove_surface
    }

    pub fn render(&mut self, time: u32, renderer: &mut Gles2Renderer) {
        // render top level surface
        if self.next_render_event.get() == Some(RenderEvent::WaitConfigure) {
            return;
        }
        if self.dirty {
            self.dirty = false;

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
            let width = s_top_level.geometry().size.w;
            let height = s_top_level.geometry().size.h;
            // dbg!((width, height));
            // dbg!(&loc);

            let _ = renderer.unbind();
            renderer
                .bind(egl_surface.clone())
                .expect("Failed to bind surface to GL");
            renderer
                .render(
                    (width, height).into(),
                    smithay::utils::Transform::Flipped180,
                    |self_: &mut Gles2Renderer, frame| {
                        let damage = smithay::utils::Rectangle::<i32, smithay::utils::Logical> {
                            loc: loc.clone(),
                            size: (width, height).into(),
                        };
                        // dbg!(damage);

                        frame
                            .clear(
                                [1.0, 1.0, 1.0, 1.0],
                                &[smithay::utils::Rectangle::<f64, smithay::utils::Logical> {
                                    loc: (loc.x as f64, loc.y as f64).clone().into(),
                                    size: (width as f64, height as f64).into(),
                                }
                                .to_physical(1.0)],
                            )
                            .expect("Failed to clear frame.");

                        let loc = (-loc.x, -loc.y);
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
        // dbg!(&self.popups);
        for p in &mut self.popups {
            if !p.dirty || !p.s_surface.alive() || p.next_render_event.get() != None {
                // dbg!(p.next_render_event.get());
                continue;
            }
            p.dirty = false;
            let wl_surface = match p.s_surface.get_surface() {
                Some(s) => s,
                _ => return,
            };
            let geometry = PopupKind::Xdg(p.s_surface.clone()).geometry();
            let loc = geometry.loc;
            let (width, height) = geometry.size.into();

            let logger = self.log.clone();
            let _ = renderer.unbind();
            renderer
                .bind(p.egl_surface.clone())
                .expect("Failed to bind surface to GL");
            renderer
                .render(
                    (width, height).into(),
                    smithay::utils::Transform::Flipped180,
                    |self_: &mut Gles2Renderer, frame| {
                        let damage = smithay::utils::Rectangle::<i32, smithay::utils::Logical> {
                            loc: loc.clone(),
                            size: (width, height).into(),
                        };

                        frame
                            .clear(
                                [1.0, 1.0, 1.0, 1.0],
                                &[smithay::utils::Rectangle::<f64, smithay::utils::Logical> {
                                    loc: (loc.x as f64, loc.y as f64).clone().into(),
                                    size: (width as f64, height as f64).into(),
                                }
                                .to_physical(1.0)],
                            )
                            .expect("Failed to clear frame.");
                        let loc = (-loc.x, -loc.y);
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

            p.egl_surface
                .swap_buffers(Some(&mut damage))
                .expect("Failed to swap buffers.");

            send_frames_surface_tree(wl_surface, time);
        }
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
    pub egl_display: Option<EGLDisplay>,
    pub renderer: Option<Gles2Renderer>,
    pub last_dirty: Instant,
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
            egl_display: None,
            renderer: None,
            surfaces: Default::default(),
            layer_shell,
            output,
            output_id,
            c_display,
            pool,
            config,
            log,
            needs_update: false,
            last_dirty: Instant::now(),
        }
    }

    pub fn handle_events(&mut self, time: u32) -> Instant {
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
                }
                if let Some(renderer) = self.renderer.as_mut() {
                    s.render(time, renderer);
                }
                return Some(s);
            })
            .collect();
        self.surfaces.append(&mut surfaces);
        self.last_dirty
    }

    pub fn apply_display(&mut self, s_display: &s_Display) {
        if !self.needs_update || self.renderer.is_none() {
            return;
        };

        if let Err(_err) = self.renderer.as_mut().unwrap().bind_wl_display(s_display) {
            warn!(
                self.log.clone(),
                "Failed to bind display to Egl renderer. Hardware acceleration will not be used."
            );
        }
        self.needs_update = false;
    }

    pub fn add_top_level(
        &mut self,
        c_surface: Attached<c_wl_surface::WlSurface>,
        s_top_level: Rc<RefCell<Window>>,
        mut dimensions: (u32, u32),
    ) {
        let layer_surface = self.layer_shell.get_layer_surface(
            &c_surface.clone(),
            Some(&self.output),
            self.config.layer.into(),
            "example".to_owned(),
        );
        dimensions = self.constrain_dim(dimensions);
        layer_surface.set_anchor(self.config.anchor.into());
        layer_surface.set_keyboard_interactivity(self.config.keyboard_interactivity.into());
        let (x, y) = dimensions;
        layer_surface.set_size(x, y);

        // Commit so that the server will send a configure event
        c_surface.commit();

        let client_egl_surface = ClientEglSurface {
            wl_egl_surface: wayland_egl::WlEglSurface::new(&c_surface, x as i32, y as i32),
            display: self.c_display.clone(),
        };

        if self.renderer.is_none() {
            self.needs_update = true;
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

            // dbg!(min_interval_attr);
            let renderer = unsafe {
                Gles2Renderer::new(egl_context, self.log.clone())
                    .expect("Failed to initialize EGL Surface")
            };
            dbg!(unsafe { SwapInterval(egl_display.get_display_handle().handle, 0) });
            self.egl_display = Some(egl_display);
            self.renderer = Some(renderer);
        }
        let renderer = self.renderer.as_ref().unwrap();

        let egl_surface = Rc::new(
            EGLSurface::new(
                self.egl_display.as_ref().unwrap(),
                renderer
                    .egl_context()
                    .pixel_format()
                    .expect("Failed to get pixel format from EGL context "),
                renderer.egl_context().config_id(),
                client_egl_surface,
                self.log.clone(),
            )
            .expect("Failed to initialize EGL Surface"),
        );

        let next_render_event = Rc::new(Cell::new(Some(RenderEvent::WaitConfigure)));

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
            dimensions: (0, 0),
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
        w: i32,
        h: i32,
    ) {
        for s in &mut self.surfaces {
            let top_level = s.s_top_level.borrow();
            let wl_s = match top_level.toplevel() {
                Kind::Xdg(wl_s) => wl_s.get_surface(),
                _ => None,
            };
            if wl_s == Some(&parent) {
                s.layer_surface.get_popup(&c_popup);
                //must be done after role is assigned as popup
                c_surface.commit();
                let next_render_event = Rc::new(Cell::new(Some(PopupRenderEvent::WaitConfigure)));
                c_xdg_surface.quick_assign(move |c_xdg_surface, e, _| {
                    if let xdg_surface::Event::Configure { serial, .. } = e {
                        c_xdg_surface.ack_configure(serial);
                    } // TODO set render event
                });

                let next_render_event_handle = next_render_event.clone();
                c_popup.quick_assign(move |_c_popup, e, _| {
                    if let Some(PopupRenderEvent::Closed) = next_render_event_handle.get().as_ref()
                    {
                        return;
                    }

                    match e {
                        xdg_popup::Event::Configure {
                            x,
                            y,
                            width,
                            height,
                        } => {
                            next_render_event_handle.set(Some(PopupRenderEvent::Configure {
                                x,
                                y,
                                width,
                                height,
                            }));
                        }
                        xdg_popup::Event::PopupDone => {
                            next_render_event_handle.set(Some(PopupRenderEvent::Closed));
                        }
                        _ => {}
                    };
                });
                let client_egl_surface = ClientEglSurface {
                    wl_egl_surface: wayland_egl::WlEglSurface::new(&c_surface, w, h),
                    display: self.c_display.clone(),
                };

                let egl_context = self.renderer.as_mut().unwrap().egl_context();
                let egl_surface = Rc::new(
                    EGLSurface::new(
                        self.egl_display.as_ref().unwrap(),
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
                    c_popup,
                    c_xdg_surface,
                    c_surface,
                    s_surface,
                    egl_surface,
                    dirty: false,
                    next_render_event,
                });
                break;
            }
        }
    }

    pub fn dirty(&mut self, other_top_level_surface: &s_WlSurface, (w, h): (u32, u32)) {
        self.last_dirty = Instant::now();
        for s in &mut self.surfaces {
            if let Kind::Xdg(surface) = s.s_top_level.borrow().toplevel() {
                if let Some(surface) = surface.get_surface() {
                    if other_top_level_surface == surface {
                        if s.dimensions != (w, h) {
                            s.layer_surface.set_size(w, h);
                            s.c_top_level.commit();
                        } else {
                            s.dirty = true;
                        }
                    }
                }
            }
        }
    }

    pub fn dirty_popup(
        &mut self,
        other_top_level_surface: &s_WlSurface,
        other_popup: PopupSurface,
    ) {
        self.last_dirty = Instant::now();
        for s in &mut self.surfaces {
            if let Kind::Xdg(surface) = s.s_top_level.borrow_mut().toplevel() {
                if let Some(surface) = surface.get_surface() {
                    if other_top_level_surface == surface {
                        for popup in &mut s.popups {
                            if popup.s_surface.get_surface() == other_popup.get_surface() {
                                popup.dirty = true;
                            }
                        }
                    }
                }
            }
        }
    }

    fn constrain_dim(&self, (mut w, mut h): (u32, u32)) -> (u32, u32) {
        let (min_w, min_h) = self.config.min_dimensions.unwrap_or((1, 1));
        w = min_w.max(w);
        h = min_h.max(h);
        // TODO get monitor dimensions
        if let Some((max_w, max_h)) = self.config.max_dimensions {
            w = max_w.min(w);
            h = max_h.min(h);
        }
        (w, h)
    }
}

impl Drop for WrapperSurface {
    fn drop(&mut self) {
        for p in &self.popups {
            p.c_popup.destroy();
            p.c_xdg_surface.destroy();
            p.c_surface.destroy();
        }
        self.layer_surface.destroy();
        self.c_top_level.destroy();
    }
}
