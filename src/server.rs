// SPDX-License-Identifier: GPL-3.0-only

use crate::util::*;
use anyhow::Result;
// use futures::io::{AsyncReadExt, AsyncWriteExt};
use sctk::reexports::calloop::{self, generic::Generic, Interest, Mode};
use smithay::reexports::wayland_server;
use smithay::wayland::compositor::compositor_init;
use std::{
    os::unix::{io::AsRawFd, net::UnixStream},
    time::Duration,
};

pub fn new_server(
    loop_handle: calloop::LoopHandle<'static, GlobalState>,
) -> Result<EmbeddedServerState> {
    let mut display = wayland_server::Display::new();
    let (display_sock, client_sock) = UnixStream::pair().unwrap();
    display.add_socket_from(display_sock).unwrap();
    let _display_client = unsafe { display.create_client(client_sock.as_raw_fd(), &mut ()) };
    compositor_init(&mut display, |_surface, _dispatch_data| {}, None);
    let display_event_source = Generic::new(display.get_poll_fd(), Interest::READ, Mode::Edge);
    loop_handle.insert_source(display_event_source, move |_e, _metadata, shared_data| {
        let display = &mut shared_data.embedded_server_state.display;
        display.dispatch(Duration::ZERO, &mut ())?;
        Ok(calloop::PostAction::Continue)
    })?;

    Ok(EmbeddedServerState { display })
}
