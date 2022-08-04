// SPDX-License-Identifier: MPL-2.0-only

use sctk::{
    reexports::client::{
        protocol::wl_seat,
        Connection, QueueHandle,
    },
    seat::SeatHandler, delegate_seat,
};
use smithay::wayland::seat::Seat;

use crate::{
    shared_state::GlobalState,
    space::WrapperSpace, server_state::SeatPair, client_state::ClientSeat,
};


impl<W: WrapperSpace> SeatHandler for GlobalState<W> {
    fn seat_state(&mut self) -> &mut sctk::seat::SeatState {
        &mut self.client_state.seat_state
    }

    fn new_seat(&mut self, _conn: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat) {
        if let Some(info) =  self.client_state.seat_state.info(&seat) {
            let name = info.name.unwrap_or_default();
            
            let mut new_server_seat = Seat::new(&self.server_state.display_handle, name.clone(), self.log.clone());
            let kbd = if info.has_keyboard {
                if let Ok(kbd) = self.client_state.seat_state.get_keyboard(qh, &seat, None) {
                    let _ = new_server_seat.add_keyboard(Default::default(), 200, 20, |_, _| {});
                    Some(kbd)
                } else {
                    None
                }
            } else {
                None
            };

            let ptr = if info.has_pointer {
                if let Ok(ptr) = self.client_state.seat_state.get_pointer(qh, &seat) {
                    new_server_seat.add_pointer(|_| {});
                    Some(ptr)
                } else {
                    None
                }
            } else {
                None
            };
        
            self.server_state.seats.push(SeatPair {
                name: name,
                client: ClientSeat {
                    _seat: seat.clone(),
                    kbd,
                    ptr,
                    // TODO forward touch
                },
                server: new_server_seat,
            });
        }
    }

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: sctk::seat::Capability,
    ) {
        let info = if let Some(info) =  self.client_state.seat_state.info(&seat) {
            info
        } else {
            return ;
        };
        let sp = if let Some(sp) = self.server_state.seats.iter_mut().find(|sp| sp.client._seat == seat) {
            sp
        } else {
            return
        };
        match capability {
            sctk::seat::Capability::Keyboard => {
                if info.has_keyboard {
                    if let Ok(kbd) = self.client_state.seat_state.get_keyboard(qh, &seat, None) {
                        let _ = sp.server.add_keyboard(Default::default(), 200, 20, |_, _| {});
                        sp.client.kbd.replace(kbd);
                    } 
                }
            },
            sctk::seat::Capability::Pointer => {
                if info.has_pointer {
                    if let Ok(ptr) = self.client_state.seat_state.get_pointer(qh, &seat) {
                        sp.server.add_pointer(|_| {});
                        sp.client.ptr.replace(ptr);
                    } 
                }
            },
            sctk::seat::Capability::Touch => {}, // TODO 
            _ => unimplemented!(),
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: sctk::seat::Capability,
    ) {
        let sp = if let Some(sp) = self.server_state.seats.iter_mut().find(|sp| sp.client._seat == seat) {
            sp
        } else {
            return
        };
        match capability {
            sctk::seat::Capability::Keyboard => {
                sp.server.remove_keyboard();
            },
            sctk::seat::Capability::Pointer => {
                sp.server.remove_pointer();
            },
            sctk::seat::Capability::Touch => {}, // TODO 
            _ => unimplemented!(),
        }    }

    fn remove_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, seat: wl_seat::WlSeat) {
        let _ = if let Some(sp_i) = self.server_state.seats.iter().position(|sp| sp.client._seat == seat) {
            self.server_state.seats.swap_remove(sp_i)
        } else {
            return
        };
    }
}

delegate_seat!(@<W: WrapperSpace + 'static> GlobalState<W>);
