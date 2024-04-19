use std::time::Instant;

use crate::{
    client_state::FocusStatus,
    server_state::{SeatPair, ServerPointerFocus},
    shared_state::GlobalState,
    space::WrapperSpace,
};
use sctk::{delegate_pointer, seat::pointer::PointerHandler, shell::WaylandSurface};
use smithay::{
    backend::input::{self, Axis, ButtonState},
    input::pointer::{AxisFrame, ButtonEvent, MotionEvent},
    reexports::wayland_server::protocol::wl_pointer::AxisSource,
    utils::{Point, SERIAL_COUNTER},
};

impl<W: WrapperSpace> PointerHandler for GlobalState<W> {
    fn pointer_frame(
        &mut self,
        conn: &sctk::reexports::client::Connection,
        _qh: &sctk::reexports::client::QueueHandle<Self>,
        pointer: &sctk::reexports::client::protocol::wl_pointer::WlPointer,
        events: &[sctk::seat::pointer::PointerEvent],
    ) {
        let start_time = self.start_time;
        let time = start_time.elapsed().as_millis();

        let seat_index = self
            .server_state
            .seats
            .iter()
            .position(|SeatPair { client, .. }| {
                client
                    .ptr
                    .as_ref()
                    .map(|p| p.pointer() == pointer)
                    .unwrap_or(false)
            })
            .unwrap();
        let seat = &self.server_state.seats[seat_index].server.seat;
        let seat_name = self.server_state.seats[seat_index].name.to_string();
        let ptr = self.server_state.seats[seat_index]
            .server
            .seat
            .get_pointer()
            .unwrap();
        let kbd = self.server_state.seats[seat_index]
            .server
            .seat
            .get_keyboard()
            .unwrap();
        for e in events {
            let seat = &mut self.server_state.seats[seat_index];
            match e.kind {
                sctk::seat::pointer::PointerEventKind::Leave { .. } => {
                    ptr.motion(
                        self,
                        None,
                        &MotionEvent {
                            location: (0.0, 0.0).into(),
                            serial: SERIAL_COUNTER.next_serial(),
                            time: time.try_into().unwrap(),
                        },
                    );
                    ptr.frame(self);

                    if let Some((..)) = self
                        .client_state
                        .proxied_layer_surfaces
                        .iter_mut()
                        .find(|(_, _, _, s, _, _, _, _)| s.wl_surface() == &e.surface)
                    {
                        continue;
                    }

                    {
                        let mut c_hovered_surface = self.client_state.hovered_surface.borrow_mut();
                        if let Some(i) = c_hovered_surface.iter().position(|f| f.0 == e.surface) {
                            c_hovered_surface[i].2 = FocusStatus::LastFocused(Instant::now());
                        }
                    }

                    self.space
                        .pointer_leave(&seat_name, Some(e.surface.clone()));
                }
                sctk::seat::pointer::PointerEventKind::Enter { serial } => {
                    seat.client.last_enter = serial;

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

                    // check tracked layer shell surface
                    let s_surface = self
                        .client_state
                        .proxied_layer_surfaces
                        .iter_mut()
                        .find_map(|(_, _, s, c, _, _, _, _)| {
                            if c.wl_surface() == &e.surface {
                                Some(s.wl_surface().clone())
                            } else {
                                None
                            }
                        });
                    if let Some(s_surface) = s_surface {
                        ptr.motion(
                            self,
                            Some((s_surface, Point::default())),
                            &MotionEvent {
                                location: Point::from((surface_x, surface_y)),
                                serial: SERIAL_COUNTER.next_serial(),
                                time: time.try_into().unwrap(),
                            },
                        );
                        ptr.frame(self);

                        continue;
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
                        ptr.motion(
                            self,
                            Some((surface.clone(), s_pos)),
                            &MotionEvent {
                                location: c_pos.to_f64() + Point::from((surface_x, surface_y)),
                                serial: SERIAL_COUNTER.next_serial(),
                                time: time.try_into().unwrap(),
                            },
                        );
                        ptr.frame(self);
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
                        ptr.frame(self);
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

                    // check tracked layer shell surface
                    let s_surface = self
                        .client_state
                        .proxied_layer_surfaces
                        .iter_mut()
                        .find_map(|(_, _, s, c, _, _, ..)| {
                            if c.wl_surface() == &e.surface {
                                Some(s.wl_surface().clone())
                            } else {
                                None
                            }
                        });
                    if let Some(s_surface) = s_surface {
                        ptr.motion(
                            self,
                            Some((s_surface, Point::default())),
                            &MotionEvent {
                                location: Point::from((surface_x, surface_y)),
                                serial: SERIAL_COUNTER.next_serial(),
                                time: time.try_into().unwrap(),
                            },
                        );
                        ptr.frame(self);

                        continue;
                    }

                    let prev_focus = ptr.current_focus().is_some();
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
                        ptr.frame(self);
                    } else {
                        if prev_focus {
                            ptr.motion(
                                self,
                                None,
                                &MotionEvent {
                                    location: Point::from((surface_x, surface_y)),
                                    serial: SERIAL_COUNTER.next_serial(),
                                    time,
                                },
                            );
                            ptr.frame(self);
                            if let Some(themed_pointer) =
                                &self.server_state.seats[seat_index].client.ptr
                            {
                                _ = themed_pointer
                                    .set_cursor(conn, sctk::seat::pointer::CursorIcon::Default);
                            }
                        }
                    }
                }
                sctk::seat::pointer::PointerEventKind::Press {
                    time,
                    button,
                    serial,
                    ..
                } => {
                    self.server_state.last_button.replace(button);
                    seat.client.last_pointer_press = (serial, time);
                    // check tracked layer shell surface
                    let s_surface = self
                        .client_state
                        .proxied_layer_surfaces
                        .iter_mut()
                        .find_map(|(_, _, s, c, _, _, ..)| {
                            if c.wl_surface() == &e.surface {
                                Some(s.wl_surface().clone())
                            } else {
                                None
                            }
                        });
                    if let Some(s_surface) = s_surface {
                        kbd.set_focus(self, Some(s_surface), SERIAL_COUNTER.next_serial());

                        ptr.button(
                            self,
                            &ButtonEvent {
                                serial: SERIAL_COUNTER.next_serial(),
                                time: time as u32,
                                button,
                                state: ButtonState::Pressed,
                            },
                        );
                        ptr.frame(self);

                        continue;
                    }

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
                    ptr.frame(self);
                }
                sctk::seat::pointer::PointerEventKind::Release { time, button, .. } => {
                    self.server_state.last_button.replace(button);

                    // check tracked layer shell surface
                    let s_surface = self
                        .client_state
                        .proxied_layer_surfaces
                        .iter_mut()
                        .find_map(|(_, _, s, c, _, _, ..)| {
                            if c.wl_surface() == &e.surface {
                                Some(s.wl_surface().clone())
                            } else {
                                None
                            }
                        });
                    if let Some(s_surface) = s_surface {
                        kbd.set_focus(self, Some(s_surface), SERIAL_COUNTER.next_serial());

                        ptr.button(
                            self,
                            &ButtonEvent {
                                serial: SERIAL_COUNTER.next_serial(),
                                time: time as u32,
                                button,
                                state: ButtonState::Released,
                            },
                        );
                        ptr.frame(self);

                        continue;
                    }

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
                    ptr.frame(self);
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
                        if horizontal.discrete.abs() > 0 {
                            af = af.v120(Axis::Horizontal, horizontal.discrete * 120);
                        }
                        if horizontal.absolute.abs() > 0.0 {
                            af = af.value(Axis::Horizontal, horizontal.absolute * 120.);
                        }
                        if horizontal.stop {
                            af.stop(Axis::Horizontal);
                        }
                    }

                    if !vertical.is_none() {
                        if vertical.discrete.abs() > 0 {
                            af = af.v120(Axis::Vertical, vertical.discrete * 120);
                        }
                        if vertical.absolute.abs() > 0.0 {
                            af = af.value(Axis::Vertical, vertical.absolute * 120.);
                        }
                        if vertical.stop {
                            af.stop(Axis::Vertical);
                        }
                    }

                    ptr.axis(self, af);
                    ptr.frame(self);
                }
            }
        }
    }
}

delegate_pointer!(@<W: WrapperSpace + 'static> GlobalState<W>);
