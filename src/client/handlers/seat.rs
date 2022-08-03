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

// pub fn send_keyboard_event<W: WrapperSpace + 'static>(
//     event: wl_keyboard::Event,
//     seat_name: &str,
//     mut dispatch_data: DispatchData<'_>,
// ) {
//     let (state, server_display) = dispatch_data
//         .get::<(GlobalState<W>, Display<GlobalState<W>>)>()
//         .unwrap();
//     let dh = server_display.handle();
//     let space = &mut state.space;
//     let ClientState {
//         last_input_serial,
//         focused_surface: c_focused_surface,
//         ..
//     } = &mut state.client_state;
//     let ServerState { seats, .. } = &mut state.server_state;
//     if let Some(seat) = seats.iter().find(|SeatPair { name, .. }| name == seat_name) {
//         let kbd = match seat.server.get_keyboard() {
//             Some(kbd) => kbd,
//             None => {
//                 error!(
//                     state.log,
//                     "Received keyboard event on {} without keyboard.", &seat_name
//                 );
//                 return;
//             }
//         };
//         match event {
//             wl_keyboard::Event::Key {
//                 serial,
//                 time,
//                 key,
//                 state,
//             } => {
//                 last_input_serial.replace(serial);
//                 let state = match state {
//                     client::protocol::wl_keyboard::KeyState::Pressed => KeyState::Pressed,
//                     client::protocol::wl_keyboard::KeyState::Released => KeyState::Released,
//                     _ => return,
//                 };
//                 match kbd.input::<(), _>(
//                     &dh,
//                     key,
//                     state,
//                     SERIAL_COUNTER.next_serial(),
//                     time,
//                     move |_modifiers, keysym| {
//                         if keysym.modified_sym() == XKB_KEY_Escape && state == KeyState::Released {
//                             FilterResult::Intercept(())
//                         } else {
//                             FilterResult::Forward
//                         }
//                     },
//                 ) {
//                     Some(_) => {
//                         space.keyboard_leave(seat_name, None);
//                         kbd.set_focus(&dh, None, SERIAL_COUNTER.next_serial());
//                     }
//                     None => {}
//                 }
//             }
//             wl_keyboard::Event::RepeatInfo { rate, delay } => {
//                 kbd.change_repeat_info(rate, delay);
//             }
//             wl_keyboard::Event::Enter { surface, .. } => {
//                 // TODO data device
//                 // let _ = set_data_device_selection(
//                 //     dh,
//                 //     &seat.client.seat,
//                 //     &seat.server,
//                 //     &selected_data_provider.seat,
//                 // );
//                 // let _client = focused_surface
//                 //     .borrow()
//                 //     .as_ref()
//                 //     .and_then(|focused_surface| focused_surface.client_id());

//                 // TODO data device
//                 // set_data_device_focus(&seat.server.0, client);

//                 {
//                     let mut c_focused_surface = c_focused_surface.borrow_mut();
//                     if let Some(i) = c_focused_surface.iter().position(|f| f.1 == seat_name) {
//                         c_focused_surface[i].0 = surface.clone();
//                         c_focused_surface[i].2 = FocusStatus::Focused;
//                     } else {
//                         c_focused_surface.push((
//                             surface.clone(),
//                             seat_name.to_string(),
//                             FocusStatus::Focused,
//                         ));
//                     }
//                 }

//                 let s = space.keyboard_enter(seat_name, surface);

//                 kbd.set_focus(&dh, s.as_ref(), SERIAL_COUNTER.next_serial());
//             }
//             wl_keyboard::Event::Leave { surface, .. } => {
//                 let kbd_focus = {
//                     let mut c_focused_surface = c_focused_surface.borrow_mut();
//                     if let Some(i) = c_focused_surface.iter().position(|f| f.0 == surface) {
//                         c_focused_surface[i].2 = FocusStatus::LastFocused(Instant::now());
//                         true
//                     } else {
//                         false
//                     }
//                 };
//                 if kbd_focus {
//                     space.keyboard_leave(seat_name, Some(surface));
//                     kbd.set_focus(&dh, None, SERIAL_COUNTER.next_serial());
//                 }
//             }
//             _ => (),
//         };
//     }
// }

// pub fn send_pointer_event<W: WrapperSpace + 'static>(
//     event: c_wl_pointer::Event,
//     seat_name: &str,
//     mut dispatch_data: DispatchData<'_>,
// ) {
//     let (global_state, server_display) = dispatch_data
//         .get::<(GlobalState<W>, Display<GlobalState<W>>)>()
//         .unwrap();
//     let dh = server_display.handle();
//     let space = &mut global_state.space;
//     let ClientState {
//         axis_frame,
//         last_input_serial,
//         hovered_surface: c_hovered_surface,
//         ..
//     } = &mut global_state.client_state;
//     let ServerState {
//         seats, last_button, ..
//     } = &mut global_state.server_state;
//     let start_time = global_state.start_time;
//     let time = start_time.elapsed().as_millis();
//     if let Some((Some(ptr), kbd)) = seats
//         .iter()
//         .position(|SeatPair { name, .. }| name == seat_name)
//         .map(|idx| &seats[idx])
//         .map(|seat| (seat.server.get_pointer(), seat.server.get_keyboard()))
//     {
//         match event {
//             c_wl_pointer::Event::Button {
//                 time: _time,
//                 button,
//                 state,
//                 serial,
//                 ..
//             } => {
//                 last_input_serial.replace(serial);
//                 last_button.replace(button);

