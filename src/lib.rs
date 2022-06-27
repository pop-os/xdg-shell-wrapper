// SPDX-License-Identifier: MPL-2.0-only
#![warn(missing_debug_implementations, rust_2018_idioms, missing_docs)]

//! Provides the core functionality for cosmic-panel

use anyhow::Result;
use calloop::{Interest, Mode, generic::Generic, PostAction};
use shared_state::GlobalState;
use smithay::{
    reexports::wayland_server::{Display},
};
use space::{CachedBuffers, Visibility, WrapperSpace};
use wrapper_client::state::DesktopClientState;
use wrapper_server::state::EmbeddedServerState;
use std::{
    cell::Cell,
    rc::Rc,
    thread,
    time::{Duration, Instant},
};
pub use wrapper_client::state as client_state;
pub use wrapper_server::state as server_state;

pub mod config;
pub mod shared_state;
pub mod space;
pub mod util;

mod wrapper_client;
mod wrapper_server;

/// run the cosmic panel xdg wrapper with the provided config
pub fn run<W: WrapperSpace + 'static>(mut space: W) -> Result<()> {
    let log = space.log().unwrap();
    let mut event_loop = calloop::EventLoop::<(GlobalState<W>, Display<GlobalState<W>>)>::try_new().unwrap();
    let loop_handle = event_loop.handle();
    
    let mut server_display = smithay::reexports::wayland_server::Display::new().unwrap();
    let s_dh = server_display.handle();
    loop_handle
    .insert_source(
        Generic::new(server_display.backend().poll_fd(), Interest::READ, Mode::Level),
        |_, _, (state, display)| {
            display.dispatch_clients(state).unwrap();
            Ok(PostAction::Continue)
        },
    )
    .expect("Failed to init wayland server source");

    let mut embedded_server_state =
        EmbeddedServerState::new(&s_dh, log.clone());

    let mut desktop_client_state = DesktopClientState::new(
        loop_handle.clone(),
        &mut space,
        log.clone(),
        &mut server_display.handle(),
        &mut embedded_server_state,
    )?;
    let _sockets = space
        .spawn_clients(&mut server_display.handle())
        .unwrap();

    let global_state = GlobalState {
        desktop_client_state,
        embedded_server_state,
        _loop_signal: event_loop.get_signal(),
        log: log.clone(),
        start_time: std::time::Instant::now(),
        cached_buffers: CachedBuffers::new(log.clone()),
        space,
    };

    let mut shared_data = (global_state, server_display);
    let mut last_cleanup = Instant::now();
    let five_min = Duration::from_secs(300);

    // TODO find better place for this
    let set_clipboard_once = Rc::new(Cell::new(false));

    loop {
        // cleanup popup manager
        if last_cleanup.elapsed() > five_min {
            shared_data
                .0
                .embedded_server_state
                .popup_manager
                .borrow_mut()
                .cleanup();
            last_cleanup = Instant::now();
        }

        // dispatch desktop client events
        let dispatch_client_res = event_loop.dispatch(Duration::from_millis(16), &mut shared_data);

        dispatch_client_res.expect("Failed to dispatch events");

        let (shared_data, server_display) = &mut shared_data;

        // rendering
        {
            let display = &mut shared_data.desktop_client_state.display;
            display.flush().unwrap();

            let space = &mut shared_data.space;

            // FIXME
            // space_manager.apply_display(server_display);
            let _ = space.handle_events(
                shared_data
                    .start_time
                    .elapsed()
                    .as_millis()
                    .try_into()
                    .unwrap(),
                &shared_data.desktop_client_state.focused_surface,
            );
        }

        // dispatch server events
        {
            server_display
                .dispatch_clients(shared_data)
                .unwrap();
            server_display.flush_clients();
        }

        // TODO find better place for this
        // the idea is to forward clipbard as soon as possible just once
        // this method is not ideal...
        // if !set_clipboard_once.get() {
        //     let desktop_client_state = &shared_data.desktop_client_state;
        //     for s in &desktop_client_state.seats {
        //         let server_seat = &s.server.0;
        //         let _ = desktop_client_state.env_handle.with_data_device(
        //             &s.client.seat,
        //             |data_device| {
        //                 data_device.with_selection(|offer| {
        //                     if let Some(offer) = offer {
        //                         offer.with_mime_types(|types| {
        //                             set_data_device_selection(
        //                                 server_display,
        //                                 server_seat,
        //                                 types.into(),
        //                             );
        //                             set_clipboard_once.replace(true);
        //                         })
        //                     }
        //                 })
        //             },
        //         );
        //     }
        // }

        // sleep if not focused...
        if matches!(
            shared_data.space.visibility(),
            Visibility::Hidden
        ) {
            thread::sleep(Duration::from_millis(100));
        }
    }
}
