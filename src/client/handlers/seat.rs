// SPDX-License-Identifier: MPL-2.0-only

use sctk::{
    reexports::client::{
        protocol::wl_seat,
        Connection, QueueHandle,
    },
    seat::SeatHandler, delegate_seat,
};

use crate::{
    shared_state::GlobalState,
    space::WrapperSpace,
};


impl<W: WrapperSpace> SeatHandler for GlobalState<W> {
    fn seat_state(&mut self) -> &mut sctk::seat::SeatState {
        todo!()
    }

    fn new_seat(&mut self, conn: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat) {
        todo!()
    }

    fn new_capability(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: sctk::seat::Capability,
    ) {
        todo!()
    }

    fn remove_capability(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: sctk::seat::Capability,
    ) {
        todo!()
    }

    fn remove_seat(&mut self, conn: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat) {
        todo!()
    }
}

delegate_seat!(@<W: WrapperSpace + 'static> GlobalState<W>);
