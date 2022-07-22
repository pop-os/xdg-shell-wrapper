use std::{rc::Rc, cell::RefCell};

use sctk::{
    default_environment,
    environment::{Environment, SimpleGlobal},
    output::{with_output_info, OutputStatusListener, XdgOutputHandler},
    reexports::{
        client::{
            self,
            protocol::{wl_keyboard, wl_pointer, wl_seat::{WlSeat}, wl_shm, wl_surface},
            Attached, Display, Proxy,
        },
        protocols::{
            unstable::xdg_output::v1::client::zxdg_output_manager_v1,
            wlr::unstable::layer_shell::v1::client::zwlr_layer_shell_v1,
            xdg_shell::client::xdg_wm_base::XdgWmBase,
        },
    },
    seat::SeatHandling,
};
use slog::Logger;
use smithay::{
    reexports::{calloop, wayland_server},
    wayland::seat,
};

use crate::{
    client::handlers::seat::send_keyboard_event,
    config::WrapperConfig,
    server_state::{ServerState, SeatPair},
    shared_state::{AxisFrameData, GlobalState, OutputGroup},
    space::WrapperSpace,
};

use super::handlers::{
    output::{c_output_as_s_output, handle_output},
    seat::{seat_handle_callback, send_pointer_event},
};

#[derive(Debug)]
pub(crate) struct ClientSeat {
    pub(crate) _seat: Attached<WlSeat>,
    pub(crate) kbd: Option<wl_keyboard::WlKeyboard>,
    pub(crate) ptr: Option<wl_pointer::WlPointer>,
}

/// list of focused surfaces and the seats that focus them
pub type ClientFocus = Rc<RefCell<Vec<(wl_surface::WlSurface, String)>>>;

/// Wrapper client state
#[derive(Debug)]
pub struct ClientState {
    /// the sctk environment
    pub env_handle: Environment<Env>,
    /// the last input serial
    pub last_input_serial: Option<u32>,
    /// state regarding the last embedded client surface with keyboard focus
    pub focused_surface: ClientFocus,
    /// state regarding the last embedded client surface with keyboard focus
    pub hovered_surface: ClientFocus,
    pub(crate) display: client::Display,
    pub(crate) cursor_surface: wl_surface::WlSurface,
    pub(crate) axis_frame: Vec<AxisFrameData>,
    pub(crate) shm: Attached<wl_shm::WlShm>,
    pub(crate) xdg_wm_base: Attached<XdgWmBase>,
    pub(crate) _output_listener: Option<OutputStatusListener>,
    pub(crate) _output_group: Vec<OutputGroup>,
}

