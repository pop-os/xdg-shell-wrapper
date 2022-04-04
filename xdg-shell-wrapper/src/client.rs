use anyhow::Result;
use sctk::{
    default_environment,
    environment::SimpleGlobal,
    output::with_output_info,
    reexports::{
        calloop,
        client::{protocol::wl_shm, GlobalManager},
    },
};
use slog::{trace, Logger};
use smithay::{
    reexports::{
        wayland_protocols::{
            wlr::unstable::layer_shell::v1::client::zwlr_layer_shell_v1,
            xdg_shell::client::xdg_wm_base::XdgWmBase,
        },
        wayland_server,
    },
    wayland::seat,
};

// SPDX-License-Identifier: MPL-2.0-only
use crate::shared_state::*;
use crate::XdgWrapperConfig;

sctk::default_environment!(KbdInputExample, desktop);

default_environment!(Env,
    fields = [
        layer_shell: SimpleGlobal<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
        xdg_wm_base: SimpleGlobal<XdgWmBase>,
    ],
    singles = [
        zwlr_layer_shell_v1::ZwlrLayerShellV1 => layer_shell,
        XdgWmBase => xdg_wm_base,
    ],
);

pub fn new_client(
    loop_handle: calloop::LoopHandle<'static, (GlobalState, wayland_server::Display)>,
    config: XdgWrapperConfig,
    log: Logger,
    server_display: &mut wayland_server::Display,
) -> Result<(DesktopClientState, Vec<OutputGroup>)> {
    /*
     * Initial setup
     */
    let (env, display, queue) = sctk::new_default_environment!(
        Env,
        fields = [
            layer_shell: SimpleGlobal::new(),
            xdg_wm_base: SimpleGlobal::new(),
        ]
    )
    .expect("Unable to connect to a Wayland compositor");

    let attached_display = (*display).clone().attach(queue.token());
    let globals = GlobalManager::new(&attached_display);

    let mut renderer = None;

    let mut s_outputs = Vec::new();
    for output in env.get_all_outputs() {
        if let Some(info) = with_output_info(&output, Clone::clone) {
            let layer_shell = env.require_global::<zwlr_layer_shell_v1::ZwlrLayerShellV1>();
            let env_handle = env.clone();
            let logger = log.clone();
            let display_ = display.clone();
            let config = config.clone();
            handle_output(
                config,
                &layer_shell,
                env_handle,
                &mut renderer,
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
    let logger = log.clone();
    let display_ = display.clone();
    let output_listener = env.listen_for_outputs(move |output, info, mut dispatch_data| {
        let (state, server_display) = dispatch_data
            .get::<(GlobalState, wayland_server::Display)>()
            .unwrap();
        let outputs = &mut state.outputs;
        let renderer = &mut state.desktop_client_state.renderer;
        handle_output(
            config.clone(),
            &layer_shell,
            env_handle.clone(),
            renderer,
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

    let mut seats = Vec::<Seat>::new();

    // first process already existing seats
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
            seats.push(new_seat);
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

    let cursor_surface = env.create_surface().detach();

    let shm = env.require_global::<wl_shm::WlShm>();
    let xdg_wm_base = env.require_global::<XdgWmBase>();

    trace!(log.clone(), "client setup complete");
    Ok((
        DesktopClientState {
            renderer,
            display,
            output_listener,
            seat_listener,
            seats: seats,
            kbd_focus: false,
            axis_frame: Default::default(),
            cursor_surface: cursor_surface,
            globals,
            shm,
            xdg_wm_base,
            env_handle: env,
            last_input_serial: None,
            focused_surface: None,
        },
        s_outputs,
    ))
}
