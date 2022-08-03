// SPDX-License-Identifier: MPL-2.0-only

use std::cell::Cell;
use std::rc::Rc;

use crate::space::ClientEglSurface;
use sctk::reexports::client::protocol::wl_display::WlDisplay;
use sctk::reexports::client::{protocol::wl_surface as c_wl_surface, Proxy};
use sctk::reexports::protocols::xdg::shell::client::xdg_popup::XdgPopup;
use sctk::reexports::protocols::xdg::shell::client::xdg_surface::XdgSurface;
use smithay::backend::egl::{EGLContext, EGLDisplay};
use smithay::{
    backend::egl::surface::EGLSurface,
    desktop::PopupManager,
    utils::{Logical, Physical, Rectangle},
    wayland::shell::xdg::PopupSurface,
};
use wayland_egl::WlEglSurface;

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
    pub c_popup: XdgPopup,
    /// the xdg surface for the popup on the layer shell surface
    pub c_xdg_surface: XdgSurface,
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
    pub rectangle: Rectangle<i32, Logical>,
    /// accumulated damage with age values
    pub accumulated_damage: Vec<Vec<Rectangle<i32, Physical>>>,
    /// full clear
    pub full_clear: u8,
}

impl Popup {
    /// Handles any events that have occurred since the last call, redrawing if needed.
    /// Returns true if the surface is alive.
    pub fn handle_events(
        &mut self,
        popup_manager: &mut PopupManager,
        egl_context: &EGLContext,
        egl_display: &EGLDisplay,
        c_display: &WlDisplay,
    ) -> bool {
        let should_keep = {
            if !self.s_surface.alive() {
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
                            let wl_egl_surface =
                                match WlEglSurface::new(self.c_wl_surface.id(), width, height) {
                                    Ok(s) => s,
                                    Err(_) => return false,
                                };
                            let client_egl_surface = ClientEglSurface {
                                wl_egl_surface,
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
                            self.egl_surface
                                .as_ref()
                                .unwrap()
                                .resize(width, height, 0, 0);
                        }
                        popup_manager.commit(self.s_surface.wl_surface());
                        self.dirty = true;
                        self.full_clear = 4;
                        self.rectangle = Rectangle::from_loc_and_size((x, y), (width, height));
                        true
                    }
                    Some(PopupState::WaitConfigure(first)) => {
                        self.popup_state
                            .replace(Some(PopupState::WaitConfigure(first)));
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
