// SPDX-License-Identifier: MPL-2.0-only

use once_cell::sync::OnceCell;
use smithay::wayland::compositor::SurfaceAttributes;
use smithay::wayland::compositor::{get_role, with_states};
use smithay::wayland::data_device::DataDeviceEvent;
use std::{
    cell::{RefCell, RefMut},
    os::unix::{io::AsRawFd, net::UnixStream},
    rc::Rc,
};
use anyhow::Result;
use sctk::reexports::{calloop::{self, generic::Generic, Interest, Mode},         client::{protocol::wl_seat as c_wl_seat, Attached},
};
use slog::{error, trace, Logger};
use smithay::{
    backend::renderer::{buffer_type, utils::on_commit_buffer_handler, BufferType},
    desktop::{utils, Kind, PopupKind, PopupManager, Window},
    reexports::{
        nix::fcntl,
        wayland_protocols::xdg_shell::client::xdg_positioner::{Anchor, Gravity},
        wayland_server::{self, protocol::wl_shm::Format},
    },
    wayland::{
        compositor::{compositor_init, BufferAssignment},
        data_device::{default_action_chooser, init_data_device},
        shell::xdg::{xdg_shell_init, PositionerState, XdgRequest},
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
        Ok(calloop::PostAction::Continue)
    })?;

    let selected_seat: Rc<RefCell<Option<Attached<c_wl_seat::WlSeat>>>> = Rc::new(RefCell::new(None));
    let env: Rc<OnceCell<sctk::environment::Environment<crate::client::Env>>> = Rc::new(OnceCell::new());
    let selected_data_provider = selected_seat.clone();
    let env_handle = env.clone();
    trace!(log.clone(), "init embedded server data device");
    let logger = log.clone();
    init_data_device(
        &mut display,
        move |event| { 
            /* a callback to react to client DnD/selection actions */            
            match event {
                DataDeviceEvent::SendSelection { mime_type, fd } => {
                    if let (Some(seat), Some(env_handle)) = (selected_data_provider.borrow().as_ref(), env_handle.get()) {
                        let res = env_handle.with_data_device(&seat, |data_device| {
                            data_device.with_selection(|offer| {
                                if let Some(offer) = offer {
                                    offer.with_mime_types(|types| {
                                        if types.contains(&mime_type) {
                                            let _ = unsafe { offer.receive_to_fd(mime_type, fd) };
                                        }
                                    })
                                }
                            })
                        });

                        if let Err(err) = res {
                            error!(logger, "{:?}", err);
                        }
                    }
                }
                _ => {},
            };
         },
        default_action_chooser,
        log.clone(),
    );

    trace!(log.clone(), "init embedded compositor");
    let (_compositor, _subcompositor) = compositor_init(
        &mut display,
        move |surface, mut dispatch_data| {
            let state = dispatch_data.get::<GlobalState>().unwrap();
            let DesktopClientState {
                cursor_surface,
                renderer,
                seats,
                shm,
                ..
            } = &mut state.desktop_client_state;
            let EmbeddedServerState {
                popup_manager,
                shell_state,
                ..
            } = &mut state.embedded_server_state;
            let cached_buffers = &mut state.cached_buffers;
            let log = &mut state.log;

            let role = get_role(&surface);
            trace!(log, "role: {:?} surface: {:?}", &role, &surface);
            if role == "xdg_toplevel".into() {
                if let Some(renderer) = renderer.as_mut() {
                    if let Some(top_level) = shell_state.lock().unwrap().toplevel_surface(&surface)
                    {
                        on_commit_buffer_handler(&surface);
                        let window = Window::new(Kind::Xdg(top_level.clone()));
                        window.refresh();
                        let w = window.bbox().size.w as u32;
                        let h = window.bbox().size.h as u32;
                        renderer.dirty(&surface, (w, h));
                    }
                }
            } else if role == "cursor_image".into() {
                // pass cursor image to parent compositor
                trace!(log, "received surface with cursor image");
                for Seat { client, .. } in seats {
                    if let Some(ptr) = client.ptr.as_ref() {
                        trace!(log, "updating cursor for pointer {:?}", &ptr);
                        let _ = with_states(&surface, |data| {
                            let surface_attributes =
                                data.cached_state.current::<SurfaceAttributes>();
                            let buf = RefMut::map(surface_attributes, |s| &mut s.buffer);
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
                                ptr.set_cursor(SERIAL_COUNTER.next_serial().into(), None, 0, 0);
                            }
                        });
                    }
                }
            } else if role == "xdg_popup".into() {
                let popup = popup_manager.borrow().find_popup(&surface);
                on_commit_buffer_handler(&surface);
                popup_manager.borrow_mut().commit(&surface);
                let (top_level_surface, popup_surface) = match popup {
                    Some(PopupKind::Xdg(s)) => (s.get_parent_surface(), s),
                    _ => return,
                };
                if let (Some(renderer), Some(top_level_surface)) = (renderer, top_level_surface) {
                    renderer.dirty_popup(&top_level_surface, popup_surface, utils::bbox_from_surface_tree(&surface, (0,0)));
                }
            } else {
                trace!(log, "{:?}", surface);
            }
        },
        log.clone(),
    );

    trace!(log.clone(), "init xdg_shell for embedded compositor");
    let (shell_state, _) = xdg_shell_init(
        &mut display,
        move |request: XdgRequest, mut dispatch_data| {
            let state = dispatch_data.get::<GlobalState>().unwrap();
            let DesktopClientState {
                seats,
                kbd_focus,
                env_handle,
                renderer,
                xdg_wm_base,
                ..
            } = &mut state.desktop_client_state;

            let EmbeddedServerState {
                focused_surface,
                popup_manager,
                root_window,
                ..
            } = &mut state.embedded_server_state;
            let log = &mut state.log;

            match request {
                XdgRequest::NewToplevel { surface } => {
                    let window = Window::new(Kind::Xdg(surface.clone()));
                    window.refresh();
                    let g = window.geometry();
                    let dimensions = (g.size.w as u32, g.size.h as u32);

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

                    let window = Rc::new(RefCell::new(smithay::desktop::Window::new(
                        smithay::desktop::Kind::Xdg(surface),
                    )));

                    let layer_shell_surface = env_handle.create_surface();

                    if let Some(renderer) = renderer.as_mut() {
                        renderer.add_top_level(layer_shell_surface, window.clone(), dimensions);
                    }
                    root_window.replace(window);
                }
                XdgRequest::NewPopup {
                    surface: s_popup_surface,
                    positioner:
                        PositionerState {
                            rect_size,
                            anchor_rect,
                            anchor_edges,
                            gravity,
                            constraint_adjustment,
                            offset,
                            reactive,
                            parent_size,
                            ..
                        },
                } => {
                    let positioner = xdg_wm_base.create_positioner();
                    positioner.set_size(rect_size.w, rect_size.h);
                    positioner.set_anchor_rect(
                        anchor_rect.loc.x,
                        anchor_rect.loc.y,
                        anchor_rect.size.w,
                        anchor_rect.size.h,
                    );
                    positioner.set_anchor(
                        Anchor::from_raw(anchor_edges.to_raw().into()).unwrap_or(Anchor::None),
                    );
                    positioner.set_gravity(
                        Gravity::from_raw(gravity.to_raw().into()).unwrap_or(Gravity::None),
                    );
                    positioner.set_constraint_adjustment(constraint_adjustment.to_raw());
                    positioner.set_offset(offset.x, offset.y);
                    if positioner.as_ref().version() >= 3 {
                        if reactive {
                            positioner.set_reactive();
                        }
                        if let Some(parent_size) = parent_size {
                            positioner.set_parent_size(parent_size.w, parent_size.h);
                        }
                    }
                    // TODO what to do with parent configure?

                    let wl_surface = env_handle.create_surface().detach();
                    let xdg_surface = xdg_wm_base.get_xdg_surface(&wl_surface);
                    let popup = xdg_surface.get_popup(None, &positioner);

                    if let (Some(parent), Some(renderer)) =
                        (s_popup_surface.get_parent_surface(), renderer.as_mut())
                    {
                        renderer.add_popup(
                            wl_surface,
                            xdg_surface,
                            popup,
                            s_popup_surface.clone(),
                            parent,
                            rect_size.w,
                            rect_size.h,
                            popup_manager.clone(),
                        );
                    }
                }
                XdgRequest::Grab {
                    surface,
                    seat,
                    serial,
                } => {
                    if *kbd_focus {
                        for s in seats {
                            if s.server.0.owns(&seat) {
                                if let Err(e) = popup_manager.borrow_mut().grab_popup(
                                    PopupKind::Xdg(surface.clone()),
                                    &s.server.0,
                                    serial,
                                ) {
                                    error!(log.clone(), "{}", e);
                                }
                                // TODO forward grab on client?
                                break;
                            }
                        }
                    }
                }
                e => {
                    trace!(log, "{:?}", e);
                }
            }
        },
        log.clone(),
    );

    init_shm_global(&mut display, vec![Format::Yuyv, Format::C8], log.clone());

    trace!(log.clone(), "embedded server setup complete");

    let popup_manager = Rc::new(RefCell::new(PopupManager::new(log.clone())));

    Ok((
        EmbeddedServerState {
            client,
            shell_state,
            popup_manager,
            root_window: Default::default(),
            focused_surface: Default::default(),
            selected_data_provider: SelectedDataProvider {seat: selected_seat, env_handle: env },
        },
        display,
        (display_sock, client_sock),
    ))
}
