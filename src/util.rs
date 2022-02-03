// SPDX-License-Identifier: GPL-3.0-only

use sctk::reexports::client::protocol::{wl_pointer, wl_surface};
use sctk::seat::keyboard::{self, KeyState, ModifiersState};
use sctk::window::Event as WEvent;

#[derive(Debug, Clone)]
pub enum KbEvent {
    /// The keyboard focus has entered a surface
    Enter {
        /// serial number of the event
        serial: u32,
        /// surface that was entered
        surface: wl_surface::WlSurface,
        /// raw values of the currently pressed keys
        rawkeys: Vec<u32>,
        /// interpreted symbols of the currently pressed keys
        keysyms: Vec<u32>,
    },
    /// The keyboard focus has left a surface
    Leave {
        /// serial number of the event
        serial: u32,
        /// surface that was left
        surface: wl_surface::WlSurface,
    },
    /// The key modifiers have changed state
    Modifiers {
        /// current state of the modifiers
        modifiers: ModifiersState,
    },
    /// A key event occurred
    Key {
        /// serial number of the event
        serial: u32,
        /// time at which the keypress occurred
        time: u32,
        /// raw value of the key
        rawkey: u32,
        /// interpreted symbol of the key
        keysym: u32,
        /// new state of the key
        state: KeyState,
        /// utf8 interpretation of the entered text
        ///
        /// will always be `None` on key release events
        utf8: Option<String>,
    },
    /// A key repetition event
    Repeat {
        /// time at which the repetition occured
        time: u32,
        /// raw value of the key
        rawkey: u32,
        /// interpreted symbol of the key
        keysym: u32,
        /// utf8 interpretation of the entered text
        utf8: Option<String>,
    },
}

impl From<keyboard::Event<'_>> for KbEvent {
    fn from(event: keyboard::Event) -> Self {
        match event {
            keyboard::Event::Enter {
                serial,
                surface,
                rawkeys,
                keysyms,
            } => Self::Enter {
                serial,
                surface: surface.clone(),
                rawkeys: rawkeys.to_vec(),
                keysyms: keysyms.to_vec(),
            },
            keyboard::Event::Key {
                serial,
                time,
                rawkey,
                keysym,
                state,
                utf8,
            } => Self::Key {
                serial,
                time,
                rawkey,
                keysym,
                state,
                utf8,
            },
            keyboard::Event::Leave { serial, surface } => Self::Leave {
                serial,
                surface: surface.clone(),
            },
            keyboard::Event::Modifiers { modifiers } => Self::Modifiers {
                modifiers: modifiers.clone(),
            },
            keyboard::Event::Repeat {
                time,
                rawkey,
                keysym,
                utf8,
            } => Self::Repeat {
                time,
                rawkey,
                keysym,
                utf8,
            },
        }
    }
}

#[derive(Debug)]
pub enum ClientMsg {
    WEvent(WEvent),
    KbEvent(KbEvent),
    PtrEvent(wl_pointer::Event),
}

pub enum ServerMsg {}
