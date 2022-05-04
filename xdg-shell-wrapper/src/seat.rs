// SPDX-License-Identifier: MPL-2.0-only

use anyhow::Result;
use sctk::{
    environment::Environment,
    reexports::{
        client::protocol::{
            wl_pointer as c_wl_pointer, wl_seat as c_wl_seat, wl_surface as c_wl_surface,
        },
        client::Attached,
        client::{self, protocol::wl_keyboard},
    },
    seat::SeatData,
};
use slog::{error, trace, Logger};
use smithay::{
    backend::input::KeyState,
    desktop::{utils::bbox_from_surface_tree, PopupKind, WindowSurfaceType},
    reexports::wayland_server::{
        protocol::{wl_pointer, wl_surface::WlSurface},
        DispatchData, Display,
    },
    wayland::{
        data_device::{set_data_device_focus, set_data_device_selection},
        seat::{self, AxisFrame, FilterResult, PointerHandle},
        SERIAL_COUNTER,
    },
};
use std::{cell::RefCell, rc::Rc};

use super::DesktopClientState;
use crate::{
    client::Env,
    shared_state::EmbeddedServerState,
    space::{ServerSurface, Space},
    ClientSeat, GlobalState, Seat,
};

pub fn send_keyboard_event(
    event: wl_keyboard::Event,
    seat_name: &str,
    mut dispatch_data: DispatchData,
) {
    let (state, _server_display) = dispatch_data.get::<(GlobalState, Display)>().unwrap();
    let DesktopClientState {
        env_handle,
        seats,
        kbd_focus,
        last_input_serial,
        space,
        ..
    } = &mut state.desktop_client_state;

    let EmbeddedServerState {
        client,
        focused_surface,
        selected_data_provider,
        ..
    } = &mut state.embedded_server_state;

    if let Some(seat) = seats.iter().find(|Seat { name, .. }| name == &seat_name) {
        let kbd = match seat.server.0.get_keyboard() {
            Some(kbd) => kbd,
            None => {
                error!(
                    state.log,
                    "Received keyboard event on {} without keyboard.", &seat_name
                );
                return;
            }
        };
        match event {
            wl_keyboard::Event::Key {
                serial,
                time,
                key,
                state,
            } => {
                last_input_serial.replace(serial);
                let state = match state {
                    client::protocol::wl_keyboard::KeyState::Pressed => KeyState::Pressed,
                    client::protocol::wl_keyboard::KeyState::Released => KeyState::Released,
                    _ => return,
                };
                match kbd.input::<(), _>(
                    key,
                    state,
                    SERIAL_COUNTER.next_serial(),
                    time,
                    move |modifiers, keysym| {
                        // Alt + tab cycles the active top level
                        if modifiers.alt && keysym.raw_code() == 23 && state == KeyState::Released {
                            FilterResult::Intercept(())
                        } else {
                            FilterResult::Forward // TODO intercept some key presses maybe
                        }
                    },
                ) {
                    Some(_) => {
                        if let Some(renderer) = space {
                            renderer.cycle_active();
                        }
                    }
                    None => {}
                }
            }
            wl_keyboard::Event::RepeatInfo { rate, delay } => {
                kbd.change_repeat_info(rate, delay);
            }
            wl_keyboard::Event::Enter { .. } => {
                let _ = set_server_device_selection(
                    env_handle,
                    &seat.client.seat,
                    &seat.server.0,
                    &selected_data_provider.seat,
                );
                set_data_device_focus(&seat.server.0, Some(client.clone()));
                *kbd_focus = true;
                kbd.set_focus(
                    focused_surface.borrow().as_ref(),
                    SERIAL_COUNTER.next_serial(),
                );
            }
            wl_keyboard::Event::Leave { .. } => {
                *kbd_focus = false;
                kbd.set_focus(None, SERIAL_COUNTER.next_serial());
            }
            _ => (),
        };
    }
    // keep Modifier state in Seat
    // trace!(logger, "{:?}", event);
}

