// SPDX-License-Identifier: GPL-3.0-only

#![feature(drain_filter)]

use std::{os::unix::io::AsRawFd, process::Command};

use anyhow::Result;
use shlex::Shlex;
use slog::{o, trace, Drain};
use smithay::reexports::nix::fcntl;

use config::XdgWrapperConfig;
use shared_state::*;

mod client;
mod config;
mod server;
mod shared_state;
mod util;

fn main() -> Result<()> {
    // A logger facility, here we use the terminal
    let log = slog::Logger::root(
        slog_async::Async::default(slog_term::term_full().fuse()).fuse(),
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

    let mut event_loop = calloop::EventLoop::<GlobalState>::try_new().unwrap();
    let loop_handle = event_loop.handle();
    let (mut embedded_server_state, (_display_sock, client_sock)) =
        server::new_server(loop_handle.clone(), config.clone(), log.clone())?;
    let (desktop_client_state, outputs) = client::new_client(
        loop_handle.clone(),
        config.clone(),
        log.clone(),
        &mut embedded_server_state,
    )?;
    let mut global_state = GlobalState {
        desktop_client_state,
        embedded_server_state,
        loop_signal: event_loop.get_signal(),
        outputs,
        log: log.clone(),
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
        .spawn()
        .expect("Failed to start child process");

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
