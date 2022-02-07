// SPDX-License-Identifier: GPL-3.0-only

use anyhow::Result;
mod client;
mod config;
mod server;
mod util;
use config::XdgWrapperConfig;
use util::*;

fn main() -> Result<()> {
    let arg = std::env::args().nth(1);
    let config = match arg.as_ref().map(|s| &s[..]) {
        Some(arg) if arg == "--profile" || arg == "-p" => {
            if let Some(profile) = std::env::args().nth(2) {
                XdgWrapperConfig::load(profile.as_str())
            } else {
                println!("USAGE: xdg_shell_wrapper <executable> OR xd_shell_wrapper --profile <profile name>");
                std::process::exit(1);
            }
        }
        Some(exec) => {
            let mut config = XdgWrapperConfig::default();
            config.exec = exec.into();
            config
        }
        None => {
            println!("USAGE: xdg_shell_wrapper <executable> OR xd_shell_wrapper --profile <profile name>");
            std::process::exit(1);
        }
    };

    let mut event_loop = calloop::EventLoop::<util::GlobalState>::try_new().unwrap();
    let loop_handle = event_loop.handle();
    let desktop_client_state = client::new_client(loop_handle.clone(), config)?;
    let embedded_server_state = server::new_server(loop_handle)?;
    let mut global_state = GlobalState {
        desktop_client_state,
        embedded_server_state,
        loop_signal: event_loop.get_signal(),
    };

    event_loop
        .run(None, &mut global_state, |shared_data| {
            let display = &mut shared_data.desktop_client_state.display;
            let surface = &mut shared_data.desktop_client_state.surface.borrow_mut();
            let loop_signal = &mut shared_data.loop_signal;
            if surface.is_some() {
                let remove_surface = surface.as_mut().unwrap().1.handle_events();
                if remove_surface {
                    println!("exiting");
                    surface.take();
                    loop_signal.stop();
                }
            }

            display.flush().unwrap();
        })
        .unwrap();
    Ok(())
}
