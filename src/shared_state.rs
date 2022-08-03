// SPDX-License-Identifier: MPL-2.0-only

use sctk::reexports::client::protocol::wl_output as c_wl_output;
use slog::Logger;
use smithay::{
    backend::renderer::{ImportDma, ImportEgl},
    reexports::wayland_server::{backend::GlobalId, DisplayHandle},
    wayland::dmabuf::DmabufState,
};
use smithay::{
    reexports::wayland_server::protocol::wl_pointer::AxisSource,
    wayland::{output::Output, seat},
};

use crate::client_state::ClientState;
use crate::server_state::ServerState;
use crate::space::WrapperSpace;

/// group of info for an output
pub type OutputGroup = (Output, GlobalId, String, c_wl_output::WlOutput);

/// axis frame date
#[derive(Debug, Default)]
pub(crate) struct AxisFrameData {
    pub(crate) seat_name: String,
    pub(crate) frame: Option<seat::AxisFrame>,
    pub(crate) source: Option<AxisSource>,
    pub(crate) h_discrete: Option<i32>,
    pub(crate) v_discrete: Option<i32>,
}

/// the  global state for the embedded server state
#[allow(missing_debug_implementations)]
pub struct GlobalState<W: WrapperSpace + 'static> {
    /// the implemented space
    pub space: W,
    /// desktop client state
    pub client_state: ClientState<W>,
    /// embedded server state
    pub server_state: ServerState<W>,
    /// instant that the panel was started
    pub start_time: std::time::Instant,
    /// panel logger
    pub log: Logger,
}

