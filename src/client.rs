use std::cell::RefCell;
use std::rc::Rc;

use anyhow::Result;
use sctk::{
    default_environment, environment::SimpleGlobal, output::with_output_info, reexports::calloop,
};
use slog::{trace, Logger};
use smithay::reexports::wayland_protocols::wlr::unstable::layer_shell::v1::client::zwlr_layer_shell_v1;
use smithay::wayland::seat::{self};

// SPDX-License-Identifier: GPL-3.0-only
use crate::shared_state::*;
use crate::XdgWrapperConfig;

sctk::default_environment!(KbdInputExample, desktop);

default_environment!(Env,
    fields = [
        layer_shell: SimpleGlobal<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    ],
    singles = [
        zwlr_layer_shell_v1::ZwlrLayerShellV1 => layer_shell
    ],
);

pub fn new_client(
    loop_handle: calloop::LoopHandle<'static, GlobalState>,
    config: XdgWrapperConfig,
    log: Logger,
    server_state: &mut EmbeddedServerState,
) -> Result<(DesktopClientState, Vec<OutputGroup>)> {
    /*
     * Initial setup
     */
    let (env, display, queue) =
        sctk::new_default_environment!(Env, fields = [layer_shell: SimpleGlobal::new(),])
            .expect("Unable to connect to a Wayland compositor");

    let surface = Rc::new(RefCell::new(None));

    let server_display = &mut server_state.display;

    let mut s_outputs = Vec::new();
    for output in env.get_all_outputs() {
        if let Some(info) = with_output_info(&output, Clone::clone) {
            let layer_shell = env.require_global::<zwlr_layer_shell_v1::ZwlrLayerShellV1>();
            let env_handle = env.clone();
            let surface_handle = Rc::clone(&surface);
            let logger = log.clone();
            let display_ = display.clone();
            let config = config.clone();
            handle_output(
                config,
                &layer_shell,
                env_handle,
                surface_handle,
                logger,
                display_,
                output,
                &info,
                server_display,
                &mut s_outputs,
            );
        }
    }

    let layer_shell = env.require_global::<zwlr_layer_shell_v1::ZwlrLayerShellV1>();
    let env_handle = env.clone();
    let surface_handle = Rc::clone(&surface);
    let logger = log.clone();
    let display_ = display.clone();
    let output_listener = env.listen_for_outputs(move |output, info, mut dispatch_data| {
        let state = dispatch_data.get::<GlobalState>().unwrap();
        let server_display = &mut state.embedded_server_state.display;
        let outputs = &mut state.outputs;
        handle_output(
            config.clone(),
            &layer_shell,
            env_handle.clone(),
            surface_handle.clone(),
            logger.clone(),
            display_.clone(),
            output,
            &info,
            server_display,
            outputs,
        );
    });

    /*
     * Keyboard initialization
     */

    let mut seats = Vec::<(String, Seat)>::new();

    // first process already existing seats
    // TODO create seats on server
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
            seats.push((name.clone(), new_seat));
        }
    }

    // then setup a listener for changes

    let logger = log.clone();
    let seat_listener = env.listen_for_seats(move |seat, seat_data, dispatch_data| {
        seat_handler(logger.clone(), seat, seat_data, dispatch_data)
    });

    sctk::WaylandSource::new(queue)
        .quick_insert(loop_handle)
        .unwrap();

    Ok((
        DesktopClientState {
            surface,
            display,
            output_listener,
            seat_listener,
            seats: Default::default(),
        },
        s_outputs,
    ))
}
