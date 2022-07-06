// SPDX-License-Identifier: MPL-2.0-only

use std::{cell::RefCell, rc::Rc, time::Instant};

use itertools::Itertools;
use sctk::{
    reexports::client::{
        self,
        protocol::{wl_keyboard, wl_pointer as c_wl_pointer, wl_seat as c_wl_seat},
        Attached, DispatchData,
    },
    seat::SeatData,
};
use slog::{error, trace, Logger};
use smithay::{
    backend::input::KeyState,
    desktop::{WindowSurfaceType, utils::under_from_surface_tree},
    reexports::wayland_server::{
        protocol::{wl_pointer, wl_surface::WlSurface},
        Display, DisplayHandle, Resource,
    },
    utils::{Logical, Point},
    wayland::{
        seat::{self, AxisFrame, ButtonEvent, FilterResult, MotionEvent, PointerHandle},
        SERIAL_COUNTER,
    },
};

use crate::{
    client_state::{ClientSeat, DesktopClientState, Focus},
    server_state::{EmbeddedServerState, SeatPair},
    shared_state::GlobalState,
    space::WrapperSpace,
};

pub fn send_keyboard_event<W: WrapperSpace + 'static>(
    event: wl_keyboard::Event,
    seat_name: &str,
    mut dispatch_data: DispatchData<'_>,
) {
    let (state, server_display) = dispatch_data
        .get::<(GlobalState<W>, Display<GlobalState<W>>)>()
        .unwrap();
    let dh = server_display.handle();
    let space = &mut state.space;
    let DesktopClientState {
        kbd_focus,
        last_input_serial,
        ..
    } = &mut state.desktop_client_state;
    let EmbeddedServerState {
        focused_surface,
        seats,
        ..
    } = &mut state.embedded_server_state;

    if let Some(seat) = seats.iter().find(|SeatPair { name, .. }| name == seat_name) {
        let kbd = match seat.server.get_keyboard() {
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
                    &dh,
                    key,
                    state,
                    SERIAL_COUNTER.next_serial(),
                    time,
                    move |_modifiers, _keysym| {
                        // TODO load shortcuts and intercept them
                        FilterResult::Forward // TODO intercept some key presses maybe
                    },
                ) {
                    Some(_) => {}
                    None => {}
                }
            }
            wl_keyboard::Event::RepeatInfo { rate, delay } => {
                kbd.change_repeat_info(rate, delay);
            }
            wl_keyboard::Event::Enter { surface: _, .. } => {
                // println!("kbd entered");

                // TODO data device
                // let _ = set_data_device_selection(
                //     dh,
                //     &seat.client.seat,
                //     &seat.server,
                //     &selected_data_provider.seat,
                // );
                let client = focused_surface
                    .borrow()
                    .as_ref()
                    .and_then(|focused_surface| focused_surface.client_id());

                // TODO data device
                // set_data_device_focus(&seat.server.0, client);
                *kbd_focus = true;
                kbd.set_focus(
                    &dh,
                    focused_surface.borrow().as_ref(),
                    SERIAL_COUNTER.next_serial(),
                );
            }
            wl_keyboard::Event::Leave { .. } => {
                *kbd_focus = false;
                space.close_popups();
                kbd.set_focus(&dh, None, SERIAL_COUNTER.next_serial());
            }
            _ => (),
        };
    }
}