//                 let s = space.handle_press(seat_name);

//                 if let Some(kbd) = kbd.as_ref() {
//                     if let Some(client_id) = s.as_ref().and_then(|s| s.client_id()) {
//                         if !kbd.has_focus(&client_id) {
//                             kbd.set_focus(&dh, s.as_ref(), SERIAL_COUNTER.next_serial());
//                         }
//                     } else {
//                         kbd.set_focus(&dh, s.as_ref(), SERIAL_COUNTER.next_serial());
//                     }
//                 }
//                 if let Ok(axis) = wl_pointer::Axis::try_from(axis as u32) {
//                     *af = af.stop(axis);
//                 }
//             }
//             c_wl_pointer::Event::AxisDiscrete { axis, discrete } => {
//                 let axis_frame =
//                     if let Some(af) = axis_frame.iter_mut().find(|af| af.seat_name == seat_name) {
//                         af
//                     } else {
//                         let mut new_afd = AxisFrameData::default();
//                         new_afd.seat_name = seat_name.to_string();
//                         axis_frame.push(new_afd);
//                         axis_frame.last_mut().unwrap()
//                     };
//                 match axis {
//                     c_wl_pointer::Axis::HorizontalScroll => {
//                         axis_frame.h_discrete.replace(discrete);
//                     }
//                     c_wl_pointer::Axis::VerticalScroll => {
//                         axis_frame.v_discrete.replace(discrete);
//                     }
//                     _ => (),
//                 }
//             }
//             c_wl_pointer::Event::Enter {
//                 surface,
//                 surface_x,
//                 surface_y,
//                 ..
//             } => {
//                 // if not popup, then must be a panel layer shell surface
//                 // TODO better handling of subsurfaces?
//                 {
//                     let mut c_hovered_surface = c_hovered_surface.borrow_mut();
//                     if let Some(i) = c_hovered_surface.iter().position(|f| f.1 == seat_name) {
//                         c_hovered_surface[i].0 = surface.clone();
//                         c_hovered_surface[i].2 = FocusStatus::Focused;
//                     } else {
//                         c_hovered_surface.push((
//                             surface.clone(),
//                             seat_name.to_string(),
//                             FocusStatus::Focused,
//                         ));
//                     }
//                 }

//                 if let Some(ServerPointerFocus {
//                     surface,
//                     c_pos,
//                     s_pos,
//                     ..
//                 }) =
//                     space.update_pointer((surface_x as i32, surface_y as i32), seat_name, surface)
//                 {
//                     ptr.motion(
//                         global_state,
//                         &dh,
//                         &MotionEvent {
//                             location: c_pos.to_f64() + Point::from((surface_x, surface_y)),
//                             focus: Some((surface.clone(), s_pos)),
//                             serial: SERIAL_COUNTER.next_serial(),
//                             time: time.try_into().unwrap(),
//                         },
//                     );
//                 } else {
//                     ptr.motion(
//                         global_state,
//                         &dh,
//                         &MotionEvent {
//                             location: Point::from((surface_x, surface_y)),
//                             focus: None,
//                             serial: SERIAL_COUNTER.next_serial(),
//                             time: time.try_into().unwrap(),
//                         },
//                     );
//                 }
//             }
//             c_wl_pointer::Event::Motion {
//                 time: _time,
//                 surface_x,
//                 surface_y,
//             } => {
//                 let c_focused_surface = match c_hovered_surface
//                     .borrow()
//                     .iter()
//                     .find(|f| f.1.as_str() == seat_name)
//                 {
//                     Some(f) => f.0.clone(),
//                     None => return,
//                 };

//                 if let Some(ServerPointerFocus {
//                     surface,
//                     c_pos,
//                     s_pos,
//                     ..
//                 }) = space.update_pointer(
//                     (surface_x as i32, surface_y as i32),
//                     seat_name,
//                     c_focused_surface,
//                 ) {
//                     ptr.motion(
//                         global_state,
//                         &dh,
//                         &MotionEvent {
//                             location: c_pos.to_f64() + Point::from((surface_x, surface_y)),
//                             focus: Some((surface.clone(), s_pos)),
//                             serial: SERIAL_COUNTER.next_serial(),
//                             time: time.try_into().unwrap(),
//                         },
//                     );
//                 } else {
//                     ptr.motion(
//                         global_state,
//                         &dh,
//                         &MotionEvent {
//                             location: Point::from((surface_x, surface_y)),
//                             focus: None,
//                             serial: SERIAL_COUNTER.next_serial(),
//                             time: time.try_into().unwrap(),
//                         },
//                     );
//                 }
//             }
//             c_wl_pointer::Event::Leave { surface, .. } => {
//                 {
//                     let mut c_hovered_surface = c_hovered_surface.borrow_mut();
//                     if let Some(i) = c_hovered_surface.iter().position(|f| f.0 == surface) {
//                         c_hovered_surface[i].2 = FocusStatus::LastFocused(Instant::now());
//                     }
//                 }

//                 space.pointer_leave(seat_name, Some(surface));

//                 ptr.motion(
//                     global_state,
//                     &dh,
//                     &MotionEvent {
//                         location: (0.0, 0.0).into(),
//                         focus: None,
//                         serial: SERIAL_COUNTER.next_serial(),
//                         time: time.try_into().unwrap(),
//                     },
//                 );
//             }
//             _ => (),
//         };
//     }
// }
