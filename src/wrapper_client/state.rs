use std::{rc::Rc, time::Instant};

use sctk::{
    default_environment,
    environment::{Environment, SimpleGlobal},
    output::{with_output_info, OutputStatusListener},
    reexports::{
        client::{
            self,
            protocol::{wl_keyboard, wl_pointer, wl_seat, wl_shm, wl_surface},
            Attached,
        },
        protocols::{
            wlr::unstable::layer_shell::v1::client::zwlr_layer_shell_v1,
            xdg_shell::client::xdg_wm_base::XdgWmBase,
        },
    },
    seat::SeatHandling,
};
use slog::Logger;
use smithay::{reexports::wayland_server, wayland::seat};

use crate::{
    config::WrapperConfig,
    server_state::{EmbeddedServerState, SeatPair},
    shared_state::{AxisFrameData, GlobalState, OutputGroup},
    space::{SpaceEvent, WrapperSpace},
    wrapper_client::handlers::seat::send_keyboard_event,
};

use super::handlers::{
    output::{c_output_as_s_output, handle_output},
    seat::{seat_handle_callback, send_pointer_event},
};

#[derive(Debug)]
pub(crate) struct ClientSeat {
    pub(crate) seat: Attached<wl_seat::WlSeat>,
    pub(crate) kbd: Option<wl_keyboard::WlKeyboard>,
    pub(crate) ptr: Option<wl_pointer::WlPointer>,
}

/// currently focused client surface
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Focus {
    /// currently focused client surface
    Current(wl_surface::WlSurface),
    /// last instant a client surface was focused
    LastFocus(Instant),
}

#[derive(Debug)]
pub(crate) struct DesktopClientState {
    pub(crate) display: client::Display,
    pub(crate) cursor_surface: wl_surface::WlSurface,
    pub(crate) axis_frame: AxisFrameData,
    pub(crate) kbd_focus: bool,
    pub(crate) shm: Attached<wl_shm::WlShm>,
    pub(crate) xdg_wm_base: Attached<XdgWmBase>,
    pub(crate) env_handle: Environment<Env>,
    pub(crate) last_input_serial: Option<u32>,
    pub(crate) focused_surface: Focus,
    pub(crate) _output_listener: Option<OutputStatusListener>,
    pub(crate) _output_group: Vec<OutputGroup>,
}

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

impl std::fmt::Debug for Env {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Env")
            .field("sctk_compositor", &self.sctk_compositor)
            .field("sctk_subcompositor", &self.sctk_subcompositor)
            .field("sctk_shm", &self.sctk_shm)
            .field("sctk_outputs", &self.sctk_outputs)
            .field("sctk_seats", &self.sctk_seats)
            .field("sctk_data_device_manager", &self.sctk_data_device_manager)
            .field(
                "sctk_primary_selection_manager",
                &self.sctk_primary_selection_manager,
            )
            .field("layer_shell", &self.layer_shell)
            .field("xdg_wm_base", &self.xdg_wm_base)
            .finish()
    }
}

