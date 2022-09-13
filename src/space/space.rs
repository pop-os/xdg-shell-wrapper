// SPDX-License-Identifier: MPL-2.0-only

use std::{
    cell::RefCell,
    os::unix::net::UnixStream,
    rc::Rc,
    time::{Duration, Instant},
};

use sctk::{
    compositor::CompositorState,
    output::OutputInfo,
    reexports::client::{
        protocol::{wl_output as c_wl_output, wl_surface},
        Connection, QueueHandle,
    },
    shell::{
        layer::{LayerState, LayerSurface, LayerSurfaceConfigure},
        xdg::{XdgPositioner, XdgShellState},
    },
};
use slog::Logger;
use smithay::{
    output::Output,
    backend::renderer::gles2::Gles2Renderer,
    desktop::{PopupManager, Window},
    reexports::wayland_server::{
        self, protocol::wl_surface::WlSurface as s_WlSurface, DisplayHandle,
    },
    wayland::{
        shell::xdg::{PopupSurface, PositionerState},
    },
};

use crate::{
    client_state::ClientFocus, config::WrapperConfig, server_state::ServerPointerFocus,
    shared_state::GlobalState,
};

/// Space events
#[derive(Eq, PartialEq, Copy, Clone, Debug)]
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
    /// the space has been scheduled to cleanup and exit
    Quit,
}

/// Visibility of the space
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
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

// TODO break this trait into several traits so that it can be better organized
// not all "space" implementations really need all of these exact methods as long as they are wrapped by a space that does
// see cosmic-panel for an example

/// Wrapper Space
/// manages and renders xdg-shell-window(s) on a layer shell surface
pub trait WrapperSpace {
    /// Wrapper config type
    type Config: WrapperConfig;

    /// set the display handle of the space
    fn set_display_handle(&mut self, display: wayland_server::DisplayHandle);

    /// get the client hovered surface of the space
    fn get_client_hovered_surface(&self) -> Rc<RefCell<ClientFocus>>;

    /// get the client focused surface of the space
    fn get_client_focused_surface(&self) -> Rc<RefCell<ClientFocus>>;

    /// setup of the space after the wayland connection is ready
    fn setup<W: WrapperSpace>(
        &mut self,
        compositor_state: &CompositorState,
        layer_state: &mut LayerState,
        conn: &Connection,
        qh: &QueueHandle<GlobalState<W>>,
    );

    /// add the configured output to the space
    fn handle_output<W: WrapperSpace>(
        &mut self,
        compositor_state: &CompositorState,
        layer_state: &mut LayerState,
        conn: &Connection,
        qh: &QueueHandle<GlobalState<W>>,
        c_output: Option<c_wl_output::WlOutput>,
        s_output: Option<Output>,
        info: Option<OutputInfo>,
    ) -> anyhow::Result<()>;

    /// remove the configured output from the space
    fn output_leave(
        &mut self,
        c_output: Option<c_wl_output::WlOutput>,
        s_output: Option<Output>,
        info: Option<OutputInfo>,
    ) -> anyhow::Result<()>;

    /// handle pointer motion on the space
    fn update_pointer(
        &mut self,
        dim: (i32, i32),
        seat_name: &str,
        surface: wl_surface::WlSurface,
    ) -> Option<ServerPointerFocus>;

    /// add a top level window to the space
    fn add_window(&mut self, s_top_level: Window);

    /// add a popup to the space
    fn add_popup<W: WrapperSpace>(
        &mut self,
        compositor_state: &CompositorState,
        conn: &Connection,
        qh: &QueueHandle<GlobalState<W>>,
        xdg_shell_state: &mut XdgShellState,
        s_surface: PopupSurface,
        positioner: &XdgPositioner,
        positioner_state: PositionerState,
    ) -> anyhow::Result<()>;

    /// handle a button press on a client surface
    /// optionally returns a pressed server wl surface
    fn handle_press(&mut self, seat_name: &str) -> Option<s_WlSurface>;

    /// keyboard focus lost handler
    fn keyboard_leave(&mut self, seat_name: &str, surface: Option<wl_surface::WlSurface>);

    /// keyboard focus gained handler
    /// optionally returns a focused server wl surface
    fn keyboard_enter(
        &mut self,
        seat_name: &str,
        surface: wl_surface::WlSurface,
    ) -> Option<s_WlSurface>;

    /// pointer focus lost handler
    fn pointer_leave(&mut self, seat_name: &str, surface: Option<wl_surface::WlSurface>);

    /// pointer focus gained handler
    fn pointer_enter(
        &mut self,
        dim: (i32, i32),
        seat_name: &str,
        surface: wl_surface::WlSurface,
    ) -> Option<ServerPointerFocus>;

    /// repositions a popup
    fn reposition_popup(
        &mut self,
        popup: PopupSurface,
        positioner: &XdgPositioner,
        positioner_state: PositionerState,
        token: u32,
    ) -> anyhow::Result<()>;

    /// called in a loop by xdg-shell-wrapper
    /// handles events for the space
    fn handle_events(
        &mut self,
        dh: &DisplayHandle,
        popup_manager: &mut PopupManager,
        time: u32,
    ) -> Instant;

    /// gets the config
    fn config(&self) -> Self::Config;

    /// spawns the clients for the wrapper
    fn spawn_clients(
        &mut self,
        display: wayland_server::DisplayHandle,
    ) -> anyhow::Result<Vec<UnixStream>>;

    /// gets visibility of the wrapper
    fn visibility(&self) -> Visibility {
        Visibility::Visible
    }

    /// gets the logger
    fn log(&self) -> Option<Logger>;

    /// cleanup
    fn destroy(&mut self);

    /// Moves an already mapped Window to top of the stack
    /// This function does nothing for unmapped windows.
    /// If activate is true it will set the new windows state to be activate and removes that state from every other mapped window.
    fn raise_window(&mut self, _: &Window, _: bool) {}

    /// marks the window as dirtied
    fn dirty_window(&mut self, dh: &DisplayHandle, w: &s_WlSurface);

    /// marks the popup as dirtied()
    fn dirty_popup(&mut self, dh: &DisplayHandle, w: &s_WlSurface);

    /// configure popup
    fn configure_popup(
        &mut self,
        popup: &sctk::shell::xdg::popup::Popup,
        config: sctk::shell::xdg::popup::PopupConfigure,
    );

    /// finished popup
    fn close_popup(&mut self, popup: &sctk::shell::xdg::popup::Popup);

    /// configure layer
    fn configure_layer(&mut self, layer: &LayerSurface, configure: LayerSurfaceConfigure);

    /// close layer
    fn close_layer(&mut self, layer: &LayerSurface);

    /// gets the renderer for the space
    fn renderer(&mut self) -> Option<&mut Gles2Renderer>;
}