pub fn send_pointer_event(
    event: c_wl_pointer::Event,
    seat_name: &str,
    mut dispatch_data: DispatchData,
) {
    let (state, _server_display) = dispatch_data.get::<(GlobalState, Display)>().unwrap();
    let DesktopClientState {
        seats,
        axis_frame,
        last_input_serial,
        focused_surface: c_focused_surface,
        space,
        ..
    } = &mut state.desktop_client_state;
    let EmbeddedServerState {
        last_button,
        focused_surface,
        ..
    } = &mut state.embedded_server_state;

    if let Some(Some(ptr)) = seats
        .iter()
        .position(|Seat { name, .. }| name == &seat_name)
        .map(|idx| &seats[idx])
        .map(|seat| seat.server.0.get_pointer())
    {
        match event {
            c_wl_pointer::Event::Motion {
                time,
                surface_x,
                surface_y,
            } => {
                handle_motion(
                    space,
                    focused_surface.borrow().clone(),
                    surface_x,
                    surface_y,
                    ptr,
                    time,
                );
            }
            c_wl_pointer::Event::Button {
                time,
                button,
                state,
                serial,
                ..
            } => {
                last_input_serial.replace(serial);
                if let Some(button_state) = wl_pointer::ButtonState::from_raw(state.to_raw()) {
                    ptr.button(button, button_state, SERIAL_COUNTER.next_serial(), time);
                }
                last_button.replace(button);
            }
            c_wl_pointer::Event::Axis { time, axis, value } => {
                let mut af = axis_frame.frame.take().unwrap_or(AxisFrame::new(time));
                if let Some(axis_source) = axis_frame.source.take() {
                    af = af.source(axis_source);
                }
                if let Some(axis) = wl_pointer::Axis::from_raw(axis.to_raw()) {
                    match axis {
                        wl_pointer::Axis::HorizontalScroll => {
                            if let Some(discrete) = axis_frame.h_discrete {
                                af = af.discrete(axis, discrete);
                            }
                        }
                        wl_pointer::Axis::VerticalScroll => {
                            if let Some(discrete) = axis_frame.v_discrete {
                                af = af.discrete(axis, discrete);
                            }
                        }
                        _ => return,
                    }
                    af = af.value(axis, value);
                }
                axis_frame.frame = Some(af);
            }
            c_wl_pointer::Event::Frame => {
                if let Some(af) = axis_frame.frame.take() {
                    ptr.axis(af);
                }
                axis_frame.h_discrete = None;
                axis_frame.v_discrete = None;
            }
            c_wl_pointer::Event::AxisSource { axis_source } => {
                // add source to the current frame if it exists
                let source = wl_pointer::AxisSource::from_raw(axis_source.to_raw());
                if let Some(af) = axis_frame.frame.as_mut() {
                    if let Some(source) = source {
                        *af = af.source(source);
                    }
                } else {
                    // hold on to source and add to the next frame
                    axis_frame.source = source;
                }
            }
            c_wl_pointer::Event::AxisStop { time, axis } => {
                let mut af = axis_frame.frame.take().unwrap_or(AxisFrame::new(time));
                if let Some(axis) = wl_pointer::Axis::from_raw(axis.to_raw()) {
                    af = af.stop(axis);
                }
                axis_frame.frame = Some(af);
            }
            c_wl_pointer::Event::AxisDiscrete { axis, discrete } => match axis {
                c_wl_pointer::Axis::HorizontalScroll => {
                    axis_frame.h_discrete.replace(discrete);
                }
                c_wl_pointer::Axis::VerticalScroll => {
                    axis_frame.v_discrete.replace(discrete);
                }
                _ => return,
            },
            c_wl_pointer::Event::Enter {
                surface,
                surface_x,
                surface_y,
                ..
            } => {
                // TODO better handling of subsurfaces?
                set_focused_surface(focused_surface, space, &surface, surface_x, surface_y);
                c_focused_surface.replace(surface);
            }
            c_wl_pointer::Event::Leave { surface, .. } => {
                if let Some(s) = c_focused_surface {
                    if s == &surface {
                        focused_surface.take();
                    }
                }
            }
            _ => (),
        };
    }
}

pub fn seat_handle_callback(
    log: Logger,
    seat: Attached<c_wl_seat::WlSeat>,
    seat_data: &SeatData,
    mut dispatch_data: DispatchData,
) {
    let (state, server_display) = dispatch_data.get::<(GlobalState, Display)>().unwrap();
    let DesktopClientState {
        seats, env_handle, ..
    } = &mut state.desktop_client_state;
    let EmbeddedServerState {
        selected_data_provider,
        ..
    } = &mut state.embedded_server_state;

    let logger = &state.log;
    // find the seat in the vec of seats, or insert it if it is unknown
    trace!(logger, "seat event: {:?} {:?}", seat, seat_data);

    let seat_name = seat_data.name.clone();
    let idx = seats
        .iter()
        .position(|Seat { name, .. }| name == &seat_name);
    let idx = idx.unwrap_or_else(|| {
        seats.push(Seat {
            name: seat_name.clone(),
            server: seat::Seat::new(server_display, seat_name.clone(), log.clone()),
            client: ClientSeat {
                kbd: None,
                ptr: None,
                seat: seat.clone(),
            },
        });
        seats.len()
    });

    let Seat {
        client:
            ClientSeat {
                kbd: ref mut opt_kbd,
                ptr: ref mut opt_ptr,
                ..
            },
        server: (ref mut server_seat, ref mut _server_seat_global),
        ..
    } = &mut seats[idx];
    // we should map a keyboard if the seat has the capability & is not defunct
    if (seat_data.has_keyboard || seat_data.has_pointer) && !seat_data.defunct {
        if opt_kbd.is_none() {
            // we should initalize a keyboard
            let kbd = seat.get_keyboard();
            kbd.quick_assign(move |_, event, dispatch_data| {
                send_keyboard_event(event, &seat_name, dispatch_data)
            });
            *opt_kbd = Some(kbd.detach());
            // TODO error handling
            if let Err(e) =
                server_seat.add_keyboard(Default::default(), 200, 20, move |_seat, _focus| {})
            {
                slog::error!(logger, "failed to add keyboard: {}", e);
            }
        }
        if opt_ptr.is_none() {
            // we should initalize a keyboard
            let seat_name = seat_data.name.clone();
            let pointer = seat.get_pointer();
            pointer.quick_assign(move |_, event, dispatch_data| {
                send_pointer_event(event, &seat_name, dispatch_data)
            });
            server_seat.add_pointer(move |_new_status| {});
            *opt_ptr = Some(pointer.detach());
        }
        let _ = set_server_device_selection(
            env_handle,
            &seat,
            server_seat,
            &selected_data_provider.seat,
        );
    } else {
        //cleanup
        if let Some(kbd) = opt_kbd.take() {
            kbd.release();
            server_seat.remove_keyboard();
        }
        if let Some(ptr) = opt_ptr.take() {
            ptr.release();
            server_seat.remove_pointer();
        }
    }
}

