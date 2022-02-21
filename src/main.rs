// SPDX-License-Identifier: GPL-3.0-only

#![feature(drain_filter)]

use std::{os::unix::io::AsRawFd, process::Command, thread, time::Duration};

use anyhow::Result;
use shlex::Shlex;
use slog::{o, trace, Drain};
use smithay::reexports::{nix::fcntl, wayland_server::Display};

use config::XdgWrapperConfig;
use shared_state::*;

mod client;
mod config;
mod server;
mod shared_state;
mod util;
use smithay::{
    backend::{allocator::dmabuf::Dmabuf, renderer::ImportDma},
    reexports::wayland_server::protocol::wl_buffer::WlBuffer,
    wayland::dmabuf::init_dmabuf_global,
};

fn main() -> Result<()> {
    // A logger facility, here we use the terminal
    let log = slog::Logger::root(
        // slog_async::Async::default(slog_term::term_full().fuse()).fuse(),
        slog::Discard,
        o!(),
    );

    let _guard = slog_scope::set_global_logger(log.clone());
    slog_stdlog::init().expect("Could not setup log backend");

    let arg = std::env::args().nth(1);
    let usage =
        "USAGE: xdg_shell_wrapper '<executable> <arg>' OR xd_shell_wrapper --profile <profile name>";
    let config = match arg.as_ref().map(|s| &s[..]) {
        Some(arg) if arg == "--profile" || arg == "-p" => {
            if let Some(profile) = std::env::args().nth(2) {
                XdgWrapperConfig::load(profile.as_str())
            } else {
                println!("{}", usage);
                std::process::exit(1);
            }
        }
        Some(exec) => {
            let mut config = XdgWrapperConfig::default();
            config.exec = exec.into();
            config
        }
        None => {
            println!("{}", usage);
            std::process::exit(1);
        }
    };

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

    let _dmabuf_global = init_dmabuf_global(
        &mut display,
        desktop_client_state
            .surface
            .borrow_mut()
            .as_ref()
            .unwrap()
            .1
            .renderer
            .dmabuf_formats()
            .copied()
            .collect::<Vec<_>>(),
        |_buffer, _dispatch_data| {
            /* validate the dmabuf and import it into your renderer state */
            true
        },
        log.clone(),
    );

    let global_state = GlobalState {
        desktop_client_state,
        embedded_server_state,
        loop_signal: event_loop.get_signal(),
        outputs,
        log: log.clone(),
        start_time: std::time::Instant::now(),
    };

    // start child process
    let mut exec_iter = Shlex::new(&config.exec);
    let exec = exec_iter
        .next()
        .expect("exec parameter must contain at least on word");
    trace!(log, "child: {}", &exec);

    let mut child = Command::new(exec);
    while let Some(arg) = exec_iter.next() {
        trace!(log, "child argument: {}", &arg);
        child.arg(arg);
    }

    let raw_fd = client_sock.as_raw_fd();
    let fd_flags =
        fcntl::FdFlag::from_bits(fcntl::fcntl(raw_fd, fcntl::FcntlArg::F_GETFD)?).unwrap();
    fcntl::fcntl(
        raw_fd,
        fcntl::FcntlArg::F_SETFD(fd_flags.difference(fcntl::FdFlag::FD_CLOEXEC)),
    )?;

    child
        .env("WAYLAND_SOCKET", raw_fd.to_string())
        .env_remove("WAYLAND_DEBUG")
        .spawn()
        .expect("Failed to start child process");

    let mut shared_data = (global_state, display);
    let mut iter_since_render = -1;
    loop {
        iter_since_render = i32::clamp(iter_since_render + 1, 0, 99999);
        dbg!(iter_since_render);
        event_loop
            .dispatch(None, &mut shared_data)
            .expect("Failed to dispatch events...");

        let (shared_data, server_display) = &mut shared_data;
        {
            let display = &mut shared_data.desktop_client_state.display;
            display.flush().unwrap();

            let surface = &mut shared_data.desktop_client_state.surface.borrow_mut();
            if surface.is_some() {
                let remove_surface = surface.as_mut().unwrap().1.handle_events();
                if remove_surface {
                    println!("exiting");
                    surface.take();
                    break;
                } else if let Some((_, surface)) = surface.as_mut() {
                    if surface.dirty {
                        // TODO: only render, when new client buffer or frame-callback called.
                        // Probably just add a "dirty"-flag to the surface state or something
                        surface.render(shared_data.start_time.elapsed().as_millis() as u32);
                        iter_since_render = 0;
                    }
                }
            }
        }
        {
            server_display
                .dispatch(Duration::ZERO, shared_data)
                .unwrap();
            server_display.flush_clients(shared_data);
        }
        if iter_since_render > 0 && iter_since_render < 120 {
            thread::sleep(Duration::from_millis(8));
        } else if iter_since_render > 0 && iter_since_render < 600 {
            thread::sleep(Duration::from_millis(32));
        } else if iter_since_render > 0 && iter_since_render < 3000 {
            thread::sleep(Duration::from_millis(128));
        }
    }
    Ok(())
}
