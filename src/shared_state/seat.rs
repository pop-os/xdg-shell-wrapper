// SPDX-License-Identifier: GPL-3.0-only

use sctk::reexports::{
    client::protocol::{wl_pointer as c_wl_pointer, wl_seat::WlSeat},
    client::Attached,
    client::{self, protocol::wl_keyboard},
};
use sctk::seat::SeatData;
use slog::{trace, Logger};
use smithay::backend::input::KeyState;
use smithay::reexports::wayland_server::{protocol::wl_pointer, DispatchData, Display};
use smithay::wayland::{
    seat::{self, AxisFrame, FilterResult},
    SERIAL_COUNTER,
};

use crate::{ClientSeat, GlobalState, Seat};

pub fn send_keyboard_event(
    event: wl_keyboard::Event,
    seat_name: &str,
    mut dispatch_data: DispatchData,
) {
    let (state, _server_display) = dispatch_data.get::<(GlobalState, Display)>().unwrap();
    let seats = &state.desktop_client_state.seats;
    let focused_surface = &state.embedded_server_state.focused_surface;
    let kbd_focus = &mut state.desktop_client_state.kbd_focus;

    if let Some(Some(kbd)) = seats
        .iter()
        .position(|Seat { name, .. }| name == &seat_name)
        .map(|idx| &seats[idx])
        .map(|seat| seat.server.0.get_keyboard())
    {
        match event {
            wl_keyboard::Event::Key {
                serial: _serial,
                time,
                key,
                state,
            } => {
                let state = match state {
                    client::protocol::wl_keyboard::KeyState::Pressed => KeyState::Pressed,
                    client::protocol::wl_keyboard::KeyState::Released => KeyState::Released,
                    _ => return,
                };
                kbd.input::<FilterResult<()>, _>(
                    key,
                    state,
                    SERIAL_COUNTER.next_serial(),
                    time,
                    |_, _| {
                        FilterResult::Forward // TODO intercept some key presses maybe
                    },
                );
            }
            wl_keyboard::Event::RepeatInfo { rate, delay } => {
                kbd.change_repeat_info(rate, delay);
            }
            wl_keyboard::Event::Enter { .. } => {
                *kbd_focus = true;
                kbd.set_focus(focused_surface.as_ref(), SERIAL_COUNTER.next_serial());
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
    let seats = &state.desktop_client_state.seats;
    let axis_frame = &mut state.desktop_client_state.axis_frame;

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
                let server_surface = state
                    .desktop_client_state
                    .surface
                    .as_ref()
                    .map(|(_, s)| {
                        s.server_surface
                            .clone()
                            .map(|server_s| (server_s, smithay::utils::Point::from((0, 0))))
                    })
                    .unwrap_or_default();
                let loc = state
                    .desktop_client_state
                    .surface
                    .as_ref()
                    .map(|(_, s)| s.pointer_loc(surface_x, surface_y))
                    .unwrap_or_default();
                ptr.motion(
                    loc.into(),
                    server_surface,
                    SERIAL_COUNTER.next_serial(),
                    time,
                );
            }
            c_wl_pointer::Event::Button {
                time,
                button,
                state,
                ..
            } => {
                if let Some(button_state) = wl_pointer::ButtonState::from_raw(state.to_raw()) {
                    ptr.button(button, button_state, SERIAL_COUNTER.next_serial(), time);
                }
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
            // TODO do these need to be handled?
            c_wl_pointer::Event::Enter { .. } => {}
            c_wl_pointer::Event::Leave { .. } => {}
            _ => (),
        };
    }
}

pub fn seat_handler(
    log: Logger,
    seat: Attached<WlSeat>,
    seat_data: &SeatData,
    mut dispatch_data: DispatchData,
) {
    let (state, server_display) = dispatch_data.get::<(GlobalState, Display)>().unwrap();
    let seats = &mut state.desktop_client_state.seats;
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
            },
        });
        seats.len()
    });

    let Seat {
        client:
            ClientSeat {
                kbd: ref mut opt_kbd,
                ptr: ref mut opt_ptr,
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
        //TODO when to destroy server_seat_global?
    }
}
