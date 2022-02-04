// SPDX-License-Identifier: GPL-3.0-only

use std::cmp::min;

use crate::util::*;
use anyhow::Result;
use sctk::reexports::calloop;
use sctk::reexports::client::protocol::{wl_pointer, wl_shm, wl_surface};
use sctk::seat::keyboard::{self, map_keyboard_repeat, RepeatKind};
use sctk::shm::AutoMemPool;
use sctk::window::{Event as WEvent, FallbackFrame};
use smithay::reexports::{
    wayland_commons::Interface,
    wayland_protocols::wlr::unstable::layer_shell::v1::client::{
        zwlr_layer_shell_v1, zwlr_layer_surface_v1,
    },
};
use wayland_client::{GlobalEvent, GlobalManager};

sctk::default_environment!(KbdInputExample, desktop);

pub fn new_client(
    loop_handle: calloop::LoopHandle<'static, GlobalState>,
) -> Result<DesktopClientState> {
    /*
     * Initial setup
     */
    let (env, display, mut queue) = sctk::new_default_environment!(KbdInputExample, desktop)
        .expect("Unable to connect to a Wayland compositor");

    let attached_display = display.attach(queue.token());
    let globals = GlobalManager::new(&attached_display);
    queue
        .sync_roundtrip(&mut (), |_, _, _| unreachable!())
        .unwrap();
    /*
     * Prepare a calloop event loop to handle key repetion
     */
    // Here `Option<WEvent>` is the type of a global value that will be shared by
    // all callbacks invoked by the event loop.
    /*
     * Create a buffer with window contents
     */

    let dimensions = (320u32, 240u32);

    /*
     * Init wayland objects
     */

    let surface = env.create_surface().detach();
    let wlr_layer_shell = globals
        .instantiate_exact::<zwlr_layer_shell_v1::ZwlrLayerShellV1>(1)
        .unwrap();
    // dbg!(wlr_layer_shell);
    let wlr_layer_surface = wlr_layer_shell.get_layer_surface(
        &surface,
        None,
        zwlr_layer_shell_v1::Layer::Top,
        "com.cosmic.xdg-wrapper".into(),
    );
    wlr_layer_surface.set_anchor(zwlr_layer_surface_v1::Anchor::empty());
    surface.commit();

    let mut window = env
        .create_window::<FallbackFrame, _>(
            surface,
            None,
            dimensions,
            move |evt, mut dispatch_data| {
                let shared_state = match dispatch_data.get::<GlobalState>() {
                    Some(s) => s,
                    None => {
                        eprintln!("Received window event before initializeing global state...");
                        return;
                    }
                };
                let next_action = &mut shared_state.desktop_client_state.next_wevent;
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
                    let surface = window.surface().clone();
                    pointer.quick_assign(move |_, event, _| {
                        send_pointer_event(event, &seat_name, &surface)
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
    let main_surface = window.surface().clone();

    let handle = loop_handle.clone();
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
                let surface = main_surface.clone();
                pointer.quick_assign(move |_, event, _| {
                    send_pointer_event(event, &seat_name, &surface)
                });
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

    // if !env.get_shell().unwrap().needs_configure() {
    //     // initial draw to bootstrap on wl_shell
    //     redraw(&mut pool, window.surface(), dimensions).expect("Failed to draw");
    //     window.refresh();
    // }

    sctk::WaylandSource::new(queue)
        .quick_insert(loop_handle)
        .unwrap();

    Ok(DesktopClientState {
        display,
        window,
        dimensions,
        pool,
        globals,
        seats: Default::default(),
        next_wevent: Default::default(),
    })
}

fn send_keyboard_event(event: keyboard::Event, _seat_name: &str) {
    // dbg!(event);
    //TODO forward event through embedded server
}

fn send_pointer_event(
    event: wl_pointer::Event,
    _seat_name: &str,
    _main_surface: &wl_surface::WlSurface,
) {
    // dbg!(event);
    //TODO forward event through embedded server
}

#[allow(clippy::many_single_char_names)]
pub fn redraw(
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