default_environment!(Env,
    fields = [
        layer_shell: SimpleGlobal<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
        xdg_wm_base: SimpleGlobal<XdgWmBase>,
        sctk_xdg_out: XdgOutputHandler,
    ],
    singles = [
        zwlr_layer_shell_v1::ZwlrLayerShellV1 => layer_shell,
        zxdg_output_manager_v1::ZxdgOutputManagerV1 => sctk_xdg_out,
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
            .field("sctk_xdg_out", &self.sctk_xdg_out)
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

impl ClientState {
    pub(crate) fn new<W: WrapperSpace + 'static>(
        loop_handle: calloop::LoopHandle<
            'static,
            (GlobalState<W>, wayland_server::Display<GlobalState<W>>),
        >,
        space: &mut W,
        log: Logger,
        dh: &mut wayland_server::DisplayHandle,
        embedded_server_state: &mut ServerState<W>,
    ) -> anyhow::Result<Self> {
        let config = space.config();
        /*
         * Initial setup
         */
        let display = Display::connect_to_env()?;
        let mut queue = display.create_event_queue();
        let env = {
            use sctk::{
                data_device::DataDeviceHandler, primary_selection::PrimarySelectionHandler,
                seat::SeatHandler, shm::ShmHandler,
            };

            let mut sctk_seats = SeatHandler::new();
            let sctk_data_device_manager = DataDeviceHandler::init(&mut sctk_seats);
            let sctk_primary_selection_manager = PrimarySelectionHandler::init(&mut sctk_seats);
            let (sctk_outputs, sctk_xdg_out) = XdgOutputHandler::new_output_handlers();

            let display = Proxy::clone(&display);
            let env = Environment::new(
                &display.attach(queue.token()),
                &mut queue,
                Env {
                    sctk_compositor: SimpleGlobal::new(),
                    sctk_subcompositor: SimpleGlobal::new(),
                    sctk_shm: ShmHandler::new(),
                    sctk_outputs,
                    sctk_xdg_out,
                    sctk_seats,
                    sctk_data_device_manager,
                    sctk_primary_selection_manager,
                    layer_shell: SimpleGlobal::new(),
                    xdg_wm_base: SimpleGlobal::new(),
                },
            );

            if let Ok(env) = env.as_ref() {
                // Bind primary selection manager.
                let _psm = env.get_primary_selection_manager();
            }

            env
        }?;

        let _ = embedded_server_state
            .selected_data_provider
            .env_handle
            .set(env.clone());
        let _attached_display = (*display).clone().attach(queue.token());

        let mut s_outputs = Vec::new();

        // TODO refactor to watch outputs and update space when outputs change or new outputs appear
        let outputs = env.get_all_outputs();
        let s_focused_surface = embedded_server_state.focused_surface.clone();
        let c_focused_surface: ClientFocus = Default::default();
        let c_hovered_surface: ClientFocus = Default::default();
        space.setup(&env, display.clone(), c_focused_surface.clone(), c_hovered_surface.clone(), s_focused_surface.clone(), s_focused_surface.clone());

        for o in &outputs {
            if let Some(info) = with_output_info(&o, Clone::clone) {
                let (s_o, _) = c_output_as_s_output::<W>(dh, &info, log.clone());
                space.space().map_output(&s_o, info.location);
            }
        }
        let configured_outputs = match config.outputs() {
            xdg_shell_wrapper_config::WrapperOutput::All => outputs
                .iter()
                .filter_map(|o| with_output_info(o, Clone::clone).map(|info| info.name))
                .collect(),
            xdg_shell_wrapper_config::WrapperOutput::Name(list) => list,
        };

        if configured_outputs.is_empty() {
            space.handle_output(&env, None, None).unwrap();
        } else {
            for o in &outputs {
                if let Some(info) = with_output_info(&o, Clone::clone)
                    .and_then(|info| {
                        if configured_outputs
                            .iter()
                            .find(|configured| *configured == &info.name).is_some()
                        {
                            Some(info)
                        } else {
                            None
                        }
                    })
                {
                    let env_handle = env.clone();
                    let logger = log.clone();
                    handle_output(&env_handle, logger, o, &info, dh, &mut s_outputs, space);
                }
            }
        }

        let output_listener = if configured_outputs.is_empty() {
            None
        } else {
            Some(env.listen_for_outputs(move |o, info, mut dispatch_data| {
                let (state, sd) = dispatch_data
                    .get::<(GlobalState<W>, wayland_server::Display<GlobalState<W>>)>()
                    .unwrap();
                if !info.obsolete {
                    return;
                }

                if configured_outputs
                    .iter()
                    .any(|configured| configured == &info.name)
                {
                    let GlobalState {
                        desktop_client_state:
                            ClientState {
                                env_handle,
                                _output_group,
                                ..
                            },
                        space,
                        log,
                        ..
                    } = state;
                    handle_output(
                        env_handle,
                        log.clone(),
                        &o,
                        &info,
                        &mut sd.handle(),
                        _output_group,
                        space,
                    );
                }
            }))
        };

        // // TODO reimplement when sctk 0.30 is ready
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

        // first process already existing seats
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
                        _seat: seat.clone(),
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

        Ok(ClientState {
            display,
            axis_frame: Default::default(),
            cursor_surface,
            shm,
            xdg_wm_base,
            env_handle: env,
            last_input_serial: None,
            focused_surface: c_focused_surface,
            hovered_surface: c_hovered_surface,
            _output_listener: output_listener,
            _output_group: s_outputs,
        })
    }
}
