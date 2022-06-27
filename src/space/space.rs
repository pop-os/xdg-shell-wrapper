// SPDX-License-Identifier: MPL-2.0-only

use std::{
    cell::{Cell, RefCell},
    os::unix::net::UnixStream,
    rc::Rc,
    time::{Duration, Instant},
};

use crate::{config::WrapperConfig, client_state::Focus};
use sctk::{
    output::OutputInfo,
    reexports::{
        client::{
            self,
            protocol::{wl_output as c_wl_output, wl_surface as c_wl_surface},
            Attached, Main,
        },
        protocols::{
            wlr::unstable::layer_shell::v1::client::zwlr_layer_shell_v1,
            xdg_shell::client::{xdg_positioner::XdgPositioner, xdg_surface::XdgSurface},
        },
    },
    shm::AutoMemPool,
};
use slog::Logger;
use smithay::{
    desktop::{PopupManager, Window, Space},
    reexports::wayland_server::{
        self, protocol::wl_surface::WlSurface as s_WlSurface, Display as s_Display,
    },
    utils::{Logical, Size},
    wayland::shell::xdg::{PopupSurface, PositionerState},
};

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum SpaceEvent {
    WaitConfigure {
        width: u32,
        height: u32,
    },
    Configure {
        width: u32,
        height: u32,
        serial: u32,
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

pub trait WrapperSpace {
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
    fn add_top_level(&mut self, s_top_level: Rc<RefCell<Window>>);

    /// add a popup to the space
    fn add_popup(
        &mut self,
        c_surface: c_wl_surface::WlSurface,
        c_xdg_surface: Main<XdgSurface>,
        s_surface: PopupSurface,
        positioner: Main<XdgPositioner>,
        positioner_state: PositionerState,
        popup_manager: Rc<RefCell<PopupManager>>,
    );

    /// close all popups for the wrapper space
    fn close_popups(&mut self);

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
    fn handle_events(&mut self, time: u32, focus: &Focus) -> Instant;

    /// gets the config
    fn config(&self) -> Self::Config;

    /// spawns the clients for the wrapper
    fn spawn_clients(
        &mut self,
        display: &mut wayland_server::DisplayHandle,
    ) -> anyhow::Result<Vec<(UnixStream, UnixStream)>>;
    fn visibility(&self) -> Visibility;

    /// gets the logger
    fn log(&self) -> Option<Logger>;

    /// cleanup
    fn destroy(&mut self);

    /// gets the space
    fn space(&mut self) -> Space;

    /// Moves an already mapped Window to top of the stack
    /// This function does nothing for unmapped windows.
    /// If activate is true it will set the new windows state to be activate and removes that state from every other mapped window.
    fn raise_window(&mut self, w: Window, active: bool) {}

    /// marks the space as dirtied
    fn dirty(&mut self) {}
}

// // TODO
// impl Drop for WrapperSpace<Config = Any> {
//     fn drop(&mut self) {
//         self.layer_surface.as_mut().map(|ls| ls.destroy());
//         elf.layer_shell_wl_surface.as_mut().map(|wls| wls.destroy());
//     }
// }

#[derive(Debug)]
pub enum Alignment {
    Left,
    Center,
    Right,
}
