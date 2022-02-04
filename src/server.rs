// SPDX-License-Identifier: GPL-3.0-only

use crate::util::*;
use anyhow::Result;
// use futures::io::{AsyncReadExt, AsyncWriteExt};
use sctk::reexports::calloop::{self, channel, generic::Generic, Interest, Mode, PostAction};
use sctk::reexports::client::protocol::wl_pointer;
use smithay::reexports::wayland_server;
use smithay::wayland::compositor::compositor_init;
use std::os::unix::{io::AsRawFd, net::UnixStream};
use std::time::Duration;

pub fn new_server(
    _tx: channel::SyncSender<ServerMsg>,
    client_rx: channel::Channel<ClientMsg>,
) -> Result<()> {
    let mut event_loop = calloop::EventLoop::<Option<()>>::try_new().unwrap();
    let mut display = wayland_server::Display::new();
    let (display_sock, client_sock) = UnixStream::pair().unwrap();
    display.add_socket_from(display_sock).unwrap();
    let display_client = unsafe { display.create_client(client_sock.as_raw_fd(), &mut ()) };
    compositor_init(&mut display, |_surface, _dispatch_data| {}, None);
    let display_event_source = Generic::new(display.get_poll_fd(), Interest::READ, Mode::Edge);
    event_loop.handle().insert_source(
        display_event_source,
        move |_e, _metadata, _shared_data| {
            display.dispatch(Duration::ZERO, &mut ())?;
            Ok(PostAction::Continue)
        },
    )?;

    // handle forwarded Events from desktop wayland server
    event_loop
        .handle()
        .insert_source(
            client_rx,
            move |event, _metadata, _shared_data| match event {
                channel::Event::Msg(e) => match e {
                    ClientMsg::WEvent(e) => {
                        dbg!(e);
                    }
                    ClientMsg::PtrEvent(e) => match e {
                        wl_pointer::Event::Enter {
                            surface_x,
                            surface_y,
                            ..
                        } => {
                            println!("Pointer entered at ({}, {})", surface_x, surface_y);
                        }
                        wl_pointer::Event::Leave { .. } => {
                            println!("Pointer left");
                        }
                        wl_pointer::Event::Button { button, state, .. } => {
                            println!("Button {:?} was {:?}", button, state);
                        }
                        wl_pointer::Event::Motion {
                            surface_x,
                            surface_y,
                            ..
                        } => {
                            println!("Pointer motion to ({}, {})", surface_x, surface_y)
                        }
                        _ => {}
                    },
                    ClientMsg::KbEvent(e) => match e {
                        KbEvent::Enter { keysyms, .. } => {
                            println!("Gained focus while {} keys pressed.", keysyms.len(),);
                        }
                        KbEvent::Leave { .. } => {
                            println!("Lost focus.");
                        }
                        KbEvent::Key {
                            keysym,
                            state,
                            utf8,
                            ..
                        } => {
                            println!("Key {:?}: {:x}.", state, keysym);
                            if let Some(txt) = utf8 {
                                println!(" -> Received text \"{}\".", txt);
                            }
                        }
                        KbEvent::Modifiers { modifiers } => {
                            println!("Modifiers changed to {:?}.", modifiers);
                        }
                        KbEvent::Repeat { keysym, utf8, .. } => {
                            println!("Key repetition {:x}.", keysym);
                            if let Some(txt) = utf8 {
                                println!(" -> Received text \"{}\".", txt);
                            }
                        }
                    },
                },
                _ => {}
            },
        )
        .unwrap();

    // start application and give
    let arg = ::std::env::args().nth(1);

    match arg.as_ref().map(|s| &s[..]) {
        Some(_path) => {
            // TODO start application using anonymous pipe & open fd
        }
        None => println!("USAGE: xdg_shell_wrapper <executable>"),
    }
    event_loop.run(None, &mut Some(()), |_| {}).unwrap();
    Ok(())
}
