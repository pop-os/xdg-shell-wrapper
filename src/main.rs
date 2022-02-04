// SPDX-License-Identifier: GPL-3.0-only

use anyhow::Result;
mod client;
mod server;
mod util;
use sctk::window::Event as WEvent;
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
            let DesktopClientState {
                display,
                next_wevent,
                window,
                dimensions,
                pool,
                ..
            } = &mut shared_data.desktop_client_state;
            let signal = &mut shared_data.loop_signal;
            if let Some(event) = next_wevent.take() {
                match event {
                    WEvent::Close => signal.stop(),
                    WEvent::Refresh => {
                        window.refresh();
                        window.surface().commit();
                    }
                    WEvent::Configure { new_size, states } => {
                        if let Some((w, h)) = new_size {
                            window.resize(w, h);
                            *dimensions = (w, h)
                        }
                        println!("Window states: {:?}", states);
                        window.refresh();
                        client::redraw(pool, window.surface(), *dimensions)
                            .expect("Failed to draw");
                    }
                }
            }

            // always flush the connection before going to sleep waiting for events
            display.flush().unwrap();
        })
        .unwrap();
    Ok(())
}
