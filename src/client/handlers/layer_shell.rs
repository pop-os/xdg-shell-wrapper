// SPDX-License-Identifier: MPL-2.0-only

use sctk::{delegate_layer, shell::layer::LayerShellHandler};

use crate::{shared_state::GlobalState, space::WrapperSpace};

impl<W: WrapperSpace> LayerShellHandler for GlobalState<W> {
    fn closed(
        &mut self,
        _conn: &sctk::reexports::client::Connection,
        _qh: &sctk::reexports::client::QueueHandle<Self>,
        layer: &sctk::shell::layer::LayerSurface,
    ) {
        self.space.close_layer(layer);
    }

    fn configure(
        &mut self,
        _conn: &sctk::reexports::client::Connection,
        _qh: &sctk::reexports::client::QueueHandle<Self>,
        layer: &sctk::shell::layer::LayerSurface,
        configure: sctk::shell::layer::LayerSurfaceConfigure,
        _serial: u32,
    ) {
        self.space.configure_layer(layer, configure);
    }
}

delegate_layer!(@<W: WrapperSpace + 'static> GlobalState<W>);