pub(crate) fn set_server_device_selection(
    env_handle: &Environment<Env>,
    seat: &Attached<c_wl_seat::WlSeat>,
    server_seat: &seat::Seat,
    selected_data_provider: &RefCell<Option<Attached<c_wl_seat::WlSeat>>>,
) -> Result<()> {
    env_handle.with_data_device(&seat, |data_device| {
        data_device.with_selection(|offer| {
            if let Some(offer) = offer {
                offer.with_mime_types(|types| {
                    set_data_device_selection(server_seat, types.into());
                })
            }
        })
    })?;
    selected_data_provider.replace(Some(seat.clone()));
    Ok(())
}

// TODO revisit motion over popup
pub(crate) fn handle_motion(
    renderer: &mut Option<Space>,
    focused_surface: Option<WlSurface>,
    surface_x: f64,
    surface_y: f64,
    ptr: PointerHandle,
    time: u32,
) {
    let focused_surface = match focused_surface {
        Some(s) => s,
        _ => return,
    };
    match renderer
        .as_ref()
        .map(|r| r.find_server_window(&focused_surface))
    {
        Some(Some(ServerSurface::TopLevel(loc_offset, toplevel))) => {
            let surface_x = surface_x - loc_offset.x as f64;
            let surface_y = surface_y - loc_offset.y as f64;
            let toplevel = &*toplevel.borrow();
            if let Some((cur_surface, location)) =
                toplevel.surface_under((surface_x, surface_y), WindowSurfaceType::ALL)
            {
                let adjusted_loc = toplevel.bbox().loc;
                let offset = if toplevel.toplevel().get_surface() == Some(&cur_surface) {
                    adjusted_loc
                } else {
                    adjusted_loc - location
                };

                ptr.motion(
                    (surface_x + offset.x as f64, surface_y + offset.y as f64).into(),
                    Some((cur_surface, (0, 0).into())),
                    SERIAL_COUNTER.next_serial(),
                    time,
                );
            }
        }
        Some(Some(ServerSurface::Popup(_, _toplevel, popup))) => {
            let popup_surface = match popup.get_surface() {
                Some(s) => s,
                _ => return,
            };
            let bbox = bbox_from_surface_tree(popup_surface, (0, 0));
            let offset = bbox.loc + PopupKind::Xdg(popup.clone()).geometry().loc;
            ptr.motion(
                (surface_x + offset.x as f64, surface_y + offset.y as f64).into(),
                Some((popup_surface.clone(), (0, 0).into())),
                SERIAL_COUNTER.next_serial(),
                time,
            );
        }
        _ => return,
    };
}

pub(crate) fn set_focused_surface(
    focused_surface: &Rc<RefCell<Option<WlSurface>>>,
    space: &mut Option<Space>,
    surface: &c_wl_surface::WlSurface,
    x: f64,
    y: f64,
) {
    let mut focused_surface = focused_surface.borrow_mut();
    *focused_surface = if let Some(space) = space {
        match space.find_server_surface(surface) {
            Some(ServerSurface::TopLevel(loc_offset, toplevel)) => {
                let toplevel = toplevel.borrow();
                if let Some((cur_surface, _)) = toplevel.surface_under(
                    (x - loc_offset.x as f64, y - loc_offset.y as f64),
                    WindowSurfaceType::ALL,
                ) {
                    Some(cur_surface)
                } else {
                    toplevel.toplevel().get_surface().map(|s| s.clone())
                }
            }
            Some(ServerSurface::Popup(_, _toplevel, popup)) => match popup.get_surface() {
                Some(s) => Some(s.clone()),
                _ => None,
            },
            _ => None,
        }
    } else {
        None
    };
}
