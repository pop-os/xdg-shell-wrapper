// SPDX-License-Identifier: MPL-2.0-only

use sctk::{
    delegate_compositor, delegate_output, delegate_shm,
    output::OutputState,
    reexports::client::{
        globals::GlobalListContents, protocol::wl_registry, Connection, Dispatch, QueueHandle,
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::SeatState,
    shm::{ShmHandler, ShmState},
};

use crate::{shared_state::GlobalState, space::WrapperSpace};

pub mod compositor;
pub mod keyboard;
pub mod layer_shell;
/// output helpers
pub mod output;
pub mod pointer;
pub mod seat;
pub mod shell;

impl<W: WrapperSpace> ShmHandler for GlobalState<W> {
    fn shm_state(&mut self) -> &mut ShmState {
        &mut self.client_state.shm_state
    }
}

impl<W: WrapperSpace> ProvidesRegistryState for GlobalState<W> {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.client_state.registry_state
    }
    registry_handlers![OutputState, SeatState,];
}

impl<W: WrapperSpace> Dispatch<wl_registry::WlRegistry, GlobalListContents> for GlobalState<W> {
    fn event(
        _state: &mut Self,
        _registry: &wl_registry::WlRegistry,
        _event: wl_registry::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // We don't need any other globals.
    }
}

delegate_compositor!(@<W: WrapperSpace + 'static> GlobalState<W>);
delegate_output!(@<W: WrapperSpace + 'static> GlobalState<W>);
delegate_shm!(@<W: WrapperSpace + 'static> GlobalState<W>);