impl<W: WrapperSpace + 'static> GlobalState<W> {
    pub(crate) fn new(
        client_state: ClientState<W>,
        server_state: ServerState<W>,
        space: W,
        start_time: std::time::Instant,
        log: Logger,
    ) -> Self {
        let global_state = Self {
            space,
            client_state,
            server_state,
            start_time,
            log,
        };

        global_state
    }

    pub fn setup(&mut self) {
        /*
         * Keyboard initialization
         */

        // first process already existing seats
        // for seat in env.get_all_seats() {
        //     if let Some((has_kbd, has_ptr, name)) = sctk::seat::with_seat_data(&seat, |seat_data| {
        //         (
        //             seat_data.has_keyboard && !seat_data.defunct,
        //             seat_data.has_pointer && !seat_data.defunct,
        //             seat_data.name.clone(),
        //         )
        //     }) {
        //         let mut new_seat = SeatPair {
        //             name: name.clone(),
        //             server: seat::Seat::new(dh, name.clone(), log.clone()),
        //             client: ClientSeat {
        //                 kbd: None,
        //                 ptr: None,
        //                 _seat: seat.clone(),
        //             },
        //         };
        //         if has_kbd || has_ptr {
        //             if has_kbd {
        //                 let seat_name = name.clone();
        //                 let kbd = seat.get_keyboard();
        //                 kbd.quick_assign(move |_, event, dispatch_data| {
        //                     send_keyboard_event::<W>(event, &seat_name, dispatch_data)
        //                 });
        //                 new_seat.client.kbd = Some(kbd.detach());
        //                 new_seat.server.add_keyboard(
        //                     Default::default(),
        //                     200,
        //                     20,
        //                     move |_seat, _focus| {},
        //                 )?;
        //             }
        //             if has_ptr {
        //                 let seat_name = name.clone();
        //                 let pointer = seat.get_pointer();
        //                 pointer.quick_assign(move |_, event, dispatch_data| {
        //                     send_pointer_event::<W>(event, &seat_name, dispatch_data)
        //                 });
        //                 new_seat.client.ptr = Some(pointer.detach());
        //                 new_seat.server.add_pointer(move |_new_status| {});
        //             }
        //         }
        //         embedded_server_state.seats.push(new_seat);
        //     }
        // }

        // // TODO reimplement when sctk 0.30 is ready
        // // FIXME focus lost after drop from source outside xdg-shell-wrapper
        // // dnd listener
        // let last_motion = Rc::new(RefCell::new(None));
        // let _ = env.set_data_device_callback(move |seat, dnd_event, mut dispatch_data| {
        //     let (state, _) = dispatch_data
        //         .get::<(GlobalState<W>, wayland_server::Display)>()
        //         .unwrap();
        //     let DesktopClientState {
        //         seats,
        //         env_handle,
        //         space,
        //         ..
        //     } = &mut state.desktop_client_state;

        //     let EmbeddedServerState {
        //         focused_surface,
        //         last_button,
        //         ..
        //     } = &state.embedded_server_state;

        //     if let (Some(last_button), Some(seat)) =
        //         (last_button, seats.iter().find(|s| *(s.client.seat) == seat))
        //     {
        //         match dnd_event {
        //             sctk::data_device::DndEvent::Enter {
        //                 offer,
        //                 serial: _,
        //                 surface,
        //                 x,
        //                 y,
        //             } => {
        //                 let client = focused_surface
        //                     .borrow()
        //                     .as_ref()
        //                     .and_then(|focused_surface| {
        //                         let res = focused_surface.as_ref();
        //                         res.client()
        //                     });
        //                 set_data_device_focus(&seat.server.0, client);

        //                 set_focused_surface(focused_surface, space, &surface, x, y);
        //                 let offer = match offer {
        //                     Some(o) => o,
        //                     None => return,
        //                 };

        //                 let mime_types = offer.with_mime_types(|mime_types| Vec::from(mime_types));

        //                 offer.accept(mime_types.get(0).cloned());
        //                 let seat_clone = seat.client.seat.clone();
        //                 let env_clone = env_handle.clone();
        //                 start_dnd(
        //                     &seat.server.0,
        //                     SERIAL_COUNTER.next_serial(),
        //                     seat::PointerGrabStartData {
        //                         focus: focused_surface
        //                             .borrow()
        //                             .as_ref()
        //                             .map(|s| (s.clone(), (0, 0).into())),
        //                         button: *last_button,
        //                         location: (x, y).into(),
        //                     },
        //                     SourceMetadata {
        //                         mime_types: mime_types.clone(),
        //                         dnd_action: DndAction::from_raw(offer.get_available_actions().to_raw())
        //                             .unwrap(),
        //                     },
        //                     move |server_dnd_event| match server_dnd_event {
        //                         smithay::wayland::data_device::ServerDndEvent::Action(action) => {
        //                             let _ = env_clone.with_data_device(&seat_clone, |device| {
        //                                 device.with_dnd(|offer| {
        //                                     if let Some(offer) = offer {
        //                                         let action =
        //                                             data_device::DndAction::from_raw(action.to_raw())
        //                                                 .unwrap();
        //                                         offer.set_actions(action, action);
        //                                     }
        //                                 });
        //                             });
        //                         }
        //                         smithay::wayland::data_device::ServerDndEvent::Dropped => {}
        //                         smithay::wayland::data_device::ServerDndEvent::Cancelled => {
        //                             let _ = env_clone.with_data_device(&seat_clone, |device| {
        //                                 device.with_dnd(|offer| {
        //                                     if let Some(offer) = offer {
        //                                         offer.finish();
        //                                     }
        //                                 });
        //                             });
        //                         }
        //                         smithay::wayland::data_device::ServerDndEvent::Send {
        //                             mime_type,
        //                             fd,
        //                         } => {
        //                             if mime_types.contains(&mime_type) {
        //                                 let _ = env_clone.with_data_device(&seat_clone, |device| {
        //                                     device.with_dnd(|offer| {
        //                                         if let Some(offer) = offer {
        //                                             unsafe { offer.receive_to_fd(mime_type, fd) };
        //                                         }
        //                                     });
        //                                 });
        //                             }
        //                         }
        //                         smithay::wayland::data_device::ServerDndEvent::Finished => {
        //                             // println!("finished");
        //                             let _ = env_clone.with_data_device(&seat_clone, |device| {
        //                                 device.with_dnd(|offer| {
        //                                     if let Some(offer) = offer {
        //                                         offer.finish();
        //                                     }
        //                                 });
        //                             });
        //                         }
        //                     },
        //                 )
        //             }
        //             sctk::data_device::DndEvent::Motion {
        //                 offer: _,
        //                 time,
        //                 x,
        //                 y,
        //             } => {
        //                 last_motion.replace(Some(((x, y), time)));
        //                 space.update_pointer((x as i32, y as i32));

        //                 handle_motion(
        //                     space,
        //                     focused_surface.borrow().clone(),
        //                     x,
        //                     y,
        //                     seat.server.0.get_pointer().unwrap(),
        //                     time,
        //                 );
        //             }
        //             sctk::data_device::DndEvent::Leave => {}
        //             sctk::data_device::DndEvent::Drop { .. } => {
        //                 if let Some(((_, _), time)) = last_motion.take() {
        //                     seat.server.0.get_pointer().unwrap().button(
        //                         *last_button,
        //                         ButtonState::Released,
        //                         SERIAL_COUNTER.next_serial(),
        //                         time + 1,
        //                     );
        //                 }
        //             }
        //         }
        //     }
        // });

        // set server device selection when offer should be available
        // event_loop.insert_idle(move |(state, _)| {
        //     let seats = &mut state.desktop_client_state.seats;
        //     for s in seats {
        //         let _ = set_server_device_selection(
        //             &env_handle,
        //             &s.client.seat,
        //             &s.server,
        //             &state.embedded_server_state.selected_data_provider.seat,
        //         );
        //     }
        // });
    }
}

impl<W: WrapperSpace + 'static> GlobalState<W> {
    /// bind the display for the space
    pub fn bind_display(&mut self, dh: &DisplayHandle) {
        if let Some(renderer) = self.space.renderer() {
            let res = renderer.bind_wl_display(dh);
            if let Err(err) = res {
                slog::error!(self.log.clone(), "{:?}", err);
            } else {
                let dmabuf_formats = renderer.dmabuf_formats().cloned().collect::<Vec<_>>();
                let mut state = DmabufState::new();
                let global =
                    state.create_global::<GlobalState<W>, _>(dh, dmabuf_formats, self.log.clone());
                self.server_state.dmabuf_state.replace((state, global));
            }
        }
    }
}

// TODO
#[derive(Debug)]
pub(crate) struct SelectedDataProvider {
    //     pub(crate) _seat: Rc<RefCell<Option<Attached<c_wl_seat::WlSeat>>>>,
    //     pub(crate) env_handle: Rc<OnceCell<Environment<Env>>>,
}
