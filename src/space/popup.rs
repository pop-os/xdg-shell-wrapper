// SPDX-License-Identifier: MPL-2.0-only

use std::rc::Rc;

use crate::space::ClientEglSurface;
use sctk::reexports::client::protocol::wl_display::WlDisplay;
use sctk::reexports::client::{protocol::wl_surface as c_wl_surface, Proxy};
use sctk::shell::xdg::popup::Popup;
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
pub enum WrapperPopupState {
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
pub struct WrapperPopup {
    /// the popup on the layer shell surface
    pub c_popup: Popup,
    /// the wl surface for the popup on the layer shell surface
    pub c_wl_surface: c_wl_surface::WlSurface,
    /// the embedded popup
    pub s_surface: PopupSurface,
    /// the egl surface
    pub egl_surface: Option<Rc<EGLSurface>>,
    /// the state of the popup
    pub state: Option<WrapperPopupState>,
    /// whether or not the popup needs to be rendered
    pub dirty: bool,
    /// position of the popup
    pub rectangle: Rectangle<i32, Logical>,
    /// accumulated damage with age values
    pub accumulated_damage: Vec<Vec<Rectangle<i32, Physical>>>,
    /// full clear
    pub full_clear: u8,
}

impl WrapperPopup {
    /// Handles any events that have occurred since the last call, redrawing if needed.
    /// Returns true if the surface is alive.
    pub fn handle_events(
        &mut self,
        _: &mut PopupManager,
        _: &EGLContext,
        _: &EGLDisplay,
        _: &WlDisplay,
    ) -> bool {
        // TODO refactor to do most of this in the space
        self.s_surface.alive()
    }
}

impl Drop for WrapperPopup {
    fn drop(&mut self) {
        self.s_surface.send_popup_done();
        // XXX causes segfault when using nvidia
        // self.c_wl_surface.destroy();
    }
}
