// SPDX-License-Identifier: GPL-3.0-only

use anyhow::Result;
mod client;
mod server;
mod util;
use util::*;

fn main() -> Result<()> {
    // A logger facility, here we use the terminal here
    // channel for messages from client to server
    let mut event_loop = calloop::EventLoop::<util::GlobalState>::try_new().unwrap();
    let loop_handle = event_loop.handle();
    let desktop_client_state = client::new_client(loop_handle.clone())?;
    let embedded_server_state = server::new_server(loop_handle)?;
    let mut global_state = GlobalState {
        desktop_client_state,
        embedded_server_state,
        loop_signal: event_loop.get_signal(),
    };
    // handles messages with desktop wayland server
    // loop {
    // }

    event_loop
        .run(None, &mut global_state, |shared_data| {
            let display = &mut shared_data.desktop_client_state.display;
            let surface = &mut shared_data.desktop_client_state.surface.borrow_mut();
            let loop_signal = &mut shared_data.loop_signal;
            if let Some(surface) = surface.as_mut() {
                if surface.1.handle_events() {
                    println!("exiting");
                    loop_signal.stop();
                }
            }

            display.flush().unwrap();
        })
        .unwrap();
    Ok(())
}