pub fn send_pointer_event<W: WrapperSpace + 'static>(
    event: c_wl_pointer::Event,
    seat_name: &str,
    mut dispatch_data: DispatchData<'_>,
) {
    let (global_state, server_display) = dispatch_data
        .get::<(GlobalState<W>, Display<GlobalState<W>>)>()
        .unwrap();
    let dh = server_display.handle();
    let space = &mut global_state.space;
    let DesktopClientState {
        axis_frame,
        last_input_serial,
        focused_surface: c_focused_surface,
        ..
    } = &mut global_state.desktop_client_state;
    let EmbeddedServerState {
        seats,
        last_button,
        focused_surface,
        ..
    } = &mut global_state.embedded_server_state;
    let start_time = global_state.start_time;
    let time = start_time.elapsed().as_millis();
    if let Some((Some(ptr), kbd)) = seats
        .iter()
        .position(|SeatPair { name, .. }| name == seat_name)
        .map(|idx| &seats[idx])
        .map(|seat| (seat.server.get_pointer(), seat.server.get_keyboard()))
    {
        match event {
            c_wl_pointer::Event::Motion {
                time: _time,
                surface_x,
                surface_y,
            } => {
                space.update_pointer((surface_x as i32, surface_y as i32));
                let focused_surface = focused_surface.clone();
                let c_focused_surface = c_focused_surface.clone();
                handle_motion(
                    &dh,
                    global_state,
                    &focused_surface,
                    &c_focused_surface,
                    surface_x,
                    surface_y,
                    ptr,
                    time as u32,
                );
            }
            c_wl_pointer::Event::Button {
                time: _time,
                button,
                state,
                serial,
                ..
            } => {
                last_input_serial.replace(serial);
                last_button.replace(button);

                if let Focus::Current(c_focused_surface) = c_focused_surface {
                    space.handle_button(c_focused_surface);
                }
                if let Some(kbd) = kbd.as_ref() {
                    kbd.set_focus(
                        &dh,
                        focused_surface.borrow().as_ref(),
                        SERIAL_COUNTER.next_serial(),
                    );
                }
                if let Ok(button_state) = wl_pointer::ButtonState::try_from(state as u32) {
                    ptr.button(
                        global_state,
                        &dh,
                        &ButtonEvent {
                            serial: SERIAL_COUNTER.next_serial(),
                            time: time as u32,
                            button,
                            state: button_state,
                        },
                    );
                }
            }
            c_wl_pointer::Event::Axis { time, axis, value } => {
                let mut af = axis_frame
                    .frame
                    .take()
                    .unwrap_or_else(|| AxisFrame::new(time));
                if let Some(axis_source) = axis_frame.source.take() {
                    af = af.source(axis_source);
                }
                if let Ok(axis) = wl_pointer::Axis::try_from(axis as u32) {
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
                    ptr.axis(global_state, &dh, af);
                }
                // axis_frame.h_discrete = None;
                // axis_frame.v_discrete = None;
            }
            c_wl_pointer::Event::AxisSource { axis_source } => {
                // add source to the current frame if it exists
                let source = wl_pointer::AxisSource::try_from(axis_source as u32);
                if let Some(af) = axis_frame.frame.as_mut() {
                    if let Ok(source) = source {
                        *af = af.source(source);
                    }
                } else {
                    // hold on to source and add to the next frame
                    axis_frame.source = source.ok();
                }
            }
            c_wl_pointer::Event::AxisStop { time, axis } => {
                let mut af = axis_frame
                    .frame
                    .take()
                    .unwrap_or_else(|| AxisFrame::new(time));
                if let Ok(axis) = wl_pointer::Axis::try_from(axis as u32) {
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
                _ => (),
            },
            c_wl_pointer::Event::Enter { surface, .. } => {
                // if not popup, then must be a panel layer shell surface
                // TODO better handling of subsurfaces?
                *c_focused_surface = Focus::Current(surface);
            }
            c_wl_pointer::Event::Leave { surface, .. } => {
                if let Focus::Current(s) = c_focused_surface {
                    if s == &surface {
                        *c_focused_surface = Focus::LastFocus(Instant::now());
                        focused_surface.take();
                    }
                }
                let focused_surface = focused_surface.clone();
                let c_focused_surface = c_focused_surface.clone();
                handle_motion(
                    &dh,
                    global_state,
                    &focused_surface,
                    &c_focused_surface,
                    -1.0,
                    -1.0,
                    ptr,
                    time as u32,
                );
            }
            _ => (),
        };
    }
}

pub fn seat_handle_callback<W: WrapperSpace + 'static>(
    log: Logger,
    seat: Attached<c_wl_seat::WlSeat>,
    seat_data: &SeatData,
    mut dispatch_data: DispatchData<'_>,
) {
    let (state, server_display) = dispatch_data
        .get::<(GlobalState<W>, Display<GlobalState<W>>)>()
        .unwrap();
    // let DesktopClientState {
    //     env_handle, ..
    // } = &mut state.desktop_client_state;
    let EmbeddedServerState { seats, .. } = &mut state.embedded_server_state;
    let dh = server_display.handle();
    let logger = &state.log;
    // find the seat in the vec of seats, or insert it if it is unknown
    trace!(logger, "seat event: {:?} {:?}", seat, seat_data);

    let seat_name = seat_data.name.clone();
    let idx = seats
        .iter()
        .position(|SeatPair { name, .. }| name == &seat_name);
    let idx = idx.unwrap_or_else(|| {
        seats.push(SeatPair {
            name: seat_name.clone(),
            server: seat::Seat::new(&dh, seat_name.clone(), log.clone()),
            client: ClientSeat {
                kbd: None,
                ptr: None,
                seat: seat.clone(),
            },
        });
        seats.len()
    });

    let SeatPair {
        client:
            ClientSeat {
                kbd: ref mut opt_kbd,
                ptr: ref mut opt_ptr,
                ..
            },
        server: ref mut server_seat,
        ..
    } = &mut seats[idx];
    // we should map a keyboard if the seat has the capability & is not defunct
    if (seat_data.has_keyboard || seat_data.has_pointer) && !seat_data.defunct {
        if opt_kbd.is_none() {
            // we should initalize a keyboard
            let kbd = seat.get_keyboard();
            kbd.quick_assign(move |_, event, dispatch_data| {
                send_keyboard_event::<W>(event, &seat_name, dispatch_data)
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
                send_pointer_event::<W>(event, &seat_name, dispatch_data)
            });
            server_seat.add_pointer(move |_new_status| {});
            *opt_ptr = Some(pointer.detach());
        }
        // TODO data device
        // let _ = set_data_device_selection(
        //     &dh,
        //     env_handle,
        //     &seat,
        //     server_seat,
        //     &selected_data_provider.seat,
        // );
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

// pub(crate) fn set_server_device_selection(
//     env_handle: &Environment<Env>,
//     seat: &Attached<c_wl_seat::WlSeat>,
//     server_seat: &seat::Seat,
//     selected_data_provider: &RefCell<Option<Attached<c_wl_seat::WlSeat>>>,
// ) -> Result<()> {
//     env_handle.with_data_device(seat, |data_device| {
//         data_device.with_selection(|offer| {
//             if let Some(offer) = offer {
//                 offer.with_mime_types(|types| {
//                     set_data_device_selection(server_seat, types.into());
//                 })
//             }
//         })
//     })?;
//     selected_data_provider.replace(Some(seat.clone()));
//     Ok(())
// }

pub(crate) fn handle_motion<W: WrapperSpace>(
    dh: &DisplayHandle,
    global_state: &mut GlobalState<W>,
    s_focused_surface: &Rc<RefCell<Option<WlSurface>>>,
    c_focused_surface: &Focus,
    surface_x: f64,
    surface_y: f64,
    ptr: PointerHandle<GlobalState<W>>,
    time: u32,
) {
    let c_focused_surface = match c_focused_surface {
        Focus::Current(s) => s,
        Focus::LastFocus(_) => return,
    };
    // let motion_point = global_state.space.point_to_compositor_space(&c_focused_surface, (surface_x as i32, surface_y as i32).into());
    let mut motion_point: Point<i32, Logical> = (surface_x as i32, surface_y as i32).into();
    if let Some(p) = global_state.space.popups().iter().find(|p| &p.c_wl_surface == c_focused_surface) {
        motion_point += p.position;
        s_focused_surface.replace(Some(p.s_surface.wl_surface().clone()));
        ptr.motion(
            global_state,
            &dh,
            &MotionEvent {
                location: motion_point.to_f64(),
                focus: Some((p.s_surface.wl_surface().clone(), p.position)),
                serial: SERIAL_COUNTER.next_serial(),
                time,
            },
        );
    } else {
    match global_state.space.space().surface_under((surface_x, surface_y), WindowSurfaceType::ALL) {
        Some((w, s, p)) => {
            ptr.motion(
                global_state,
                &dh,
                &MotionEvent {
                    location: motion_point.to_f64() - w.geometry().loc.to_f64(),
                    focus: Some((s, p)),
                    serial: SERIAL_COUNTER.next_serial(),
                    time,
                },
            );
        }
        None => {
            ptr.motion(
                global_state,
                &dh,
                &MotionEvent {
                    location: (surface_x, surface_y).into(),
                    focus: None,
                    serial: SERIAL_COUNTER.next_serial(),
                    time,
                },
            );
        }
    }
}
}
