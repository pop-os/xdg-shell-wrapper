use std::time::Instant;

use crate::{
    client_state::FocusStatus,
    server_state::{SeatPair, ServerPointerFocus},
    shared_state::GlobalState,
    space::WrapperSpace,
};
use sctk::{delegate_pointer, seat::pointer::PointerHandler};
use smithay::{
    backend::input::{self, Axis, ButtonState},
    input::pointer::{AxisFrame, ButtonEvent, MotionEvent},
    reexports::wayland_server::protocol::wl_pointer::AxisSource,
    utils::{Point, SERIAL_COUNTER},
};

impl<W: WrapperSpace> PointerHandler for GlobalState<W> {
    fn pointer_frame(
        &mut self,
        _conn: &sctk::reexports::client::Connection,
        _qh: &sctk::reexports::client::QueueHandle<Self>,
        pointer: &sctk::reexports::client::protocol::wl_pointer::WlPointer,
        events: &[sctk::seat::pointer::PointerEvent],
    ) {
        let start_time = self.start_time;
        let time = start_time.elapsed().as_millis();

        let (seat_name, ptr, kbd) = if let Some((name, Some(ptr), Some(kbd))) = self
            .server_state
            .seats
            .iter()
            .find(|SeatPair { client, .. }| {
                client.ptr.as_ref().map(|p| p == pointer).unwrap_or(false)
            })
            .map(|seat| {
                let ret = (
                    seat.name.as_str(),
                    seat.server.get_pointer(),
                    seat.server.get_keyboard(),
                );
                ret
            }) {
            (name.to_string(), ptr, kbd)
        } else {
            return;
        };

        for e in events {
            match e.kind {
                sctk::seat::pointer::PointerEventKind::Leave { .. } => {
                    {
                        let mut c_hovered_surface = self.client_state.hovered_surface.borrow_mut();
                        if let Some(i) = c_hovered_surface.iter().position(|f| f.0 == e.surface) {
                            c_hovered_surface[i].2 = FocusStatus::LastFocused(Instant::now());
                        }
                    }

                    self.space
                        .pointer_leave(&seat_name, Some(e.surface.clone()));
                    ptr.motion(
                        self,
                        None,
                        &MotionEvent {
                            location: (0.0, 0.0).into(),
                            serial: SERIAL_COUNTER.next_serial(),
                            time: time.try_into().unwrap(),
                        },
                    );
                }
                sctk::seat::pointer::PointerEventKind::Enter { .. } => {
                    // if not popup, then must be a panel layer shell surface
                    // TODO better handling of subsurfaces?
                    let (surface_x, surface_y) = e.position;
                    {
                        let mut c_hovered_surface = self.client_state.hovered_surface.borrow_mut();
                        if let Some(i) = c_hovered_surface.iter().position(|f| f.1 == seat_name) {
                            c_hovered_surface[i].0 = e.surface.clone();
                            c_hovered_surface[i].2 = FocusStatus::Focused;
                        } else {
                            c_hovered_surface.push((
                                e.surface.clone(),
                                seat_name.to_string(),
                                FocusStatus::Focused,
                            ));
                        }
                    }

                    if let Some(ServerPointerFocus {
                        surface,
                        c_pos,
                        s_pos,
                        ..
                    }) = self.space.update_pointer(
                        (surface_x as i32, surface_y as i32),
                        &seat_name,
                        e.surface.clone(),
                    ) {
                        // ptr.set_grab(self, GrabStartData { focus: Some((surface.clone(), s_pos)), button: 0, location: c_pos.to_f64() + Point::from((surface_x, surface_y)) }, SERIAL_COUNTER.next_serial(), Focus::Keep);
                        ptr.motion(
                            self,
                            Some((surface.clone(), s_pos)),
                            &MotionEvent {
                                location: c_pos.to_f64() + Point::from((surface_x, surface_y)),
                                serial: SERIAL_COUNTER.next_serial(),
                                time: time.try_into().unwrap(),
                            },
                        );
                    } else {
                        ptr.motion(
                            self,
                            None,
                            &MotionEvent {
                                location: Point::from((surface_x, surface_y)),
                                serial: SERIAL_COUNTER.next_serial(),
                                time: time.try_into().unwrap(),
                            },
                        );
                    }
                }
                sctk::seat::pointer::PointerEventKind::Motion { time } => {
                    let (surface_x, surface_y) = e.position;

                    let c_focused_surface = match self
                        .client_state
                        .hovered_surface
                        .borrow()
                        .iter()
                        .find(|f| f.1.as_str() == seat_name)
                    {
                        Some(f) => f.0.clone(),
                        None => return,
                    };

                    if let Some(ServerPointerFocus {
                        surface,
                        c_pos,
                        s_pos,
                        ..
                    }) = self.space.update_pointer(
                        (surface_x as i32, surface_y as i32),
                        &seat_name,
                        c_focused_surface,
                    ) {
                        ptr.motion(
                            self,
                            Some((surface.clone(), s_pos)),
                            &MotionEvent {
                                location: c_pos.to_f64() + Point::from((surface_x, surface_y)),
                                serial: SERIAL_COUNTER.next_serial(),
                                time,
                            },
                        );
                    } else {
                        ptr.motion(
                            self,
                            None,
                            &MotionEvent {
                                location: Point::from((surface_x, surface_y)),
                                serial: SERIAL_COUNTER.next_serial(),
                                time,
                            },
                        );
                    }
                }
                sctk::seat::pointer::PointerEventKind::Press { time, button, .. } => {
                    self.server_state.last_button.replace(button);

                    let s = self.space.handle_press(&seat_name);
                    kbd.set_focus(self, s, SERIAL_COUNTER.next_serial());

                    ptr.button(
                        self,
                        &ButtonEvent {
                            serial: SERIAL_COUNTER.next_serial(),
                            time: time as u32,
                            button,
                            state: ButtonState::Pressed,
                        },
                    );
                }
                sctk::seat::pointer::PointerEventKind::Release { time, button, .. } => {
                    self.server_state.last_button.replace(button);

                    let s = self.space.handle_press(&seat_name);
                    kbd.set_focus(self, s, SERIAL_COUNTER.next_serial());

                    ptr.button(
                        self,
                        &ButtonEvent {
                            serial: SERIAL_COUNTER.next_serial(),
                            time: time as u32,
                            button,
                            state: ButtonState::Released,
                        },
                    );
                }
                sctk::seat::pointer::PointerEventKind::Axis {
                    time,
                    horizontal,
                    vertical,
                    source,
                } => {
                    let source = match source.and_then(|s| {
                        AxisSource::try_from(s as u32).ok().and_then(|s| match s {
                            AxisSource::Wheel => Some(input::AxisSource::Wheel),
                            AxisSource::Finger => Some(input::AxisSource::Finger),
                            AxisSource::Continuous => Some(input::AxisSource::Continuous),
                            AxisSource::WheelTilt => Some(input::AxisSource::WheelTilt),
                            _ => None,
                        })
                    }) {
                        Some(s) => s,
                        _ => continue,
                    };

                    let mut af = AxisFrame::new(time).source(source);

                    if !horizontal.is_none() {
                        if horizontal.discrete > 0 {
                            af = af.discrete(Axis::Horizontal, horizontal.discrete)
                        }
                        if horizontal.absolute.abs() > 0.0 {
                            af = af.value(Axis::Horizontal, horizontal.absolute);
                        }
                        if horizontal.stop {
                            af.stop(Axis::Horizontal);
                        }
                    }

                    if !vertical.is_none() {
                        if vertical.discrete > 0 {
                            af = af.discrete(Axis::Vertical, vertical.discrete)
                        }
                        af = af.value(Axis::Vertical, vertical.absolute);
                        if vertical.stop {
                            af.stop(Axis::Vertical);
                        }
                    }

                    ptr.axis(self, af);
                }
            }
        }
    }
}

delegate_pointer!(@<W: WrapperSpace + 'static> GlobalState<W>);
