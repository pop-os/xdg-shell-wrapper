use crate::{space::WrapperSpace, shared_state::GlobalState};
use sctk::{seat::keyboard::KeyboardHandler, delegate_keyboard};


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

impl<W: WrapperSpace> KeyboardHandler for GlobalState<W> {
    fn enter(
        &mut self,
        conn: &sctk::reexports::client::Connection,
        qh: &sctk::reexports::client::QueueHandle<Self>,
        keyboard: &sctk::reexports::client::protocol::wl_keyboard::WlKeyboard,
        surface: &sctk::reexports::client::protocol::wl_surface::WlSurface,
        serial: u32,
        raw: &[u32],
        keysyms: &[u32],
    ) {
        todo!()
    }

    fn leave(
        &mut self,
        conn: &sctk::reexports::client::Connection,
        qh: &sctk::reexports::client::QueueHandle<Self>,
        keyboard: &sctk::reexports::client::protocol::wl_keyboard::WlKeyboard,
        surface: &sctk::reexports::client::protocol::wl_surface::WlSurface,
        serial: u32,
    ) {
        todo!()
    }

    fn press_key(
        &mut self,
        conn: &sctk::reexports::client::Connection,
        qh: &sctk::reexports::client::QueueHandle<Self>,
        keyboard: &sctk::reexports::client::protocol::wl_keyboard::WlKeyboard,
        serial: u32,
        event: sctk::seat::keyboard::KeyEvent,
    ) {
        todo!()
    }

    fn release_key(
        &mut self,
        conn: &sctk::reexports::client::Connection,
        qh: &sctk::reexports::client::QueueHandle<Self>,
        keyboard: &sctk::reexports::client::protocol::wl_keyboard::WlKeyboard,
        serial: u32,
        event: sctk::seat::keyboard::KeyEvent,
    ) {
        todo!()
    }

    fn update_modifiers(
        &mut self,
        conn: &sctk::reexports::client::Connection,
        qh: &sctk::reexports::client::QueueHandle<Self>,
        keyboard: &sctk::reexports::client::protocol::wl_keyboard::WlKeyboard,
        serial: u32,
        modifiers: sctk::seat::keyboard::Modifiers,
    ) {
        todo!()
    }
}

delegate_keyboard!(@<W: WrapperSpace + 'static> GlobalState<W>);