impl DesktopClientState {
    pub(crate) fn new<W: WrapperSpace + 'static>(
        loop_handle: calloop::LoopHandle<
            'static,
            (GlobalState<W>, wayland_server::Display<GlobalState<W>>),
        >,
        space: &mut W,
        log: Logger,
        dh: &mut wayland_server::DisplayHandle,
        embedded_server_state: &mut EmbeddedServerState<W>,
    ) -> anyhow::Result<Self> {
        let config = space.config();
        /*
         * Initial setup
         */
        let (mut env, display, queue) = sctk::new_default_environment!(
            Env,
            fields = [
                layer_shell: SimpleGlobal::new(),
                xdg_wm_base: SimpleGlobal::new(),
            ]
        )
        .expect("Unable to connect to a Wayland compositor");
        let _ = embedded_server_state
            .selected_data_provider
            .env_handle
            .set(env.clone());
        let focused_surface = Rc::clone(&embedded_server_state.focused_surface);
        let _attached_display = (*display).clone().attach(queue.token());

        let layer_shell = env.require_global::<zwlr_layer_shell_v1::ZwlrLayerShellV1>();
        let mut s_outputs = Vec::new();

        // TODO refactor to watch outputs and update space when outputs change or new outputs appear
        let outputs = env.get_all_outputs();
        match config.output() {
            None => {
                let pool = env
                    .create_auto_pool()
                    .expect("Failed to create a memory pool!");
                for o in &outputs {
                    if let Some(info) = with_output_info(&o, Clone::clone) {
                        let (s_o, _) = c_output_as_s_output::<W>(dh, &info, log.clone());
                        space.space().map_output(&s_o, info.location);
                    }
                }
                space
                    .add_output(
                        None,
                        None,
                        pool,
                        display.clone(),
                        layer_shell,
                        log.clone(),
                        env.create_surface(),
                        focused_surface,
                    )
                    .unwrap();
                let _ = space.bind_display(dh);
            }
            Some(configured_output) => {
                if let Some((o, info)) = outputs.iter().find_map(|o| {
                    with_output_info(o, Clone::clone).and_then(|info| {
                        if info.name == *configured_output {
                            Some((o, info))
                        } else {
                            None
                        }
                    })
                }) {
                    let (s_o, _) = c_output_as_s_output::<W>(dh, &info, log.clone());
                    space.space().map_output(&s_o, info.location);

                    let layer_shell = env.require_global::<zwlr_layer_shell_v1::ZwlrLayerShellV1>();
                    let env_handle = env.clone();
                    let logger = log.clone();
                    let display_ = display.clone();
                    handle_output(
                        &layer_shell,
                        env_handle,
                        logger,
                        display_,
                        o,
                        &info,
                        dh,
                        &mut s_outputs,
                        Rc::clone(&focused_surface),
                        space,
                    )
                } else {
                    eprintln!(
                        "Could not attach to configured output: {}",
                        configured_output
                    );
                    std::process::exit(1);
                }
            }
        };

        let output_listener = config.output().map(|configured_output| {
            env.listen_for_outputs(move |o, info, mut dispatch_data| {
                let (state, _server_display) = dispatch_data
                    .get::<(GlobalState<W>, wayland_server::Display<GlobalState<W>>)>()
                    .unwrap();
                if info.name == configured_output && info.obsolete {
                    state
                        .space
                        .next_space_event()
                        .replace(Some(SpaceEvent::Quit));
                    o.release();
                }
            })
        });

        // // TODO logging
        // // FIXME focus lost after drop from source outside xdg-shell-wrapper
        // // dnd listener
        // let last_motion = Rc::new(RefCell::new(None));
        // let _ = env.set_data_device_callback(move |seat, dnd_event, mut dispatch_data| {
        //     let (state, _) = dispatch_data
        //         .get::<(GlobalState<W>, wayland_server::Display)>()
        //         .unwrap();
        //     let DesktopClientState {
        //         seats,
        //         env_handle,
        //         space,
        //         ..
        //     } = &mut state.desktop_client_state;

        //     let EmbeddedServerState {
        //         focused_surface,
        //         last_button,
        //         ..
        //     } = &state.embedded_server_state;

        //     if let (Some(last_button), Some(seat)) =
        //         (last_button, seats.iter().find(|s| *(s.client.seat) == seat))
        //     {
        //         match dnd_event {
        //             sctk::data_device::DndEvent::Enter {
        //                 offer,
        //                 serial: _,
        //                 surface,
        //                 x,
        //                 y,
        //             } => {
        //                 let client = focused_surface
        //                     .borrow()
        //                     .as_ref()
        //                     .and_then(|focused_surface| {
        //                         let res = focused_surface.as_ref();
        //                         res.client()
        //                     });
        //                 set_data_device_focus(&seat.server.0, client);

        //                 set_focused_surface(focused_surface, space, &surface, x, y);
        //                 let offer = match offer {
        //                     Some(o) => o,
        //                     None => return,
        //                 };

        //                 let mime_types = offer.with_mime_types(|mime_types| Vec::from(mime_types));

        //                 offer.accept(mime_types.get(0).cloned());
        //                 let seat_clone = seat.client.seat.clone();
        //                 let env_clone = env_handle.clone();
        //                 start_dnd(
        //                     &seat.server.0,
        //                     SERIAL_COUNTER.next_serial(),
        //                     seat::PointerGrabStartData {
        //                         focus: focused_surface
        //                             .borrow()
        //                             .as_ref()
        //                             .map(|s| (s.clone(), (0, 0).into())),
        //                         button: *last_button,
        //                         location: (x, y).into(),
        //                     },
        //                     SourceMetadata {
        //                         mime_types: mime_types.clone(),
        //                         dnd_action: DndAction::from_raw(offer.get_available_actions().to_raw())
        //                             .unwrap(),
        //                     },
        //                     move |server_dnd_event| match server_dnd_event {
        //                         smithay::wayland::data_device::ServerDndEvent::Action(action) => {
        //                             let _ = env_clone.with_data_device(&seat_clone, |device| {
        //                                 device.with_dnd(|offer| {
        //                                     if let Some(offer) = offer {
        //                                         let action =
        //                                             data_device::DndAction::from_raw(action.to_raw())
        //                                                 .unwrap();
        //                                         offer.set_actions(action, action);
        //                                     }
        //                                 });
        //                             });
        //                         }
        //                         smithay::wayland::data_device::ServerDndEvent::Dropped => {}
        //                         smithay::wayland::data_device::ServerDndEvent::Cancelled => {
        //                             let _ = env_clone.with_data_device(&seat_clone, |device| {
        //                                 device.with_dnd(|offer| {
        //                                     if let Some(offer) = offer {
        //                                         offer.finish();
        //                                     }
        //                                 });
        //                             });
        //                         }
        //                         smithay::wayland::data_device::ServerDndEvent::Send {
        //                             mime_type,
        //                             fd,
        //                         } => {
        //                             if mime_types.contains(&mime_type) {
        //                                 let _ = env_clone.with_data_device(&seat_clone, |device| {
        //                                     device.with_dnd(|offer| {
        //                                         if let Some(offer) = offer {
        //                                             unsafe { offer.receive_to_fd(mime_type, fd) };
        //                                         }
        //                                     });
        //                                 });
        //                             }
        //                         }
        //                         smithay::wayland::data_device::ServerDndEvent::Finished => {
        //                             // println!("finished");
        //                             let _ = env_clone.with_data_device(&seat_clone, |device| {
        //                                 device.with_dnd(|offer| {
        //                                     if let Some(offer) = offer {
        //                                         offer.finish();
        //                                     }
        //                                 });
        //                             });
        //                         }
        //                     },
        //                 )
        //             }
        //             sctk::data_device::DndEvent::Motion {
        //                 offer: _,
        //                 time,
        //                 x,
        //                 y,
        //             } => {
        //                 last_motion.replace(Some(((x, y), time)));
        //                 space.update_pointer((x as i32, y as i32));

        //                 handle_motion(
        //                     space,
        //                     focused_surface.borrow().clone(),
        //                     x,
        //                     y,
        //                     seat.server.0.get_pointer().unwrap(),
        //                     time,
        //                 );
        //             }
        //             sctk::data_device::DndEvent::Leave => {}
        //             sctk::data_device::DndEvent::Drop { .. } => {
        //                 if let Some(((_, _), time)) = last_motion.take() {
        //                     seat.server.0.get_pointer().unwrap().button(
        //                         *last_button,
        //                         ButtonState::Released,
        //                         SERIAL_COUNTER.next_serial(),
        //                         time + 1,
        //                     );
        //                 }
        //             }
        //         }
        //     }
        // });

        /*
         * Keyboard initialization
         */

        let mut seats = Vec::<SeatPair<W>>::new();

        // first process already existing seats
        let env_handle = env.clone();
        let event_loop = loop_handle.clone();
        for seat in env.get_all_seats() {
            if let Some((has_kbd, has_ptr, name)) = sctk::seat::with_seat_data(&seat, |seat_data| {
                (
                    seat_data.has_keyboard && !seat_data.defunct,
                    seat_data.has_pointer && !seat_data.defunct,
                    seat_data.name.clone(),
                )
            }) {
                let mut new_seat = SeatPair {
                    name: name.clone(),
                    server: seat::Seat::new(dh, name.clone(), log.clone()),
                    client: ClientSeat {
                        kbd: None,
                        ptr: None,
                        seat: seat.clone(),
                    },
                };
                if has_kbd || has_ptr {
                    if has_kbd {
                        let seat_name = name.clone();
                        let kbd = seat.get_keyboard();
                        kbd.quick_assign(move |_, event, dispatch_data| {
                            send_keyboard_event::<W>(event, &seat_name, dispatch_data)
                        });
                        new_seat.client.kbd = Some(kbd.detach());
                        new_seat.server.add_keyboard(
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
                            send_pointer_event::<W>(event, &seat_name, dispatch_data)
                        });
                        new_seat.client.ptr = Some(pointer.detach());
                        new_seat.server.add_pointer(move |_new_status| {});
                    }
                }
                embedded_server_state.seats.push(new_seat);
            }
        }
        // set server device selection when offer should be available
        // event_loop.insert_idle(move |(state, _)| {
        //     let seats = &mut state.desktop_client_state.seats;
        //     for s in seats {
        //         let _ = set_server_device_selection(
        //             &env_handle,
        //             &s.client.seat,
        //             &s.server,
        //             &state.embedded_server_state.selected_data_provider.seat,
        //         );
        //     }
        // });

        // then setup a listener for changes
        let logger = log.clone();
        env.with_inner(|env_inner| {
            env_inner.listen(move |seat, seat_data, dispatch_data| {
                seat_handle_callback::<W>(logger.clone(), seat, seat_data, dispatch_data)
            })
        });

        sctk::WaylandSource::new(queue)
            .quick_insert(loop_handle)
            .unwrap();

        let cursor_surface = env.create_surface().detach();

        let shm = env.require_global::<wl_shm::WlShm>();
        let xdg_wm_base = env.require_global::<XdgWmBase>();

        Ok(DesktopClientState {
            display,
            kbd_focus: false,
            axis_frame: Default::default(),
            cursor_surface,
            shm,
            xdg_wm_base,
            env_handle: env,
            last_input_serial: None,
            focused_surface: Focus::LastFocus(Instant::now()),
            _output_listener: output_listener,
            _output_group: s_outputs,
        })
    }
}
