// SPDX-License-Identifier: MPL-2.0-only

use anyhow::Result;
use config::XdgWrapperConfig;
use render::CachedBuffers;
use shared_state::*;
use slog::Logger;
use smithay::reexports::{nix::fcntl, wayland_server::Display};
use std::{
    os::unix::io::AsRawFd,
    process::Command,
    thread,
    time::{Duration, Instant},
};

mod client;
pub mod config;
mod server;
mod render;
mod util;
mod output;
mod seat;
mod shared_state;

pub fn xdg_shell_wrapper(mut child: Command, log: Logger, config: XdgWrapperConfig) -> Result<()> {
    let mut event_loop = calloop::EventLoop::<(GlobalState, Display)>::try_new().unwrap();
    let loop_handle = event_loop.handle();
    let (embedded_server_state, mut display, (_display_sock, client_sock)) =
        server::new_server(loop_handle.clone(), config.clone(), log.clone())?;
    let (desktop_client_state, outputs) = client::new_client(
        loop_handle.clone(),
        config.clone(),
        log.clone(),
        &mut display,
    )?;

    let global_state = GlobalState {
        desktop_client_state,
        embedded_server_state,
        loop_signal: event_loop.get_signal(),
        outputs,
        log: log.clone(),
        start_time: std::time::Instant::now(),
        cached_buffers: CachedBuffers::new(log.clone()),
    };

    let raw_fd = client_sock.as_raw_fd();
    let fd_flags =
        fcntl::FdFlag::from_bits(fcntl::fcntl(raw_fd, fcntl::FcntlArg::F_GETFD)?).unwrap();
    fcntl::fcntl(
        raw_fd,
        fcntl::FcntlArg::F_SETFD(fd_flags.difference(fcntl::FdFlag::FD_CLOEXEC)),
    )?;

    let mut child = child
        .env("WAYLAND_SOCKET", raw_fd.to_string())
        .env_remove("WAYLAND_DEBUG")
        .spawn()
        .expect("Failed to start child process");

    let mut shared_data = (global_state, display);
    let mut last_dirty = Instant::now();
    let mut last_cleanup = Instant::now();
    let five_min = Duration::from_secs(300);
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

            let renderer = &mut shared_data.desktop_client_state.renderer.as_mut();
            if let Some(renderer) = renderer {
                renderer.apply_display(&server_display);
                last_dirty =
                    renderer.handle_events(shared_data.start_time.elapsed().as_millis() as u32);
            }
        }

        // dispatch server events
        {
            server_display
                .dispatch(Duration::from_millis(16), shared_data)
                .unwrap();
            server_display.flush_clients(shared_data);
        }

        if let Ok(Some(_)) = child.try_wait() {
            return Ok(());
        }

        // sleep if not much is changing...
        let milli_since_last_dirty = (Instant::now() - last_dirty).as_millis();
        if milli_since_last_dirty < 120 {
            thread::sleep(Duration::from_millis(8));
        } else if milli_since_last_dirty < 600 {
            thread::sleep(Duration::from_millis(32));
        } else if milli_since_last_dirty < 3000 {
            thread::sleep(Duration::from_millis(128));
        }
    }
}
