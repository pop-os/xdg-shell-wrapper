// SPDX-License-Identifier: GPL-3.0-only

use smithay::backend::renderer::utils::on_commit_buffer_handler;
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
        SERIAL_COUNTER,
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

    compositor_init(
        &mut display,
        move |surface, mut dispatch_data| {
            on_commit_buffer_handler(&surface);
            let state = dispatch_data.get::<GlobalState>().unwrap();
            let desktop_client_surface = &mut state.desktop_client_state.surface;
            if let Some((_, desktop_client_surface)) = desktop_client_surface.borrow_mut().as_mut()
            {
                desktop_client_surface.server_surface = Some(surface);
                desktop_client_surface.dirty = true;
            }
        },
        log.clone(),
    );

    let (shell_state, _) = xdg_shell_init(
        &mut display,
        move |request: XdgRequest, mut dispatch_data| {
            let state = dispatch_data.get::<GlobalState>().unwrap();
            let seats = &mut state.desktop_client_state.seats;
            let kbd_focus = &state.desktop_client_state.kbd_focus;
            let focused_surface = &mut state.embedded_server_state.focused_surface;
            let log = &mut state.log;

            match request {
                XdgRequest::NewToplevel { surface } => {
                    surface.send_configure();
                    let wl_surface = surface.get_surface();
                    *focused_surface = wl_surface.map(|s| s.clone());
                    if *kbd_focus {
                        for s in seats {
                            if let Some(kbd) = s.server.0.get_keyboard() {
                                kbd.set_focus(wl_surface, SERIAL_COUNTER.next_serial());
                            }
                        }
                    }
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
            focused_surface: None,
        },
        display,
        (display_sock, client_sock),
    ))
}
