// SPDX-License-Identifier: GPL-3.0-only

use core::cell::RefMut;
use smithay::wayland::compositor::SurfaceAttributes;
use smithay::wayland::compositor::{get_role, with_states};
use std::os::unix::{io::AsRawFd, net::UnixStream};

use anyhow::Result;
use sctk::reexports::calloop::{self, generic::Generic, Interest, Mode};
use slog::{error, trace, Logger};
use smithay::{
    backend::renderer::{buffer_type, utils::on_commit_buffer_handler, BufferType},
    reexports::{
        nix::fcntl,
        wayland_server::{self, protocol::wl_shm::Format},
    },
    wayland::{
        compositor::{compositor_init, BufferAssignment},
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
    loop_handle.insert_source(display_event_source, move |_e, _metadata, _shared_data| {
        // let display = &mut shared_data.embedded_server_state.display;
        Ok(calloop::PostAction::Continue)
    })?;

    trace!(log.clone(), "init embedded server data device");
    init_data_device(
        &mut display,
        |_dnd_event| { /* a callback to react to client DnD/selection actions */ },
        default_action_chooser,
        log.clone(),
    );

    trace!(log.clone(), "init embedded compositor");
    compositor_init(
        &mut display,
        move |surface, mut dispatch_data| {
            let state = dispatch_data.get::<GlobalState>().unwrap();
            let cursor_surface = &mut state.desktop_client_state.cursor_surface;
            let cached_buffers = &mut state.cached_buffers;
            let shm = &state.desktop_client_state.shm;
            let log = &mut state.log;

            let role = get_role(&surface);
            trace!(log, "role: {:?} surface: {:?}", &role, &surface);
            if role == "xdg_toplevel".into() {
                on_commit_buffer_handler(&surface);
                let desktop_client_surface = &mut state.desktop_client_state.surface;
                if let Some((_, desktop_client_surface)) = desktop_client_surface.as_mut() {
                    trace!(log.clone(), "rendering top level surface");
                    desktop_client_surface.server_surface = Some(surface);
                    desktop_client_surface.dirty = true;
                }
            } else if role == "cursor_image".into() {
                // pass cursor image to parent compositor
                trace!(log, "received surface with cursor image");
                for Seat { client, .. } in &mut state.desktop_client_state.seats {
                    if let Some(ptr) = client.ptr.as_ref() {
                        trace!(log, "updating cursore for pointer {:?}", &ptr);
                        let _ = with_states(&surface, |data| {
                            let surface_attributes =
                                data.cached_state.current::<SurfaceAttributes>();
                            // dbg!(&surface_attributes);
                            let buf = RefMut::map(surface_attributes, |s| &mut s.buffer);
                            // dbg!(&buf);
                            if let Some(BufferAssignment::NewBuffer { buffer, .. }) = buf.as_ref() {
                                if let Some(BufferType::Shm) = buffer_type(buffer) {
                                    trace!(log, "attaching buffer to cursor surface.");
                                    let _ = cached_buffers.write_and_attach_buffer(
                                        &buf.as_ref().unwrap(),
                                        cursor_surface,
                                        shm,
                                    );

                                    trace!(log, "requesting update");
                                    ptr.set_cursor(
                                        SERIAL_COUNTER.next_serial().into(),
                                        Some(cursor_surface),
                                        0,
                                        0,
                                    );
                                }
                            } else {
                                // ptr.set_cursor(SERIAL_COUNTER.next_serial().into(), None, 0, 0);
                            }
                        });
                    }
                }
            } else if role == "xdg_popup".into() {
                // TODO render popup on surface
            }
        },
        log.clone(),
    );

    trace!(log.clone(), "init xdg_shell for embedded compositor");
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
                    let layer_shell_surface = state.desktop_client_state.surface.as_mut();
                    let _ = surface.with_pending_state(move |top_level_state| {
                        if let Some(layer_shell_surface) = layer_shell_surface.as_ref() {
                            let w = layer_shell_surface.1.dimensions.0 as i32;
                            let h = layer_shell_surface.1.dimensions.1 as i32;
                            top_level_state.size = Some((w, h).into());
                        }
                    });
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

                    let mut layer_shell_surface = state.desktop_client_state.surface.as_mut();
                    let window =
                        smithay::desktop::Window::new(smithay::desktop::Kind::Xdg(surface));

                    if let Some((_, surface)) = &mut layer_shell_surface {
                        surface.xdg_window.replace(window);
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

    trace!(log.clone(), "embedded server setup complete");

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
