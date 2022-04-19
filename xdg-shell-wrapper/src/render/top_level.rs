// SPDX-License-Identifier: MPL-2.0-only

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use sctk::reexports::{
        client::protocol::wl_surface as c_wl_surface,
        client::{Attached, Main},
    };
use slog::Logger;
use smithay::{
    backend::{
        egl::surface::EGLSurface,
        renderer::{
            gles2::Gles2Renderer, utils::draw_surface_tree, Bind, Frame, Renderer,
            Unbind,
        },
    },
    desktop::{utils::send_frames_surface_tree, Kind},
    reexports::wayland_protocols::wlr::unstable::layer_shell::v1::client::zwlr_layer_surface_v1,

};

use super::{Popup, PopupRenderEvent};

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum RenderEvent {
    WaitConfigure,
    Configure { width: u32, height: u32 },
    Closed,
}

#[derive(Debug, Clone)]
pub struct TopLevelSurface {
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

impl TopLevelSurface {
    /// Handles any events that have occurred since the last call, redrawing if needed.
    /// Returns true if the surface should be dropped.
    pub fn handle_events(&mut self) -> bool {
        if self.s_top_level.borrow().toplevel().get_surface().is_none() {
            return true;
        }
        let mut remove_surface = false;
        // TODO replace with drain_filter when stable

        let mut i = 0;
        while i < self.popups.len() {
            let p = &mut self.popups[i];
            let should_keep = {
                if !p.s_surface.alive() {
                    false
                }
                else {
                    match p.next_render_event.take() {
                        Some(PopupRenderEvent::Closed) => false,
                        Some(PopupRenderEvent::Configure { width, height, .. }) => {
                            p.egl_surface.resize(width, height, 0, 0);
                            p.bbox.size = (width, height).into();
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
                }
            };

            if !should_keep {
                let _ = self.popups.remove(i);
                // your code here
            } else {
                i += 1;
            }
        }

        match self.next_render_event.take() {
            Some(RenderEvent::Closed) => {
                remove_surface = true;
            }
            Some(RenderEvent::Configure { width, height }) => {
                if self.dimensions != (width, height) {
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
            };

            let width = self.dimensions.0 as i32;
            let height = self.dimensions.1 as i32;

            let loc = self.s_top_level.borrow().bbox().loc;

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
                            loc: loc.clone().into(),
                            size: (width, height).into(),
                        };

                        frame
                            .clear(
                                [1.0, 1.0, 1.0, 1.0],
                                &[smithay::utils::Rectangle::<f64, smithay::utils::Logical> {
                                    loc: (loc.x as f64, loc.y as f64).into(),
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
                loc: (0,0).into(),
                size: (width, height).into(),
            }];

            egl_surface
                .swap_buffers(Some(&mut damage))
                .expect("Failed to swap buffers.");

            send_frames_surface_tree(server_surface, time);
        }
        // render popups
        for p in &mut self.popups {
            if !p.dirty || !p.s_surface.alive() || p.next_render_event.get() != None {
                continue;
            }
            p.dirty = false;
            let wl_surface = match p.s_surface.get_surface() {
                Some(s) => s,
                _ => return,
            };
            
            let (width, height) = p.bbox.size.into();
            let loc = p.bbox.loc;
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
                            loc: loc.clone().into(),
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


impl Drop for TopLevelSurface {
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