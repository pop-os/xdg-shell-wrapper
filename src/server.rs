// SPDX-License-Identifier: GPL-3.0-only

use crate::{config::XdgWrapperConfig, util::*};
use anyhow::Result;
use sctk::reexports::calloop::{self, generic::Generic, Interest, Mode};
use smithay::reexports::wayland_server;
use smithay::wayland::compositor::compositor_init;
use std::{
    env,
    os::unix::{io::IntoRawFd, net::UnixStream},
    time::Duration,
};

pub fn new_server(
    loop_handle: calloop::LoopHandle<'static, GlobalState>,
    config: XdgWrapperConfig,
) -> Result<EmbeddedServerState> {
    let mut display = wayland_server::Display::new();
    let (display_sock, client_sock) = UnixStream::pair().unwrap();

    let client = unsafe { display.create_client(display_sock.into_raw_fd(), &mut ()) };

    env::set_var("WAYLAND_SOCKET", client_sock.into_raw_fd().to_string());

    let display_event_source = Generic::new(display.get_poll_fd(), Interest::READ, Mode::Edge);
    loop_handle.insert_source(display_event_source, move |_e, _metadata, shared_data| {
        let display = &mut shared_data.embedded_server_state.display;
        display.dispatch(Duration::ZERO, &mut ())?;
        Ok(calloop::PostAction::Continue)
    })?;
    compositor_init(&mut display, |_surface, _dispatch_data| {}, None);

    // TODO launch config.exec

    Ok(EmbeddedServerState { display, client })
}
