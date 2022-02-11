// SPDX-License-Identifier: GPL-3.0-only

use crate::{config::XdgWrapperConfig, util::*};
use anyhow::Result;
use sctk::reexports::calloop::{self, generic::Generic, Interest, Mode};
use slog::{trace, Logger};
use smithay::reexports::{nix::fcntl, wayland_server};
use smithay::wayland::{
    compositor::compositor_init,
    shell::xdg::{xdg_shell_init, XdgRequest},
};
use std::{
    os::unix::{io::AsRawFd, net::UnixStream},
    time::Duration,
};

pub fn new_server(
    loop_handle: calloop::LoopHandle<'static, GlobalState>,
    _config: XdgWrapperConfig,
    log: Logger,
) -> Result<(EmbeddedServerState, UnixStream)> {
    let mut display = wayland_server::Display::new();
    let (display_sock, client_sock) = UnixStream::pair().unwrap();
    let raw_fd = display_sock.as_raw_fd();
    let fd_flags =
        fcntl::FdFlag::from_bits(fcntl::fcntl(raw_fd, fcntl::FcntlArg::F_GETFD)?).unwrap();
    fcntl::fcntl(
        raw_fd,
        fcntl::FcntlArg::F_SETFD(fd_flags.difference(fcntl::FdFlag::FD_CLOEXEC)),
    )?;

    let client = unsafe { display.create_client(raw_fd, &mut ()) };

    let display_event_source = Generic::new(display.get_poll_fd(), Interest::READ, Mode::Edge);
    loop_handle.insert_source(display_event_source, move |_e, _metadata, shared_data| {
        let display = &mut shared_data.embedded_server_state.display;
        display.dispatch(Duration::ZERO, &mut ())?;
        Ok(calloop::PostAction::Continue)
    })?;
    compositor_init(
        &mut display,
        |surface, dispatch_data| {
            dbg!(surface);
            dbg!(dispatch_data);
        },
        log.clone(),
    );

    let logger = log.clone();
    let (shell_state, _) = xdg_shell_init(
        &mut display,
        // your implementation
        move |event: XdgRequest, _dispatch_data| {
            trace!(logger, "xdg shell event: {:?}", event);
        },
        log.clone(), // put a logger if you want
    );

    Ok((
        EmbeddedServerState {
            display,
            client,
            shell_state,
        },
        client_sock,
    ))
}
