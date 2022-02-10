// SPDX-License-Identifier: GPL-3.0-only

use crate::{config::XdgWrapperConfig, util::*};
use anyhow::Result;
use sctk::reexports::calloop::{self, generic::Generic, Interest, Mode};
use slog::{trace, Logger};
use smithay::reexports::wayland_server;
use smithay::wayland::{
    compositor::compositor_init,
    shell::xdg::{xdg_shell_init, XdgRequest},
};
use std::{
    os::unix::{io::IntoRawFd, net::UnixStream},
    time::Duration,
};

pub fn new_server(
    loop_handle: calloop::LoopHandle<'static, GlobalState>,
    config: XdgWrapperConfig,
    log: Logger,
) -> Result<(EmbeddedServerState, UnixStream)> {
    let mut display = wayland_server::Display::new();
    let (display_sock, client_sock) = UnixStream::pair().unwrap();

    let client = unsafe { display.create_client(display_sock.into_raw_fd(), &mut ()) };

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
        move |event: XdgRequest, dispatch_data| {
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
