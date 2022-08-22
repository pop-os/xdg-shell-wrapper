use std::time::Instant;

use crate::{
    client_state::FocusStatus,
    server_state::{SeatPair, ServerPointerFocus},
    shared_state::GlobalState,
    space::WrapperSpace,
};
use sctk::{delegate_pointer, seat::pointer::PointerHandler};
use smithay::{
    reexports::wayland_server::{
        self,
        protocol::wl_pointer::{Axis, ButtonState},
        Resource,
    },
    utils::Point,
    wayland::{
        seat::{AxisFrame, ButtonEvent, MotionEvent},
        SERIAL_COUNTER,
    },
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
        let dh = self.server_state.display_handle.clone();

        let (seat_name, ptr, kbd) = if let Some((name, Some(ptr), Some(kbd))) = self
            .server_state
            .seats
            .iter()
            .find(|SeatPair { client, .. }| {
                client.ptr.as_ref().map(|p| p == pointer).unwrap_or(false)
            })
            .map(|seat| {
                (
                    seat.name.as_str(),
                    seat.server.get_pointer(),
                    seat.server.get_keyboard(),
                )
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
                        &dh,
                        &MotionEvent {
                            location: (0.0, 0.0).into(),
                            focus: None,
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
                        ptr.motion(
                            self,
                            &dh,
                            &MotionEvent {
                                location: c_pos.to_f64() + Point::from((surface_x, surface_y)),
                                focus: Some((surface.clone(), s_pos)),
                                serial: SERIAL_COUNTER.next_serial(),
                                time: time.try_into().unwrap(),
                            },
                        );
                    } else {
                        ptr.motion(
                            self,
                            &dh,
                            &MotionEvent {
                                location: Point::from((surface_x, surface_y)),
                                focus: None,
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
                            &dh,
                            &MotionEvent {
                                location: c_pos.to_f64() + Point::from((surface_x, surface_y)),
                                focus: Some((surface.clone(), s_pos)),
                                serial: SERIAL_COUNTER.next_serial(),
                                time,
                            },
                        );
                    } else {
                        ptr.motion(
                            self,
                            &dh,
                            &MotionEvent {
                                location: Point::from((surface_x, surface_y)),
                                focus: None,
                                serial: SERIAL_COUNTER.next_serial(),
                                time,
                            },
                        );
                    }
                }
                sctk::seat::pointer::PointerEventKind::Press { time, button, .. } => {
                    self.server_state.last_button.replace(button);

                    let s = self.space.handle_press(&seat_name);

                    if let Some(client_id) = s.as_ref().and_then(|s| s.client_id()) {
                        if !kbd.has_focus(&client_id) {
                            kbd.set_focus(&dh, s.as_ref(), SERIAL_COUNTER.next_serial());
                        }
                    } else {
                        kbd.set_focus(&dh, s.as_ref(), SERIAL_COUNTER.next_serial());
                    }

                    ptr.button(
                        self,
                        &dh,
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

                    if let Some(client_id) = s.as_ref().and_then(|s| s.client_id()) {
                        if !kbd.has_focus(&client_id) {
                            kbd.set_focus(&dh, s.as_ref(), SERIAL_COUNTER.next_serial());
                        }
                    } else {
                        kbd.set_focus(&dh, s.as_ref(), SERIAL_COUNTER.next_serial());
                    }

                    ptr.button(
                        self,
                        &dh,
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
                    let source = match source.map(|s| {
                        wayland_server::protocol::wl_pointer::AxisSource::try_from(s as u32)
                    }) {
                        Some(Ok(s)) => s,
                        _ => continue,
                    };

                    let mut af = AxisFrame::new(time).source(source);

                    if !horizontal.is_none() {
                        if horizontal.discrete > 0 {
                            af = af.discrete(Axis::HorizontalScroll, horizontal.discrete)
                        }
                        if horizontal.absolute.abs() > 0.0 {
                            af = af.value(Axis::HorizontalScroll, horizontal.absolute);
                        }
                        if horizontal.stop {
                            af.stop(Axis::HorizontalScroll);
                        }
                    }

                    if !vertical.is_none() {
                        if vertical.discrete > 0 {
                            af = af.discrete(Axis::VerticalScroll, vertical.discrete)
                        }
                        af = af.value(Axis::VerticalScroll, vertical.absolute);
                        if vertical.stop {
                            af.stop(Axis::VerticalScroll);
                        }
                    }

                    ptr.axis(self, &dh, af);

                    // c_wl_pointer::Event::Axis { time, axis, value } => {
                    //     let axis_frame =
                    //         if let Some(af) = axis_frame.iter_mut().find(|af| af.seat_name == seat_name) {
                    //             af
                    //         } else {
                    //             let mut new_afd = AxisFrameData::default();
                    //             new_afd.seat_name = seat_name.to_string();
                    //             axis_frame.push(new_afd);
                    //             axis_frame.last_mut().unwrap()
                    //         };

                    //     let af = if let Some(af) = &mut axis_frame.frame {
                    //         af
                    //     } else {
                    //         axis_frame.frame.replace(AxisFrame::new(time));
                    //         axis_frame.frame.as_mut().unwrap()
                    //     };

                    //     if let Some(axis_source) = axis_frame.source.take() {
                    //         *af = af.source(axis_source);
                    //     }
                    //     if let Ok(axis) = wl_pointer::Axis::try_from(axis as u32) {
                    //         match axis {
                    //             wl_pointer::Axis::HorizontalScroll => {
                    //                 if let Some(discrete) = axis_frame.h_discrete {
                    //                     *af = af.discrete(axis, discrete);
                    //                 }
                    //             }
                    //             wl_pointer::Axis::VerticalScroll => {
                    //                 if let Some(discrete) = axis_frame.v_discrete {
                    //                     *af = af.discrete(axis, discrete);
                    //                 }
                    //             }
                    //             _ => return,
                    //         }
                    //         *af = af.value(axis, value);
                    //     }
                    // }
                    // c_wl_pointer::Event::Frame => {
                    //     // return if no matching axis frame
                    //     let axis_frame =
                    //         if let Some(af) = axis_frame.iter_mut().find(|af| af.seat_name == seat_name) {
                    //             af
                    //         } else {
                    //             return;
                    //         };
                    //     if let Some(af) = axis_frame.frame.take() {
                    //         ptr.axis(global_state, &dh, af);
                    //     }
                    //     // axis_frame.h_discrete = None;
                    //     // axis_frame.v_discrete = None;
                    // }
                    // c_wl_pointer::Event::AxisSource { axis_source } => {
                    //     // add source to the current frame if it exists
                    //     let mut axis_frame =
                    //         if let Some(af) = axis_frame.iter_mut().find(|af| af.seat_name == seat_name) {
                    //             af
                    //         } else {
                    //             let mut new_afd = AxisFrameData::default();
                    //             new_afd.seat_name = seat_name.to_string();
                    //             axis_frame.push(new_afd);
                    //             axis_frame.last_mut().unwrap()
                    //         };
                    //     let source = wl_pointer::AxisSource::try_from(axis_source as u32);
                    //     if let Some(af) = axis_frame.frame.as_mut() {
                    //         if let Ok(source) = source {
                    //             *af = af.source(source);
                    //         }
                    //     } else {
                    //         // hold on to source and add to the next frame
                    //         axis_frame.source = source.ok();
                    //     }
                    // }
                    // c_wl_pointer::Event::AxisStop { time, axis } => {
                    //     let axis_frame =
                    //         if let Some(af) = axis_frame.iter_mut().find(|af| af.seat_name == seat_name) {
                    //             af
                    //         } else {
                    //             let mut new_afd = AxisFrameData::default();
                    //             new_afd.seat_name = seat_name.to_string();
                    //             axis_frame.push(new_afd);
                    //             axis_frame.last_mut().unwrap()
                    //         };

                    //     let af = if let Some(af) = &mut axis_frame.frame {
                    //         af
                    //     } else {
                    //         axis_frame.frame.replace(AxisFrame::new(time));
                    //         axis_frame.frame.as_mut().unwrap()
                    //     };

                    //     if let Ok(axis) = wl_pointer::Axis::try_from(axis as u32) {
                    //         *af = af.stop(axis);
                    //     }
                    // }
                    // c_wl_pointer::Event::AxisDiscrete { axis, discrete } => {
                    //     let axis_frame =
                    //         if let Some(af) = axis_frame.iter_mut().find(|af| af.seat_name == seat_name) {
                    //             af
                    //         } else {
                    //             let mut new_afd = AxisFrameData::default();
                    //             new_afd.seat_name = seat_name.to_string();
                    //             axis_frame.push(new_afd);
                    //             axis_frame.last_mut().unwrap()
                    //         };
                    //     match axis {
                    //         c_wl_pointer::Axis::HorizontalScroll => {
                    //             axis_frame.h_discrete.replace(discrete);
                    //         }
                    //         c_wl_pointer::Axis::VerticalScroll => {
                    //             axis_frame.v_discrete.replace(discrete);
                    //         }
                    //         _ => (),
                    //     }
                    // }
                }
            }
        }
    }
}

delegate_pointer!(@<W: WrapperSpace + 'static> GlobalState<W>);
