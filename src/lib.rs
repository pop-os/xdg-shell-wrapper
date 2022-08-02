// SPDX-License-Identifier: MPL-2.0-only
#![warn(missing_debug_implementations, rust_2018_idioms, missing_docs)]

//! Provides the core functionality for cosmic-panel

use std::{
    thread,
    time::{Duration, Instant},
};

use anyhow::Result;
use smithay::reexports::{
    calloop::{self, generic::Generic, Interest, Mode, PostAction},
};

use client::state::ClientState;
pub use client::{handlers::output, state as client_state};
pub use server::state as server_state;
use server::state::ServerState;
use shared_state::GlobalState;
use space::{cached_buffer::CachedBuffers, Visibility, WrapperSpace};
pub use xdg_shell_wrapper_config as config;

mod client;
mod server;
/// shared state
pub mod shared_state;
/// wrapper space abstraction
pub mod space;
/// utilities
pub mod util;

/// run the cosmic panel xdg wrapper with the provided config
pub fn run<W: WrapperSpace + 'static>(
    mut space: W,
    mut event_loop: calloop::EventLoop<'static, GlobalState<W>>,
) -> Result<()> {
    let log = space.log().unwrap();
    let loop_handle = event_loop.handle();

    let mut server_display = smithay::reexports::wayland_server::Display::new().unwrap();
    let s_dh = server_display.handle();

    let mut embedded_server_state = ServerState::new(&s_dh, log.clone());

    let desktop_client_state = ClientState::new(
        loop_handle.clone(),
        &mut space,
        log.clone(),
        &mut server_display.handle(),
        &mut embedded_server_state,
    )?;
    let _sockets = space.spawn_clients(server_display.handle()).unwrap();

    let mut global_state = GlobalState {
        client_state: desktop_client_state,
        server_state: embedded_server_state,
        _loop_signal: event_loop.get_signal(),
        log: log.clone(),
        start_time: std::time::Instant::now(),
        cached_buffers: CachedBuffers::new(log.clone()),
        space,
    };
    global_state.bind_display(&s_dh);

    let mut last_cleanup = Instant::now();
    let five_min = Duration::from_secs(300);

    // TODO find better place for this
    // let set_clipboard_once = Rc::new(Cell::new(false));

    loop {
        // cleanup popup manager
        if last_cleanup.elapsed() > five_min {
            global_state.server_state.popup_manager.cleanup();
            last_cleanup = Instant::now();
        }

        // dispatch desktop client events
        event_loop.dispatch(Duration::from_millis(16), &mut global_state).expect("Failed to dispatch events");


        // rendering
        {
            let display = &mut global_state.client_state.display;
            display.flush().unwrap();

            let space = &mut global_state.space;

            let _ = space.handle_events(
                &s_dh,
                &mut global_state.server_state.popup_manager,
                global_state
                    .start_time
                    .elapsed()
                    .as_millis()
                    .try_into()
                    .unwrap(),
            );
        }

        // dispatch server events
        {
            server_display.dispatch_clients(&mut global_state).unwrap();
            server_display.flush_clients().unwrap();
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
        if matches!(global_state.space.visibility(), Visibility::Hidden) {
            thread::sleep(Duration::from_millis(100));
        }
    }
}
