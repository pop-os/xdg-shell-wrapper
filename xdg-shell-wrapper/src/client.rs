// SPDX-License-Identifier: MPL-2.0-only

use anyhow::Result;
use sctk::{
    data_device, default_environment,
    environment::SimpleGlobal,
    output::with_output_info,
    reexports::{calloop, client::protocol::wl_shm},
    seat::SeatHandling,
};
use slog::{trace, Logger};
use smithay::{
    reexports::{
        wayland_protocols::{
            wlr::unstable::layer_shell::v1::client::zwlr_layer_shell_v1,
            xdg_shell::client::xdg_wm_base::XdgWmBase,
        },
        wayland_server::{
            self,
            protocol::{wl_data_device_manager::DndAction, wl_pointer::ButtonState},
        },
    },
    wayland::{
        data_device::{set_data_device_focus, start_dnd, SourceMetadata},
        seat, SERIAL_COUNTER,
    },
};
use std::{cell::RefCell, rc::Rc};

use crate::{
    output::handle_output,
    seat::{
        handle_motion, seat_handle_callback, send_keyboard_event, send_pointer_event,
        set_focused_surface, set_server_device_selection,
    },
    shared_state::*,
};
use crate::{space::Space, XdgWrapperConfig};

default_environment!(Env,
    fields = [
        layer_shell: SimpleGlobal<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
        xdg_wm_base: SimpleGlobal<XdgWmBase>,
    ],
    singles = [
        zwlr_layer_shell_v1::ZwlrLayerShellV1 => layer_shell,
        XdgWmBase => xdg_wm_base,
    ],
);

