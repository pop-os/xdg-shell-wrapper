// SPDX-License-Identifier: MPL-2.0-only

use std::cell::Cell;
use std::rc::Rc;

use sctk::reexports::{
    client::Main,
    client::protocol::wl_surface as c_wl_surface,
    protocols::xdg_shell::client::{xdg_popup::XdgPopup, xdg_surface::XdgSurface},
};
use sctk::reexports::client::Display;
use smithay::{
    backend::egl::surface::EGLSurface,
    desktop::PopupManager,
    utils::{Logical, Physical, Point, Rectangle},
    wayland::shell::xdg::PopupSurface,
};
use smithay::backend::egl::{EGLContext, EGLDisplay};
use wayland_egl::WlEglSurface;
use crate::space::ClientEglSurface;

/// Popup events
#[derive(PartialEq, Copy, Clone, Debug)]
pub enum PopupState {
    /// Waiting for the configure event for the popup surface
    WaitConfigure(bool),
    /// Configure Event
    Configure {
        /// first configure event
        first: bool,
        /// x position
        x: i32,
        /// y position
        y: i32,
        /// width
        width: i32,
        /// height
        height: i32,
    },
    /// Popup reposition token
    Repositioned(u32),
    /// Popup closed
    Closed,
}

/// Popup
#[derive(Debug)]
pub struct Popup {
    /// the popup on the layer shell surface
    pub c_popup: Main<XdgPopup>,
    /// the xdg surface for the popup on the layer shell surface
    pub c_xdg_surface: Main<XdgSurface>,
    /// the wl surface for the popup on the layer shell surface
    pub c_wl_surface: c_wl_surface::WlSurface,
    /// the embedded popup
    pub s_surface: PopupSurface,
    /// the egl surface
    pub egl_surface: Option<Rc<EGLSurface>>,
    /// the state of the popup
    pub popup_state: Rc<Cell<Option<PopupState>>>,
    /// whether or not the popup needs to be rendered
    pub dirty: bool,
    /// position of the popup
    pub position: Point<i32, Logical>,
    /// accumulated damage with age values
    pub accumulated_damage: Vec<Vec<Rectangle<i32, Physical>>>,
    /// full clear
    pub full_clear: u8,
}

impl Popup {
    /// Handles any events that have occurred since the last call, redrawing if needed.
    /// Returns true if the surface is alive.
    pub fn handle_events(&mut self, popup_manager: &mut PopupManager, egl_context: &EGLContext, egl_display: &EGLDisplay, c_display: &Display, ) -> bool {
        let should_keep = {
            if !self.s_surface.alive() || !self.c_wl_surface.as_ref().is_alive() {
                false
            } else {
                match self.popup_state.take() {
                    Some(PopupState::Closed) => false,
                    Some(PopupState::Configure {
                        first,
                        width,
                        height,
                        x,
                        y,
                    }) => {
                        if first {
                            let client_egl_surface = ClientEglSurface {
                                wl_egl_surface: WlEglSurface::new(
                                    &self.c_wl_surface,
                                    width,
                                    height,
                                ),
                                display: c_display.clone(),
                            };

                            let egl_surface = Rc::new(
                                EGLSurface::new(
                                    &egl_display,
                                        egl_context
                                        .pixel_format()
                                        .expect("Failed to get pixel format from EGL context "),
                                    egl_context.config_id(),
                                    client_egl_surface,
                                    None,
                                )
                                    .expect("Failed to initialize EGL Surface"),
                            );

                            self.egl_surface.replace(egl_surface);
                        } else {
                            self.egl_surface.as_ref().unwrap().resize(width, height, 0, 0);
                        }
                        popup_manager.commit(self.s_surface.wl_surface());
                        self.dirty = true;
                        self.full_clear = 4;
                        self.position = (x, y).into();
                        true
                    }
                    Some(PopupState::WaitConfigure(first)) => {
                        self.popup_state.replace(Some(PopupState::WaitConfigure(first)));
                        true
                    }
                    Some(PopupState::Repositioned(_)) => true,
                    None => true,
                }
            }
        };

        should_keep
    }
}

impl Drop for Popup {
    fn drop(&mut self) {
        self.s_surface.send_popup_done();
        self.c_popup.destroy();
        self.c_xdg_surface.destroy();
        // XXX causes segfault when using nvidia
        // self.c_wl_surface.destroy();
    }
}
