// SPDX-License-Identifier: MPL-2.0-only

use std::rc::Rc;

use sctk::compositor::Region;
use sctk::reexports::client::protocol::wl_display::WlDisplay;
use sctk::reexports::client::protocol::wl_surface as c_wl_surface;
use sctk::shell::xdg::popup::Popup;
use smithay::backend::egl::{EGLContext, EGLDisplay};
use smithay::{
    backend::egl::surface::EGLSurface,
    desktop::PopupManager,
    utils::{Logical, Physical, Rectangle},
    wayland::shell::xdg::PopupSurface,
};

/// Popup events
#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum WrapperPopupState {
    /// Wait for configure event to render
    WaitConfigure,
    /// Configure Event
    Rectangle {
        /// x position
        x: i32,
        /// y position
        y: i32,
        /// width
        width: i32,
        /// height
        height: i32,
    },
}

/// Popup
#[derive(Debug)]
pub struct WrapperPopup {
    // XXX implicitly drops egl_surface first to avoid segfault
    /// the egl surface
    pub egl_surface: Option<Rc<EGLSurface>>,

    /// the popup on the layer shell surface
    pub c_popup: Popup,
    /// the wl surface for the popup on the layer shell surface
    pub c_wl_surface: c_wl_surface::WlSurface,
    /// the embedded popup
    pub s_surface: PopupSurface,
    /// the state of the popup
    pub state: Option<WrapperPopupState>,
    /// whether or not the popup needs to be rendered
    pub dirty: bool,
    /// full rectangle of the inner popup, including dropshadow borders
    pub rectangle: Rectangle<i32, Logical>,
    /// accumulated damage with age values
    pub accumulated_damage: Vec<Vec<Rectangle<i32, Physical>>>,
    /// full clear
    pub full_clear: u8,
    /// input region for the popup
    pub input_region: Region,
    /// location of the popup wrapper
    pub wrapper_rectangle: Rectangle<i32, Logical>,
}

impl WrapperPopup {
    /// Handles any events that have occurred since the last call, redrawing if needed.
    /// Returns true if the surface is alive.
    pub fn handle_events(
        &mut self,
        popup_manager: &mut PopupManager,
        _: &EGLContext,
        _: &EGLDisplay,
        _: &WlDisplay,
    ) -> bool {
        if let Some(WrapperPopupState::Rectangle {
            width,
            height,
            x,
            y,
        }) = self.state
        {
            self.egl_surface
                .as_ref()
                .unwrap()
                .resize(width, height, 0, 0);
            popup_manager.commit(self.s_surface.wl_surface());
            self.dirty = true;
            self.full_clear = 4;
            self.rectangle = Rectangle::from_loc_and_size((x, y), (width, height));
            self.state.take();
        };
        self.s_surface.alive()
    }
}
