// SPDX-License-Identifier: GPL-3.0-only

use std::cmp::min;

use crate::util::*;
use crate::XdgWrapperConfig;
use anyhow::Result;
use sctk::{
    default_environment,
    environment::SimpleGlobal,
    output::{with_output_info, OutputInfo, OutputStatusListener},
    reexports::{
        calloop, client,
        client::protocol::{wl_output, wl_pointer, wl_shm, wl_surface},
        client::{Attached, Main},
    },
    seat::{
        keyboard::{self, map_keyboard_repeat, RepeatKind},
        SeatListener,
    },
    shm::AutoMemPool,
};
use smithay::reexports::wayland_protocols::wlr::unstable::layer_shell::v1::client::{
    zwlr_layer_shell_v1, zwlr_layer_surface_v1,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

sctk::default_environment!(KbdInputExample, desktop);

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum RenderEvent {
    Configure { width: u32, height: u32 },
    Closed,
}

default_environment!(Env,
                     fields = [
                         layer_shell: SimpleGlobal<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
                     ],
                     singles = [
                         zwlr_layer_shell_v1::ZwlrLayerShellV1 => layer_shell
                     ],
);
#[derive(Debug)]
pub struct DesktopClientState {
    pub display: client::Display,
    pub seats: Vec<Seat>,
    pub seat_listener: SeatListener,
    pub output_listener: OutputStatusListener,
    pub surface: Rc<RefCell<Option<(u32, Surface)>>>,
}

#[derive(Debug)]
pub struct Surface {
    pub surface: wl_surface::WlSurface,
    pub layer_surface: Main<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>,
    pub next_render_event: Rc<Cell<Option<RenderEvent>>>,
    pub pool: AutoMemPool,
    pub dimensions: (u32, u32),
    pub config: XdgWrapperConfig,
}

impl Surface {
    fn new(
        output: &wl_output::WlOutput,
        surface: wl_surface::WlSurface,
        layer_shell: &Attached<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
        pool: AutoMemPool,
        config: XdgWrapperConfig,
    ) -> Self {
        let layer_surface = layer_shell.get_layer_surface(
            &surface,
            Some(output),
            config.layer.into(),
            "example".to_owned(),
        );

        layer_surface.set_anchor(config.anchor.into());
        layer_surface.set_keyboard_interactivity(config.keyboard_interactivity.into());
        let (x, y) = config.dimensions;
        layer_surface.set_size(x, y);
        // Anchor to the top left corner of the output

        let next_render_event = Rc::new(Cell::new(None::<RenderEvent>));
        let next_render_event_handle = Rc::clone(&next_render_event);
        layer_surface.quick_assign(move |layer_surface, event, _| {
            match (event, next_render_event_handle.get()) {
                (zwlr_layer_surface_v1::Event::Closed, _) => {
                    println!("closing");
                    next_render_event_handle.set(Some(RenderEvent::Closed));
                }
                (
                    zwlr_layer_surface_v1::Event::Configure {
                        serial,
                        width,
                        height,
                    },
                    next,
                ) if next != Some(RenderEvent::Closed) => {
                    layer_surface.ack_configure(serial);
                    next_render_event_handle.set(Some(RenderEvent::Configure { width, height }));
                }
                (_, _) => {}
            }
        });

        // Commit so that the server will send a configure event
        surface.commit();

        Self {
            surface,
            layer_surface,
            next_render_event,
            pool,
            dimensions: (0, 0),
            config,
        }
    }

    /// Handles any events that have occurred since the last call, redrawing if needed.
    /// Returns true if the surface should be dropped.
    pub fn handle_events(&mut self) -> bool {
        match self.next_render_event.take() {
            Some(RenderEvent::Closed) => true,
            Some(RenderEvent::Configure { width, height }) => {
                if self.dimensions != (width, height) {
                    self.dimensions = (width, height);
                    self.draw();
                }
                false
            }
            None => false,
        }
    }

    fn draw(&mut self) {
        let stride = 4 * self.dimensions.0 as i32;
        let width = self.dimensions.0 as i32;
        let height = self.dimensions.1 as i32;

        // Note: unwrap() is only used here in the interest of simplicity of the example.
        // A "real" application should handle the case where both pools are still in use by the
        // compositor.
        let (canvas, buffer) = self
            .pool
            .buffer(width, height, stride, wl_shm::Format::Argb8888)
            .unwrap();

        for dst_pixel in canvas.chunks_exact_mut(4) {
            let pixel = 0xff00ff00u32.to_ne_bytes();
            dst_pixel[0] = pixel[0];
            dst_pixel[1] = pixel[1];
            dst_pixel[2] = pixel[2];
            dst_pixel[3] = pixel[3];
        }

        // Attach the buffer to the surface and mark the entire surface as damaged
        self.surface.attach(Some(&buffer), 0, 0);
        self.surface
            .damage_buffer(0, 0, width as i32, height as i32);

        // Finally, commit the surface
        self.surface.commit();
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        self.layer_surface.destroy();
        self.surface.destroy();
    }
}

pub fn new_client(
    loop_handle: calloop::LoopHandle<'static, GlobalState>,
    config: XdgWrapperConfig,
) -> Result<DesktopClientState> {
    /*
     * Initial setup
     */
    let (env, display, queue) =
        sctk::new_default_environment!(Env, fields = [layer_shell: SimpleGlobal::new(),])
            .expect("Unable to connect to a Wayland compositor");

    let surface = Rc::new(RefCell::new(None));
    let layer_shell = env.require_global::<zwlr_layer_shell_v1::ZwlrLayerShellV1>();
    let env_handle = env.clone();
    let surface_handle = Rc::clone(&surface);
    let output_handler = move |output: wl_output::WlOutput, info: &OutputInfo| {
        let mut handle = surface_handle.borrow_mut();
        dbg!(&handle);
        if info.obsolete {
            // an output has been removed, release it
            handle.as_ref().filter(|(i, _)| *i != info.id);
            output.release();
        } else if handle.is_none() {
            // an output has been created, construct a surface for it
            let surface = env_handle.create_surface().detach();
            let pool = env_handle
                .create_auto_pool()
                .expect("Failed to create a memory pool!");
            *handle = Some((
                info.id,
                Surface::new(&output, surface, &layer_shell.clone(), pool, config.clone()),
            ));
            dbg!(&handle);
        }
    };

    for output in env.get_all_outputs() {
        if let Some(info) = with_output_info(&output, Clone::clone) {
            output_handler(output, &info);
        }
    }

    let output_listener =
        env.listen_for_outputs(move |output, info, _| output_handler(output, info));

    /*
     * Keyboard initialization
     */

    let mut seats = Vec::<(String, Seat)>::new();

    // first process already existing seats
    let handle = loop_handle.clone();
    for seat in env.get_all_seats() {
        if let Some((has_kbd, has_ptr, name)) = sctk::seat::with_seat_data(&seat, |seat_data| {
            (
                seat_data.has_keyboard && !seat_data.defunct,
                seat_data.has_pointer && !seat_data.defunct,
                seat_data.name.clone(),
            )
        }) {
            if has_kbd || has_ptr {
                let mut new_seat: Seat = (None, None);
                if has_kbd {
                    let seat_name = name.clone();
                    match map_keyboard_repeat(
                        handle.clone(),
                        &seat,
                        None,
                        RepeatKind::System,
                        move |event, _, _| send_keyboard_event(event, &seat_name),
                    ) {
                        Ok((kbd, repeat_source)) => {
                            new_seat.0 = Some((kbd, repeat_source));
                        }
                        Err(e) => {
                            eprintln!("Failed to map keyboard on seat {} : {:?}.", name, e);
                        }
                    }
                }
                if has_ptr {
                    let seat_name = name.clone();
                    let pointer = seat.get_pointer();
                    pointer.quick_assign(move |_, event, _| send_pointer_event(event, &seat_name));
                    new_seat.1 = Some(pointer.detach());
                }
                seats.push((name.clone(), new_seat));
            } else {
                seats.push((name, (None, None)));
            }
        }
    }

    // then setup a listener for changes

    let handle = loop_handle.clone();
    let seat_listener = env.listen_for_seats(move |seat, seat_data, _| {
        // find the seat in the vec of seats, or insert it if it is unknown
        let idx = seats.iter().position(|(name, _)| name == &seat_data.name);
        let idx = idx.unwrap_or_else(|| {
            seats.push((seat_data.name.clone(), (None, None)));
            seats.len() - 1
        });

        let (_, ref mut opt_seat) = &mut seats[idx];
        // we should map a keyboard if the seat has the capability & is not defunct
        if (seat_data.has_keyboard || seat_data.has_pointer) && !seat_data.defunct {
            if opt_seat.0.is_none() {
                // we should initalize a keyboard
                let seat_name = seat_data.name.clone();
                match map_keyboard_repeat(
                    handle.clone(),
                    &seat,
                    None,
                    RepeatKind::System,
                    move |event, _, _| send_keyboard_event(event, &seat_name),
                ) {
                    Ok((kbd, repeat_source)) => {
                        (*opt_seat).0 = Some((kbd, repeat_source));
                    }
                    Err(e) => {
                        eprintln!(
                            "Failed to map keyboard on seat {} : {:?}.",
                            seat_data.name, e
                        )
                    }
                }
            }
            if opt_seat.1.is_none() {
                // we should initalize a keyboard
                let seat_name = seat_data.name.clone();
                let pointer = seat.get_pointer();
                pointer.quick_assign(move |_, event, _| send_pointer_event(event, &seat_name));
                (*opt_seat).1 = Some(pointer.detach());
            }
        } else {
            let (kbd_seat, ptr_seat) = opt_seat;
            //cleanup
            if let Some((kbd, source)) = kbd_seat.take() {
                kbd.release();
                handle.remove(source);
            }
            if let Some(ptr) = ptr_seat.take() {
                ptr.release();
            }
        }
    });

    sctk::WaylandSource::new(queue)
        .quick_insert(loop_handle)
        .unwrap();

    Ok(DesktopClientState {
        surface,
        display,
        output_listener,
        seat_listener,
        seats: Default::default(),
    })
}

fn send_keyboard_event(event: keyboard::Event, _seat_name: &str) {
    dbg!(event);
    //TODO forward event through embedded server
}

fn send_pointer_event(event: wl_pointer::Event, _seat_name: &str) {
    // dbg!(event);
    //TODO forward event through embedded server
}
