// SPDX-License-Identifier: GPL-3.0-only

use std::cmp::min;

use crate::util::*;
use anyhow::Result;
use sctk::reexports::calloop::{self, channel};
use sctk::reexports::client::protocol::{wl_keyboard, wl_pointer, wl_shm, wl_surface};
use sctk::seat::keyboard::{self, map_keyboard_repeat, RepeatKind};
use sctk::shm::AutoMemPool;
use sctk::window::{Event as WEvent, FallbackFrame};

sctk::default_environment!(KbdInputExample, desktop);

type Seat = (
    Option<(wl_keyboard::WlKeyboard, calloop::RegistrationToken)>,
    Option<wl_pointer::WlPointer>,
);

pub fn new_client(
    client_tx: channel::SyncSender<ClientMsg>,
    server_rx: channel::Channel<ServerMsg>,
) -> Result<()> {
    /*
     * Initial setup
     */
    let (env, display, queue) = sctk::new_default_environment!(KbdInputExample, desktop)
        .expect("Unable to connect to a Wayland compositor");

    /*
     * Prepare a calloop event loop to handle key repetion
     */
    // Here `Option<WEvent>` is the type of a global value that will be shared by
    // all callbacks invoked by the event loop.
    let mut event_loop = calloop::EventLoop::<Option<WEvent>>::try_new().unwrap();
    /*
     * Create a buffer with window contents
     */

    let mut dimensions = (320u32, 240u32);

    /*
     * Init wayland objects
     */

    let surface = env.create_surface().detach();

    let mut window = env
        .create_window::<FallbackFrame, _>(
            surface,
            None,
            dimensions,
            move |evt, mut dispatch_data| {
                let next_action = dispatch_data.get::<Option<WEvent>>().unwrap();
                // Keep last event in priority order : Close > Configure > Refresh
                let replace = matches!(
                    (&evt, &*next_action),
                    (_, &None)
                        | (_, &Some(WEvent::Refresh))
                        | (&WEvent::Configure { .. }, &Some(WEvent::Configure { .. }))
                        | (&WEvent::Close, _)
                );
                if replace {
                    *next_action = Some(evt);
                }
            },
        )
        .expect("Failed to create a window !");

    window.set_title("Kbd Input".to_string());

    let mut pool = env
        .create_auto_pool()
        .expect("Failed to create a memory pool !");

    /*
     * Keyboard initialization
     */

    let mut seats = Vec::<(String, Seat)>::new();

    // first process already existing seats
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
                    let tx = client_tx.clone();
                    match map_keyboard_repeat(
                        event_loop.handle(),
                        &seat,
                        None,
                        RepeatKind::System,
                        move |event, _, _| send_keyboard_event(event, &seat_name, tx.clone()),
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
                    let surface = window.surface().clone();
                    let tx = client_tx.clone();
                    pointer.quick_assign(move |_, event, _| {
                        send_pointer_event(event, &seat_name, &surface, tx.clone())
                    });
                    new_seat.1 = Some(pointer.detach());
                }
                seats.push((name.clone(), new_seat));
            } else {
                seats.push((name, (None, None)));
            }
        }
    }

    // then setup a listener for changes
    let loop_handle = event_loop.handle();
    let main_surface = window.surface().clone();

    let tx = client_tx.clone();
    let _seat_listener = env.listen_for_seats(move |seat, seat_data, _| {
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
                let tx_ = tx.clone();
                match map_keyboard_repeat(
                    loop_handle.clone(),
                    &seat,
                    None,
                    RepeatKind::System,
                    move |event, _, _| send_keyboard_event(event, &seat_name, tx_.clone()),
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
                let surface = main_surface.clone();
                let tx_ = tx.clone();
                pointer.quick_assign(move |_, event, _| {
                    send_pointer_event(event, &seat_name, &surface, tx_.clone())
                });
                (*opt_seat).1 = Some(pointer.detach());
            }
        } else {
            let (kbd_seat, ptr_seat) = opt_seat;
            //cleanup
            if let Some((kbd, source)) = kbd_seat.take() {
                kbd.release();
                loop_handle.remove(source);
            }
            if let Some(ptr) = ptr_seat.take() {
                ptr.release();
            }
        }
    });

    if !env.get_shell().unwrap().needs_configure() {
        // initial draw to bootstrap on wl_shell
        redraw(&mut pool, window.surface(), dimensions).expect("Failed to draw");
        window.refresh();
    }

    let mut next_action: Option<WEvent> = None;

    sctk::WaylandSource::new(queue)
        .quick_insert(event_loop.handle())
        .unwrap();

    // handle messages from embedded wayland server
    event_loop
        .handle()
        .insert_source(
            server_rx,
            move |event, _metadata, _shared_data| match event {
                channel::Event::Msg(e) => match e {
                    ServerMsg::Other => {
                        println!("hello");
                    }
                },
                _ => {}
            },
        )
        .unwrap();

    // handles messages with desktop wayland server
    loop {
        if let Some(event) = next_action.take() {
            let _ = (&client_tx).clone().send(ClientMsg::WEvent(event.clone()));
            match event {
                WEvent::Close => break,
                WEvent::Refresh => {
                    window.refresh();
                    window.surface().commit();
                }
                WEvent::Configure { new_size, states } => {
                    if let Some((w, h)) = new_size {
                        window.resize(w, h);
                        dimensions = (w, h)
                    }
                    println!("Window states: {:?}", states);
                    window.refresh();
                    redraw(&mut pool, window.surface(), dimensions).expect("Failed to draw");
                }
            }
        }

        // always flush the connection before going to sleep waiting for events
        display.flush().unwrap();

        event_loop.dispatch(None, &mut next_action).unwrap();
    }
    Ok(())
}

