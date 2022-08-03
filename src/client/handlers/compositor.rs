// SPDX-License-Identifier: MPL-2.0-only

use sctk::{
    compositor::{CompositorHandler, CompositorState},
    reexports::client::{protocol::wl_surface, Connection, QueueHandle},
};

use crate::{shared_state::GlobalState, space::WrapperSpace};

impl<W: WrapperSpace> CompositorHandler for GlobalState<W> {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.client_state.compositor_state
    }

    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
    }
}
