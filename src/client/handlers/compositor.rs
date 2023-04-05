// SPDX-License-Identifier: MPL-2.0

use sctk::{
    compositor::CompositorHandler,
    reexports::client::{protocol::wl_surface, Connection, QueueHandle},
};

use crate::{shared_state::GlobalState, space::WrapperSpace};

impl<W: WrapperSpace> CompositorHandler for GlobalState<W> {
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
        surface: &wl_surface::WlSurface,
        time: u32,
    ) {
        self.space.frame(surface, time);
    }
}