fn send_keyboard_event(
    event: keyboard::Event,
    _seat_name: &str,
    tx: channel::SyncSender<ClientMsg>,
) {
    let e: KbEvent = event.into();
    let _ = tx.send(ClientMsg::KbEvent(e.clone()));
}

fn send_pointer_event(
    event: wl_pointer::Event,
    _seat_name: &str,
    _main_surface: &wl_surface::WlSurface,
    tx: channel::SyncSender<ClientMsg>,
) {
    let _ = tx.send(ClientMsg::PtrEvent(event));
}

#[allow(clippy::many_single_char_names)]
fn redraw(
    pool: &mut AutoMemPool,
    surface: &wl_surface::WlSurface,
    (buf_x, buf_y): (u32, u32),
) -> Result<(), ::std::io::Error> {
    let (canvas, new_buffer) = pool.buffer(
        buf_x as i32,
        buf_y as i32,
        4 * buf_x as i32,
        wl_shm::Format::Argb8888,
    )?;
    for (i, dst_pixel) in canvas.chunks_exact_mut(4).enumerate() {
        let x = i as u32 % buf_x;
        let y = i as u32 / buf_x;
        let r: u32 = min(((buf_x - x) * 0xFF) / buf_x, ((buf_y - y) * 0xFF) / buf_y);
        let g: u32 = min((x * 0xFF) / buf_x, ((buf_y - y) * 0xFF) / buf_y);
        let b: u32 = min(((buf_x - x) * 0xFF) / buf_x, (y * 0xFF) / buf_y);
        let pixel: [u8; 4] = ((0xFF << 24) + (r << 16) + (g << 8) + b).to_ne_bytes();
        dst_pixel[0] = pixel[0];
        dst_pixel[1] = pixel[1];
        dst_pixel[2] = pixel[2];
        dst_pixel[3] = pixel[3];
    }
    surface.attach(Some(&new_buffer), 0, 0);
    if surface.as_ref().version() >= 4 {
        surface.damage_buffer(0, 0, buf_x as i32, buf_y as i32);
    } else {
        surface.damage(0, 0, buf_x as i32, buf_y as i32);
    }
    surface.commit();
    Ok(())
}
