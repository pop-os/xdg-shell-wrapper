// SPDX-License-Identifier: MPL-2.0-only

use std::{
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};

use sctk::{
    environment::Environment,
    output::OutputInfo,
    reexports::{
        client::{self, protocol::wl_output as c_wl_output, Attached, Main},
        protocols::xdg_shell::client::{xdg_positioner::XdgPositioner, xdg_wm_base::XdgWmBase},
    },
};
use slog::Logger;
use smithay::{
    backend::renderer::gles2::Gles2Renderer,
    desktop::{PopupManager, Space, Window},
    reexports::wayland_server::{
        self, protocol::wl_surface::WlSurface as s_WlSurface, DisplayHandle,
    },
    wayland::shell::xdg::{PopupSurface, PositionerState},
};

use crate::{
    client_state::{ClientFocus, Env},
    config::WrapperConfig,
    server_state::ServerFocus,
    space::Popup,
};

/// Space events
#[derive(PartialEq, Copy, Clone, Debug)]
pub enum SpaceEvent {
    /// waiting for the next configure event
    WaitConfigure {
        /// whether it is waiting for the first configure event
        first: bool,
        /// width
        width: i32,
        /// height
        height: i32,
    },
    /// the next configure event
    Configure {
        /// whether it is the first configure event
        first: bool,
        /// width
        width: i32,
        /// height
        height: i32,
        /// serial
        serial: i32,
    },
    /// the space has been scheduled to cleanup and exit
    Quit,
}

/// Visibility of the space
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Visibility {
    /// hidden
    Hidden,
    /// visible
    Visible,
    /// transitioning to hidden
    TransitionToHidden {
        /// previous instant that was processed
        last_instant: Instant,
        /// duration of the transition progressed
        progress: Duration,
        /// previously calculated value
        prev_margin: i32,
    },
    /// transitioning to visible
    TransitionToVisible {
        /// previous instant that was processed
        last_instant: Instant,
        /// duration of the transition progressed
        progress: Duration,
        /// previously calculated value
        prev_margin: i32,
    },
}

impl Default for Visibility {
    fn default() -> Self {
        Self::Visible
    }
}

/// Wrapper Space
/// manages and renders xdg-shell-window(s) on a layer shell surface
pub trait WrapperSpace {
    /// Wrapper config type
    type Config: WrapperConfig;

    /// initial setup of the space
    fn setup(
        &mut self,
        env: &Environment<Env>,
        c_display: client::Display,
        c_focused_surface: ClientFocus,
        c_hovered_surface: ClientFocus,
        s_focused_surface: ServerFocus,
        s_hovered_surface: ServerFocus,
    );

    /// add the configured output to the space
    fn handle_output(
        &mut self,
        env: &Environment<Env>,
        output: Option<&c_wl_output::WlOutput>,
        output_info: Option<&OutputInfo>,
    ) -> anyhow::Result<()>;

    /// handle pointer motion on the space
    fn update_pointer(&mut self, dim: (i32, i32), seat_name: &str);

    /// handle a button press on a client surface
    fn handle_button(&mut self, seat_name: &str) -> bool;

    /// add a top level window to the space
    fn add_window(&mut self, s_top_level: Window);

    /// add a popup to the space
    fn add_popup(
        &mut self,
        env: &Environment<Env>,
        xdg_surface_state: &Attached<XdgWmBase>,
        s_surface: PopupSurface,
        positioner: Main<XdgPositioner>,
        positioner_state: PositionerState,
    );

    /// keyboard focus lost handler
    fn keyboard_leave(&mut self, seat_name: &str);

    /// keyboard focus gained handler
    fn keyboard_enter(&mut self, seat_name: &str);

    /// pointer focus lost handler
    fn pointer_leave(&mut self, seat_name: &str);

    /// pointer focus gained handler
    fn pointer_enter(&mut self, seat_name: &str);

    /// repositions a popup
    fn reposition_popup(
        &mut self,
        popup: PopupSurface,
        positioner: Main<XdgPositioner>,
        positioner_state: PositionerState,
        token: u32,
    ) -> anyhow::Result<()>;

    /// called in a loop by xdg-shell-wrapper
    /// handles events for the space
    fn handle_events(&mut self, dh: &DisplayHandle, time: u32, focus: &ClientFocus) -> Instant;

    /// gets the config
    fn config(&self) -> Self::Config;

    /// spawns the clients for the wrapper
    fn spawn_clients(
        &mut self,
        display: &mut wayland_server::DisplayHandle,
    ) -> anyhow::Result<Vec<UnixStream>>;

    /// gets visibility of the wrapper
    fn visibility(&self) -> Visibility {
        Visibility::Visible
    }

    /// gets the logger
    fn log(&self) -> Option<Logger>;

    /// cleanup
    fn destroy(&mut self);

    /// gets the space
    fn space(&mut self) -> &mut Space;

    /// Moves an already mapped Window to top of the stack
    /// This function does nothing for unmapped windows.
    /// If activate is true it will set the new windows state to be activate and removes that state from every other mapped window.
    fn raise_window(&mut self, _: &Window, _: bool) {}

    /// marks the window as dirtied
    fn dirty_window(&mut self, dh: &DisplayHandle, w: &s_WlSurface);

    /// marks the popup as dirtied()
    fn dirty_popup(&mut self, dh: &DisplayHandle, w: &s_WlSurface);

    /// gets the popup manager for this space
    fn popup_manager(&mut self) -> &mut PopupManager;

    /// gets the popups
    fn popups(&self) -> Vec<&Popup>;

    /// gets the renderer for the space
    fn renderer(&mut self) -> Option<&mut Gles2Renderer>;

    // gets the z-index for the requested applet
    // fn z_index(&self, applet: &str) -> Option<RenderZindex> {
    //     match self.config().layer(applet) {
    //         Layer::Background => Some(RenderZindex::Background),
    //         Layer::Bottom => Some(RenderZindex::Bottom),
    //         Layer::Top => Some(RenderZindex::Top),
    //         Layer::Overlay => Some(RenderZindex::Overlay),
    //         _ => None,
    //     }
    // }
}
