use smithay::{delegate_data_device, delegate_output, delegate_seat, wayland::{seat::{SeatHandler, SeatState}, data_device::{DataDeviceHandler, ClientDndGrabHandler, ServerDndGrabHandler}}};

use crate::{space::WrapperSpace, shared_state::GlobalState};

pub(crate) mod compositor;
pub(crate) mod xdg_shell;

//
// Wl Seat
//

impl<W: WrapperSpace> SeatHandler for GlobalState<W> {
    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.embedded_server_state.seat_state
    }
}

delegate_seat!(@<W: WrapperSpace + 'static> GlobalState<W>);

//
// Wl Data Device
//

impl<W: WrapperSpace> DataDeviceHandler for GlobalState<W> {
    fn data_device_state(&self) -> &smithay::wayland::data_device::DataDeviceState {
        &self.embedded_server_state.data_device_state
    }
}

impl<W: WrapperSpace> ClientDndGrabHandler for GlobalState<W> {}
impl<W: WrapperSpace> ServerDndGrabHandler for GlobalState<W> {}

delegate_data_device!(@<W: WrapperSpace + 'static> GlobalState<W>);

//
// Wl Output
//

delegate_output!(@<W: WrapperSpace + 'static> GlobalState<W>);
