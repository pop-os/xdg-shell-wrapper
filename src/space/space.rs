// SPDX-License-Identifier: MPL-2.0-only

use std::{
    cell::{Cell, RefCell},
    os::unix::net::UnixStream,
    rc::Rc,
    time::{Duration, Instant},
};

use sctk::{
    environment::Environment,
    output::OutputInfo,
    reexports::{
        client::{
            self,
            Attached,
            Main, protocol::{wl_output as c_wl_output, wl_surface as c_wl_surface},
        },
        protocols::{
            wlr::unstable::layer_shell::v1::client::zwlr_layer_shell_v1::{self, Layer},
            xdg_shell::client::{xdg_positioner::XdgPositioner, xdg_wm_base::XdgWmBase},
        },
    },
    shm::AutoMemPool,
};
use slog::Logger;
use smithay::{
    backend::renderer::gles2::Gles2Renderer,
    desktop::{PopupManager, Space, Window},
    reexports::wayland_server::{
        self, DisplayHandle, protocol::wl_surface::WlSurface as s_WlSurface,
    },
    wayland::shell::xdg::{PopupSurface, PositionerState},
};

use crate::{
    client_state::{Env, Focus},
    config::WrapperConfig,
    space::Popup,
};

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum SpaceEvent {
    WaitConfigure {
        first: bool,
        width: i32,
        height: i32,
    },
    Configure {
        first: bool,
        width: i32,
        height: i32,
        serial: i32,
    },
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Visibility {
    Hidden,
    Visible,
    TransitionToHidden {
        last_instant: Instant,
        progress: Duration,
        prev_margin: i32,
    },
    TransitionToVisible {
        last_instant: Instant,
        progress: Duration,
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

    /// add the configured output to the space
    fn add_output(
        &mut self,
        output: Option<&c_wl_output::WlOutput>,
        output_info: Option<&OutputInfo>,
        pool: AutoMemPool,
        c_display: client::Display,
        layer_shell: Attached<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
        log: Logger,
        c_surface: Attached<c_wl_surface::WlSurface>,
        focused_surface: Rc<RefCell<Option<s_WlSurface>>>,
    ) -> anyhow::Result<()>;

    /// handle pointer motion on the space
    fn update_pointer(&mut self, dim: (i32, i32));

    /// handle a button press on a client surface
    fn handle_button(&mut self, c_focused_surface: &c_wl_surface::WlSurface);

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
    fn keyboard_focus_lost(&mut self);

    /// accesses the next event for the space
    fn next_space_event(&self) -> Rc<Cell<Option<SpaceEvent>>>;

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
    fn handle_events(&mut self, dh: &DisplayHandle, time: u32, focus: &Focus) -> Instant;

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
