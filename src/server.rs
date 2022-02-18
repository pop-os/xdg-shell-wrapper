// SPDX-License-Identifier: GPL-3.0-only

use std::{
    os::unix::{io::AsRawFd, net::UnixStream},
    time::Duration,
};

use anyhow::Result;
use sctk::reexports::calloop::{self, generic::Generic, Interest, Mode};
use slog::{trace, Logger};
use smithay::{
    reexports::{
        nix::fcntl,
        wayland_server::{
            self,
            protocol::{wl_data_device_manager::DndAction, wl_shm::Format},
        },
    },
    wayland::{
        compositor::compositor_init,
        data_device::{default_action_chooser, init_data_device},
        shell::xdg::{xdg_shell_init, XdgRequest},
        shm::init_shm_global,
    },
};

use crate::config::XdgWrapperConfig;
use crate::shared_state::*;

pub fn new_server(
    loop_handle: calloop::LoopHandle<'static, (GlobalState, wayland_server::Display)>,
    _config: XdgWrapperConfig,
    log: Logger,
) -> Result<(
    EmbeddedServerState,
    wayland_server::Display,
    (UnixStream, UnixStream),
)> {
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
        // let display = &mut shared_data.embedded_server_state.display;
        Ok(calloop::PostAction::Continue)
    })?;

    init_data_device(
        &mut display,
        |dnd_event| { /* a callback to react to client DnD/selection actions */ },
        default_action_chooser,
        log.clone(),
    );

    let log_handle = log.clone();
    compositor_init(
        &mut display,
        move |surface, mut dispatch_data| {
            let state = dispatch_data.get::<GlobalState>().unwrap();
            let desktop_client_surface = &mut state.desktop_client_state.surface;
            if let Some((_, desktop_client_surface)) = desktop_client_surface.borrow_mut().as_mut()
            {
                desktop_client_surface.render(surface, state.start_time.elapsed().as_millis() as u32);
                slog::debug!(&log_handle, "Rendered");
            }
        },
        log.clone(),
    );

    let (shell_state, _) = xdg_shell_init(
        &mut display,
        move |request: XdgRequest, mut dispatch_data| {
            let state = dispatch_data.get::<GlobalState>().unwrap();
            let log = &mut state.log;
            match request {
                XdgRequest::NewToplevel { surface } => {
                    let layer_shell_surface = state.desktop_client_state.surface.borrow_mut();

                    let _ = surface.with_pending_state(move |top_level_state| {
                        if let Some(layer_shell_surface) = layer_shell_surface.as_ref() {
                            let w = layer_shell_surface.1.dimensions.0 as i32;
                            let h = layer_shell_surface.1.dimensions.1 as i32;
                            top_level_state.size = Some((w, h).into());
                        }
                    });
                    surface.send_configure();
                }
                XdgRequest::NewPopup { surface, .. } => {
                    let _ = surface.send_configure();
                }
                _ => {
                    trace!(log, "Received xdg request.");
                }
            }
        },
        log.clone(),
    );

    init_shm_global(&mut display, vec![Format::Yuyv, Format::C8], log.clone());

    Ok((
        EmbeddedServerState {
            client,
            shell_state,
        },
        display,
        (display_sock, client_sock),
    ))
}
