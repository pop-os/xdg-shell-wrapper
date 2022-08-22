use std::time::Instant;

use crate::{
    client_state::FocusStatus, server_state::SeatPair, shared_state::GlobalState,
    space::WrapperSpace,
};
use sctk::{
    delegate_keyboard,
    seat::keyboard::{keysyms::XKB_KEY_Escape, KeyboardHandler, RepeatInfo},
};
use smithay::{
    backend::input::KeyState,
    wayland::{seat::FilterResult, SERIAL_COUNTER},
};

impl<W: WrapperSpace> KeyboardHandler for GlobalState<W> {
    fn enter(
        &mut self,
        _conn: &sctk::reexports::client::Connection,
        _qh: &sctk::reexports::client::QueueHandle<Self>,
        keyboard: &sctk::reexports::client::protocol::wl_keyboard::WlKeyboard,
        surface: &sctk::reexports::client::protocol::wl_surface::WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[u32],
    ) {
        let (seat_name, kbd) = if let Some((name, Some(kbd))) = self
            .server_state
            .seats
            .iter()
            .find(|SeatPair { client, .. }| {
                client.kbd.as_ref().map(|k| k == keyboard).unwrap_or(false)
            })
            .map(|seat| (seat.name.as_str(), seat.server.get_keyboard()))
        {
            (name.to_string(), kbd)
        } else {
            return;
        };

        {
            let mut c_focused_surface = self.client_state.focused_surface.borrow_mut();
            if let Some(i) = c_focused_surface.iter().position(|f| f.1 == seat_name) {
                c_focused_surface[i].0 = surface.clone();
                c_focused_surface[i].2 = FocusStatus::Focused;
            } else {
                c_focused_surface.push((
                    surface.clone(),
                    seat_name.to_string(),
                    FocusStatus::Focused,
                ));
            }
        }

        let s = self.space.keyboard_enter(&seat_name, surface.clone());
        kbd.set_focus(
            &self.server_state.display_handle,
            s.as_ref(),
            SERIAL_COUNTER.next_serial(),
        );
    }

    fn leave(
        &mut self,
        _conn: &sctk::reexports::client::Connection,
        _qh: &sctk::reexports::client::QueueHandle<Self>,
        keyboard: &sctk::reexports::client::protocol::wl_keyboard::WlKeyboard,
        surface: &sctk::reexports::client::protocol::wl_surface::WlSurface,
        _serial: u32,
    ) {
        let (seat_name, kbd) = if let Some((name, Some(kbd))) = self
            .server_state
            .seats
            .iter()
            .find(|SeatPair { client, .. }| {
                client.kbd.as_ref().map(|k| k == keyboard).unwrap_or(false)
            })
            .map(|seat| (seat.name.as_str(), seat.server.get_keyboard()))
        {
            (name.to_string(), kbd)
        } else {
            return;
        };

        let kbd_focus = {
            let mut c_focused_surface = self.client_state.focused_surface.borrow_mut();
            if let Some(i) = c_focused_surface.iter().position(|f| &f.0 == surface) {
                c_focused_surface[i].2 = FocusStatus::LastFocused(Instant::now());
                true
            } else {
                false
            }
        };
        if kbd_focus {
            self.space.keyboard_leave(&seat_name, Some(surface.clone()));
            kbd.set_focus(
                &self.server_state.display_handle,
                None,
                SERIAL_COUNTER.next_serial(),
            );
        }
    }

    fn press_key(
        &mut self,
        _conn: &sctk::reexports::client::Connection,
        _qh: &sctk::reexports::client::QueueHandle<Self>,
        keyboard: &sctk::reexports::client::protocol::wl_keyboard::WlKeyboard,
        _serial: u32,
        event: sctk::seat::keyboard::KeyEvent,
    ) {
        let kbd = if let Some(Some(kbd)) =
            self.server_state
                .seats
                .iter()
                .find_map(|SeatPair { client, server, .. }| {
                    client.kbd.as_ref().map(|k| {
                        if k == keyboard {
                            server.get_keyboard()
                        } else {
                            None
                        }
                    })
                }) {
            kbd
        } else {
            return;
        };

        let _ = kbd.input::<(), _>(
            &self.server_state.display_handle,
            event.keysym,
            KeyState::Pressed,
            SERIAL_COUNTER.next_serial(),
            event.time,
            move |_modifiers, _keysym| FilterResult::Forward,
        );
    }

    fn release_key(
        &mut self,
        _conn: &sctk::reexports::client::Connection,
        _qh: &sctk::reexports::client::QueueHandle<Self>,
        keyboard: &sctk::reexports::client::protocol::wl_keyboard::WlKeyboard,
        _serial: u32,
        event: sctk::seat::keyboard::KeyEvent,
    ) {
        let (seat_name, kbd) = if let Some((name, Some(kbd))) = self
            .server_state
            .seats
            .iter()
            .find(|SeatPair { client, .. }| {
                client.kbd.as_ref().map(|k| k == keyboard).unwrap_or(false)
            })
            .map(|seat| (seat.name.as_str(), seat.server.get_keyboard()))
        {
            (name.to_string(), kbd)
        } else {
            return;
        };

        match kbd.input::<(), _>(
            &self.server_state.display_handle,
            event.keysym,
            KeyState::Released,
            SERIAL_COUNTER.next_serial(),
            event.time,
            move |_modifiers, keysym| {
                if keysym.modified_sym() == XKB_KEY_Escape {
                    FilterResult::Intercept(())
                } else {
                    FilterResult::Forward
                }
            },
        ) {
            Some(_) => {
                self.space.keyboard_leave(&seat_name, None);
                kbd.set_focus(
                    &self.server_state.display_handle,
                    None,
                    SERIAL_COUNTER.next_serial(),
                );
            }
            None => {}
        };
    }

    fn update_repeat_info(
        &mut self,
        _conn: &sctk::reexports::client::Connection,
        _qh: &sctk::reexports::client::QueueHandle<Self>,
        kbd: &sctk::reexports::client::protocol::wl_keyboard::WlKeyboard,
        info: RepeatInfo,
    ) {
        if let Some(kbd) =
            self.server_state
                .seats
                .iter()
                .find_map(|SeatPair { client, server, .. }| {
                    client.kbd.as_ref().and_then(|k| {
                        if k == kbd {
                            server.get_keyboard()
                        } else {
                            None
                        }
                    })
                })
        {
            match info {
                RepeatInfo::Repeat { rate, delay } => {
                    kbd.change_repeat_info(u32::from(rate) as i32, delay.try_into().unwrap())
                }
                RepeatInfo::Disable => kbd.change_repeat_info(0, 0),
            };
        }
    }

    fn update_modifiers(
        &mut self,
        _conn: &sctk::reexports::client::Connection,
        _qh: &sctk::reexports::client::QueueHandle<Self>,
        _keyboard: &sctk::reexports::client::protocol::wl_keyboard::WlKeyboard,
        _serial: u32,
        _modifiers: sctk::seat::keyboard::Modifiers,
    ) {
        // TODO should these be handled specially
    }
}

delegate_keyboard!(@<W: WrapperSpace + 'static> GlobalState<W>);
