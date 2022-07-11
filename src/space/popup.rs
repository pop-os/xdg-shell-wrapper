// SPDX-License-Identifier: MPL-2.0-only

use std::cell::Cell;
use std::rc::Rc;

use sctk::reexports::{
    client::Main,
    client::protocol::wl_surface as c_wl_surface,
    protocols::xdg_shell::client::{xdg_popup::XdgPopup, xdg_surface::XdgSurface},
};
use smithay::{
    backend::egl::surface::EGLSurface,
    desktop::PopupManager,
    utils::{Logical, Physical, Point, Rectangle},
    wayland::shell::xdg::PopupSurface,
};

/// Popup events
#[derive(PartialEq, Copy, Clone, Debug)]
pub enum PopupState {
    /// Waiting for the configure event for the popup surface
    WaitConfigure,
    /// Configure Event
    Configure {
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
    pub egl_surface: Rc<EGLSurface>,
    /// the state of the popup
    pub popup_state: Rc<Cell<Option<PopupState>>>,
    /// whether or not the popup needs to be rendered
    pub dirty: bool,
    /// position of the popup
    pub position: Point<i32, Logical>,
    /// accumulated damage with age values
    pub accumulated_damage: Vec<Vec<Rectangle<i32, Physical>>>,
}

impl Popup {
    /// Handles any events that have occurred since the last call, redrawing if needed.
    /// Returns true if the surface is alive.
    pub fn handle_events(&mut self, popup_manager: &mut PopupManager) -> bool {
        let should_keep = {
            if !self.s_surface.alive() || !self.c_wl_surface.as_ref().is_alive() {
                false
            } else {
                match self.popup_state.take() {
                    Some(PopupState::Closed) => false,
                    Some(PopupState::Configure {
                        width,
                        height,
                        x,
                        y,
                    }) => {
                        self.position = (x, y).into();
                        popup_manager.commit(self.s_surface.wl_surface());
                        self.egl_surface.resize(width, height, 0, 0);
                        self.dirty = true;
                        true
                    }
                    Some(PopupState::WaitConfigure) => {
                        self.popup_state.replace(Some(PopupState::WaitConfigure));
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
        self.c_wl_surface.destroy();
    }
}
