use crate::{space::WrapperSpace, shared_state::GlobalState};
use sctk::{seat::keyboard::KeyboardHandler, delegate_keyboard};

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
