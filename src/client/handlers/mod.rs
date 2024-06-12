// SPDX-License-Identifier: MPL-2.0

use sctk::{
    delegate_compositor, delegate_output, delegate_registry, delegate_shm,
    output::OutputState,
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::SeatState,
    shm::{Shm, ShmHandler},
};

use crate::{shared_state::GlobalState, space::WrapperSpace};

pub mod compositor;
pub mod data_device;
pub mod keyboard;
pub mod layer_shell;
/// output helpers
pub mod output;
pub mod pointer;
pub mod seat;
pub mod shell;
pub mod toplevel;
pub mod touch;
pub mod workspace;
pub mod wp_fractional_scaling;
pub mod wp_security_context;
pub mod wp_viewporter;

impl<W: WrapperSpace> ShmHandler for GlobalState<W> {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.client_state.shm_state
    }
}

impl<W: WrapperSpace> ProvidesRegistryState for GlobalState<W> {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.client_state.registry_state
    }
    registry_handlers![OutputState, SeatState,];
}

delegate_registry!(@<W: WrapperSpace + 'static> GlobalState<W>);
delegate_compositor!(@<W: WrapperSpace + 'static> GlobalState<W>);
delegate_output!(@<W: WrapperSpace + 'static> GlobalState<W>);
delegate_shm!(@<W: WrapperSpace + 'static> GlobalState<W>);