pub fn new_client(
    loop_handle: calloop::LoopHandle<'static, (GlobalState, wayland_server::Display)>,
    config: XdgWrapperConfig,
    log: Logger,
    server_display: &mut wayland_server::Display,
    embedded_server_state: &EmbeddedServerState,
) -> Result<(DesktopClientState, Vec<OutputGroup>)> {
    /*
     * Initial setup
     */
    let (mut env, display, queue) = sctk::new_default_environment!(
        Env,
        fields = [
            layer_shell: SimpleGlobal::new(),
            xdg_wm_base: SimpleGlobal::new(),
        ]
    )
    .expect("Unable to connect to a Wayland compositor");
    let _ = embedded_server_state
        .selected_data_provider
        .env_handle
        .set(env.clone());
    let focused_surface = embedded_server_state.focused_surface.clone();
    let _attached_display = (*display).clone().attach(queue.token());

    let layer_shell = env.require_global::<zwlr_layer_shell_v1::ZwlrLayerShellV1>();
    let mut space = None;
    let mut s_outputs = Vec::new();
    if let Some(preferred_output) = config.output.as_ref() {
        // swap preffered output to front of list if it is available
        let mut outputs = env.get_all_outputs();
        if let Some(preferred_output_index) = outputs.iter().position(|o| {
            if let Some(info) = with_output_info(&o, Clone::clone) {
                &info.name == preferred_output
            } else {
                false
            }
        }) {
            outputs.swap(preferred_output_index, 0);
        }

        for output in outputs {
            if let Some(info) = with_output_info(&output, Clone::clone) {
                let layer_shell = env.require_global::<zwlr_layer_shell_v1::ZwlrLayerShellV1>();
                let env_handle = env.clone();
                let logger = log.clone();
                let display_ = display.clone();
                let config = config.clone();
                handle_output(
                    config,
                    &layer_shell,
                    env_handle,
                    &mut space,
                    logger,
                    display_,
                    output,
                    &info,
                    server_display,
                    &mut s_outputs,
                    focused_surface.clone(),
                );
            }
        }
    } else {
        space = Some(Space::new(
            None,
            env.create_auto_pool()
                .expect("Failed to create a memory pool!"),
            config.clone(),
            display.clone(),
            layer_shell.clone(),
            log.clone(),
            env.create_surface(),
            embedded_server_state.focused_surface.clone(),
        ));
    }

    let env_handle = env.clone();
    let logger = log.clone();
    let display_ = display.clone();
    let output_listener = if config.output.is_some() {
        Some(
            env.listen_for_outputs(move |output, info, mut dispatch_data| {
                let (state, server_display) = dispatch_data
                    .get::<(GlobalState, wayland_server::Display)>()
                    .unwrap();
                let outputs = &mut state.outputs;
                let renderer = &mut state.desktop_client_state.space;
                handle_output(
                    config.clone(),
                    &layer_shell,
                    env_handle.clone(),
                    renderer,
                    logger.clone(),
                    display_.clone(),
                    output,
                    &info,
                    server_display,
                    outputs,
                    focused_surface.clone(),
                );
            }),
        )
    } else {
        None
    };

    // TODO logging
    // FIXME focus lost after drop from source outside xdg-shell-wrapper
    // dnd listener
    let last_motion = Rc::new(RefCell::new(None));
    let _ = env.set_data_device_callback(move |seat, dnd_event, mut dispatch_data| {
        let (state, _) = dispatch_data
            .get::<(GlobalState, wayland_server::Display)>()
            .unwrap();
        let DesktopClientState {
            seats,
            env_handle,
            space,
            ..
        } = &mut state.desktop_client_state;

        let EmbeddedServerState {
            focused_surface,
            last_button,
            client,
            ..
        } = &state.embedded_server_state;

        if let (Some(last_button), Some(seat)) =
            (last_button, seats.iter().find(|s| *(s.client.seat) == seat))
        {
            match dnd_event {
                sctk::data_device::DndEvent::Enter {
                    offer,
                    serial: _,
                    surface,
                    x,
                    y,
                } => {
                    set_data_device_focus(&seat.server.0, Some(client.clone()));

                    set_focused_surface(focused_surface, space, &surface, x, y);
                    let offer = match offer {
                        Some(o) => o,
                        None => return,
                    };

                    let mime_types = offer.with_mime_types(|mime_types| Vec::from(mime_types));

                    offer.accept(mime_types.iter().next().map(|s| s.clone()));
                    let seat_clone = seat.client.seat.clone();
                    let env_clone = env_handle.clone();
                    start_dnd(
                        &seat.server.0,
                        SERIAL_COUNTER.next_serial(),
                        seat::PointerGrabStartData {
                            focus: focused_surface
                                .borrow()
                                .as_ref()
                                .map(|s| (s.clone(), (0, 0).into())),
                            button: *last_button,
                            location: (x, y).into(),
                        },
                        SourceMetadata {
                            mime_types: mime_types.clone(),
                            dnd_action: DndAction::from_raw(offer.get_available_actions().to_raw())
                                .unwrap(),
                        },
                        move |server_dnd_event| match server_dnd_event {
                            smithay::wayland::data_device::ServerDndEvent::Action(action) => {
                                let _ = env_clone.with_data_device(&seat_clone, |device| {
                                    device.with_dnd(|offer| {
                                        if let Some(offer) = offer {
                                            let action =
                                                data_device::DndAction::from_raw(action.to_raw())
                                                    .unwrap();
                                            offer.set_actions(action, action);
                                        }
                                    });
                                });
                            }
                            smithay::wayland::data_device::ServerDndEvent::Dropped => {}
                            smithay::wayland::data_device::ServerDndEvent::Cancelled => {
                                let _ = env_clone.with_data_device(&seat_clone, |device| {
                                    device.with_dnd(|offer| {
                                        if let Some(offer) = offer {
                                            offer.finish();
                                        }
                                    });
                                });
                            }
                            smithay::wayland::data_device::ServerDndEvent::Send {
                                mime_type,
                                fd,
                            } => {
                                if mime_types.contains(&mime_type) {
                                    let _ = env_clone.with_data_device(&seat_clone, |device| {
                                        device.with_dnd(|offer| {
                                            if let Some(offer) = offer {
                                                unsafe { offer.receive_to_fd(mime_type, fd) };
                                            }
                                        });
                                    });
                                }
                            }
                            smithay::wayland::data_device::ServerDndEvent::Finished => {
                                // println!("finished");
                                let _ = env_clone.with_data_device(&seat_clone, |device| {
                                    device.with_dnd(|offer| {
                                        if let Some(offer) = offer {
                                            offer.finish();
                                        }
                                    });
                                });
                            }
                        },
                    )
                }
                sctk::data_device::DndEvent::Motion {
                    offer: _,
                    time,
                    x,
                    y,
                } => {
                    last_motion.replace(Some(((x, y), time)));

                    handle_motion(
                        space,
                        focused_surface.borrow().clone(),
                        x,
                        y,
                        seat.server.0.get_pointer().unwrap(),
                        time,
                    );
                }
                sctk::data_device::DndEvent::Leave => {}
                sctk::data_device::DndEvent::Drop { .. } => {
                    if let Some(((_, _), time)) = last_motion.take() {
                        seat.server.0.get_pointer().unwrap().button(
                            *last_button,
                            ButtonState::Released,
                            SERIAL_COUNTER.next_serial(),
                            time + 1,
                        );
                    }
                }
            }
        }
    });

    /*
     * Keyboard initialization
     */

    let mut seats = Vec::<Seat>::new();

    // first process already existing seats
    let env_handle = env.clone();
    let event_loop = loop_handle.clone();
    for seat in env.get_all_seats() {
        if let Some((has_kbd, has_ptr, name)) = sctk::seat::with_seat_data(&seat, |seat_data| {
            (
                seat_data.has_keyboard && !seat_data.defunct,
                seat_data.has_pointer && !seat_data.defunct,
                seat_data.name.clone(),
            )
        }) {
            let mut new_seat = Seat {
                name: name.clone(),
                server: seat::Seat::new(server_display, name.clone(), log.clone()),
                client: ClientSeat {
                    kbd: None,
                    ptr: None,
                    seat: seat.clone(),
                },
            };
            if has_kbd || has_ptr {
                if has_kbd {
                    let seat_name = name.clone();
                    trace!(log, "found seat: {:?}", &new_seat);
                    let kbd = seat.get_keyboard();
                    kbd.quick_assign(move |_, event, dispatch_data| {
                        send_keyboard_event(event, &seat_name, dispatch_data)
                    });
                    new_seat.client.kbd = Some(kbd.detach());
                    new_seat.server.0.add_keyboard(
                        Default::default(),
                        200,
                        20,
                        move |_seat, _focus| {},
                    )?;
                }
                if has_ptr {
                    let seat_name = name.clone();
                    let pointer = seat.get_pointer();
                    pointer.quick_assign(move |_, event, dispatch_data| {
                        send_pointer_event(event, &seat_name, dispatch_data)
                    });
                    new_seat.client.ptr = Some(pointer.detach());
                    new_seat.server.0.add_pointer(move |_new_status| {});
                }
            }
            seats.push(new_seat);
        }
    }
    // set server device selection when offer should be available
    event_loop.insert_idle(move |(state, _)| {
        let seats = &mut state.desktop_client_state.seats;
        for s in seats {
            let _ = set_server_device_selection(
                &env_handle,
                &s.client.seat,
                &s.server.0,
                &state.embedded_server_state.selected_data_provider.seat,
            );
        }
    });

    // then setup a listener for changes
    let logger = log.clone();
    env.with_inner(|env_inner| {
        env_inner.listen(move |seat, seat_data, dispatch_data| {
            seat_handle_callback(logger.clone(), seat, seat_data, dispatch_data)
        })
    });

    sctk::WaylandSource::new(queue)
        .quick_insert(loop_handle)
        .unwrap();

    let cursor_surface = env.create_surface().detach();

    let shm = env.require_global::<wl_shm::WlShm>();
    let xdg_wm_base = env.require_global::<XdgWmBase>();

    trace!(log.clone(), "client setup complete");
    Ok((
        DesktopClientState {
            space,
            display,
            _output_listener: output_listener,
            seats: seats,
            kbd_focus: false,
            axis_frame: Default::default(),
            cursor_surface: cursor_surface,
            shm,
            xdg_wm_base,
            env_handle: env,
            last_input_serial: None,
            focused_surface: None,
        },
        s_outputs,
    ))
}
