// SPDX-License-Identifier: MPL-2.0-only

use sctk::{
    compositor::CompositorState,
    delegate_compositor, delegate_output, delegate_registry, delegate_shm,
    output::OutputState,
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::SeatState,
    shell::{xdg::XdgShellState, layer::LayerState},
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
    registry_handlers![
        CompositorState,
        OutputState,
        ShmState,
        SeatState,
        XdgShellState,
        LayerState
    ];
}

delegate_compositor!(@<W: WrapperSpace + 'static> GlobalState<W>);
delegate_output!(@<W: WrapperSpace + 'static> GlobalState<W>);
delegate_shm!(@<W: WrapperSpace + 'static> GlobalState<W>);
delegate_registry!(@<W: WrapperSpace + 'static> GlobalState<W>);
