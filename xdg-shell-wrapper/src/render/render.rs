// SPDX-License-Identifier: MPL-2.0-only

use std::cell::{Cell, RefCell};
use std::process::Child;
use std::rc::Rc;
use std::time::Instant;

use libc::c_int;
use sctk::{
    reexports::{
        client::protocol::{wl_output as c_wl_output, wl_surface as c_wl_surface},
        client::{self, Attached, Main},
    },
    shm::AutoMemPool,
};
use slog::{info, trace, warn, Logger};
use smithay::{
    backend::{
        egl::{
            context::{EGLContext, GlAttributes},
            display::EGLDisplay,
            ffi::{
                self,
                egl::{GetConfigAttrib, SwapInterval},
            },
            surface::EGLSurface,
        },
        renderer::{gles2::Gles2Renderer, ImportEgl},
    },
    desktop::{Kind, PopupKind, PopupManager, Window},
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
    utils::{Logical, Rectangle},
    wayland::shell::xdg::PopupSurface,
};

use crate::config::XdgWrapperConfig;
use crate::render::RenderEvent;

use super::{ClientEglSurface, Popup, PopupRenderEvent, ServerSurface, TopLevelSurface};

#[derive(Debug)]
pub struct WrapperRenderer {
    pub surfaces: Vec<TopLevelSurface>,
    pub pool: AutoMemPool,
    pub layer_shell: Attached<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    pub output: Option<(c_wl_output::WlOutput, String)>,
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
        output: Option<(c_wl_output::WlOutput, String)>,
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
            c_display,
            pool,
            config,
            log,
            needs_update: false,
            last_dirty: Instant::now(),
        }
    }

    pub fn handle_events(&mut self, time: u32, child: &mut Child) -> Instant {
        let mut surfaces = self
            .surfaces
            .drain(..)
            .filter_map(|mut s| {
                let remove = s.handle_events();
                if remove {
                    if s.is_root {
                        trace!(self.log, "root window removed, exiting...");
                        let _ = child.kill();
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
        dimensions = self.constrain_dim(dimensions);
        let (layer_surface, next_render_event, egl_surface) =
            self.get_layer_shell(c_surface.clone(), dimensions);

        let is_root = self.surfaces.len() == 0;
        let top_level = TopLevelSurface {
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
        popup_manager: Rc<RefCell<PopupManager>>,
    ) {
        let s = match self.surfaces.iter_mut().find(|s| {
            let top_level = s.s_top_level.borrow();
            let wl_s = match top_level.toplevel() {
                Kind::Xdg(wl_s) => wl_s.get_surface(),
            };
            wl_s == Some(&parent)
        }) {
            Some(s) => s,
            None => return,
        };

        s.layer_surface.get_popup(&c_popup);
        //must be done after role is assigned as popup
        c_surface.commit();
        let next_render_event = Rc::new(Cell::new(Some(PopupRenderEvent::WaitConfigure)));
        c_xdg_surface.quick_assign(move |c_xdg_surface, e, _| {
            if let xdg_surface::Event::Configure { serial, .. } = e {
                c_xdg_surface.ack_configure(serial);
            }
        });

        let next_render_event_handle = next_render_event.clone();
        let s_popup_surface = s_surface.clone();
        c_popup.quick_assign(move |_c_popup, e, _| {
            if let Some(PopupRenderEvent::Closed) = next_render_event_handle.get().as_ref() {
                return;
            }

            match e {
                xdg_popup::Event::Configure {
                    x,
                    y,
                    width,
                    height,
                } => {
                    let kind = PopupKind::Xdg(s_popup_surface.clone());
                    let _ = s_popup_surface.with_pending_state(|popup_state| {
                        popup_state.geometry.loc = (x, y).into();
                        popup_state.geometry.size = (width, height).into();
                    });

                    let _ = s_popup_surface.send_configure();
                    let _ = popup_manager.borrow_mut().track_popup(kind.clone());
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
            bbox: Rectangle::from_loc_and_size((0, 0), (0, 0)),
        });
    }

    pub fn dirty(&mut self, other_top_level_surface: &s_WlSurface, (w, h): (u32, u32)) {
        self.last_dirty = Instant::now();

        if let Some(s) = self.surfaces.iter_mut().find(|s| {
            let top_level = s.s_top_level.borrow();
            let wl_s = match top_level.toplevel() {
                Kind::Xdg(wl_s) => wl_s.get_surface(),
            };
            wl_s == Some(other_top_level_surface)
        }) {
            if s.dimensions != (w, h) {
                s.dimensions = (w, h);
                s.egl_surface.resize(w as i32, h as i32, 0, 0);
                s.layer_surface.set_size(w, h);
                s.c_top_level.commit();
                s.dirty = true;
            } else {
                s.dirty = true;
            }
        }
    }

    pub fn dirty_popup(
        &mut self,
        other_top_level_surface: &s_WlSurface,
        other_popup: PopupSurface,
        dim: Rectangle<i32, Logical>,
    ) {
        self.last_dirty = Instant::now();
        if let Some(s) = self.surfaces.iter_mut().find(|s| {
            let top_level = s.s_top_level.borrow();
            let wl_s = match top_level.toplevel() {
                Kind::Xdg(wl_s) => wl_s.get_surface(),
            };
            wl_s == Some(other_top_level_surface)
        }) {
            for popup in &mut s.popups {
                if popup.s_surface.get_surface() == other_popup.get_surface() {
                    // TODO use loc
                    if popup.bbox != dim {
                        popup.bbox = dim;
                        popup.egl_surface.resize(dim.size.w, dim.size.h, 0, 0);
                    }
                    popup.dirty = true;
                    break;
                }
            }
        }
    }

    pub fn find_server_surface(
        &self,
        other_c_surface: &c_wl_surface::WlSurface,
    ) -> Option<ServerSurface> {
        for s in &self.surfaces {
            if *s.c_top_level == *other_c_surface {
                return Some(ServerSurface::TopLevel(s.s_top_level.clone()));
            } else {
                for popup in &s.popups {
                    if &popup.c_surface == other_c_surface {
                        return Some(ServerSurface::Popup(
                            s.s_top_level.clone(),
                            popup.s_surface.clone(),
                        ));
                    }
                }
            }
        }
        None
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

    // TODO cleanup & test thouroughly
    pub fn set_output(&mut self, output: Option<(c_wl_output::WlOutput, String)>) {
        self.output = output;
        let c_surfaces: Vec<_> = self
            .surfaces
            .iter()
            .map(|top_level| (top_level.c_top_level.clone(), top_level.dimensions.clone()))
            .collect();
        let layer_surfaces: Vec<_> = c_surfaces
            .iter()
            .map(|(c_surface, dimensions)| {
                self.get_layer_shell(c_surface.clone(), dimensions.clone())
            })
            .collect();

        for (top_level, (layer_surface, next_render_event, egl_surface)) in
            &mut self.surfaces.iter_mut().zip(layer_surfaces)
        {
            top_level.layer_surface.destroy();

            top_level.next_render_event = next_render_event;
            top_level.layer_surface = layer_surface;
            top_level.egl_surface = egl_surface;
            top_level.dirty = true;
        }
    }

    // TODO cleanup
    fn get_layer_shell(
        &mut self,
        c_surface: Attached<c_wl_surface::WlSurface>,
        dimensions: (u32, u32),
    ) -> (
        Main<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>,
        Rc<Cell<Option<RenderEvent>>>,
        Rc<EGLSurface>,
    ) {
        let layer_surface = self.layer_shell.get_layer_surface(
            &c_surface.clone(),
            self.output.as_ref().map(|o| o.0.clone()).as_ref(),
            self.config.layer.into(),
            "example".to_owned(),
        );

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

            let renderer = unsafe {
                Gles2Renderer::new(egl_context, self.log.clone())
                    .expect("Failed to initialize EGL Surface")
            };
            trace!(self.log, "{:?}", unsafe {
                SwapInterval(egl_display.get_display_handle().handle, 0)
            });
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
        (layer_surface, next_render_event, egl_surface)
    }
}
